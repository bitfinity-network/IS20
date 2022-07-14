//! Module     : factory
//! Copyright  : 2022 InfinitySwap Team
//! Stability  : Experimental

use std::cell::RefCell;
use std::rc::Rc;

use crate::state::StableState;
use crate::{error::TokenFactoryError, state::State};
use candid::Principal;
use ic_canister::{init, post_upgrade, pre_upgrade, query, update, Canister, PreUpdate};
use ic_factory::{api::FactoryCanister, error::FactoryError, FactoryConfiguration, FactoryState};
use ic_helpers::candid_header::{candid_header, CandidHeader};
use token::types::Metadata;

const DEFAULT_LEDGER_PRINCIPAL: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai";
const DEFAULT_ICP_FEE: u64 = 10u64.pow(8); // 1 ICP

#[cfg(not(feature = "no_api"))]
mod inspect_message;

#[derive(Clone, Canister)]
#[canister_no_upgrade_methods]
pub struct TokenFactoryCanister {
    #[id]
    principal: Principal,

    #[state]
    state: Rc<RefCell<State>>,
}

#[allow(dead_code)]
impl TokenFactoryCanister {
    #[pre_upgrade]
    fn pre_upgrade(&self) {
        let token_factory_state = self.state.replace(State::default());
        let base_factory_state = self.factory_state().replace(FactoryState::default());

        ic_storage::stable::write(&StableState {
            token_factory_state,
            base_factory_state,
        })
        .expect("failed to serialize state to the stable storage");
    }

    #[post_upgrade]
    fn post_upgrade(&self) {
        let stable_state = ic_storage::stable::read::<StableState>()
            .expect("failed to read stable state from the stable storage");
        let StableState {
            token_factory_state,
            base_factory_state,
        } = stable_state;

        self.state.replace(token_factory_state);
        self.factory_state().replace(base_factory_state);
    }

    #[init]
    pub fn init(&self, controller: Principal, ledger_principal: Option<Principal>) {
        let ledger = ledger_principal
            .unwrap_or_else(|| Principal::from_text(DEFAULT_LEDGER_PRINCIPAL).unwrap());

        let mut factory_state = FactoryState::default();
        let factory_configuration =
            FactoryConfiguration::new(ledger, DEFAULT_ICP_FEE, controller, controller);
        ic_cdk::println!("factory controller: {}", controller.to_string());

        factory_state.configuration = factory_configuration;

        self.factory_state().replace(factory_state);
    }

    /// Returns the token, or None if it does not exist.
    #[query]
    pub async fn get_token(&self, name: String) -> Option<Principal> {
        let principal = Principal::from_text(name).unwrap();
        self.factory_state().borrow().factory.get(&principal)
    }

    #[update]
    pub async fn set_token_bytecode(&self, bytecode: Vec<u8>) {
        self.state.borrow_mut().token_wasm.replace(bytecode);
    }

    /// Creates a new token.
    ///
    /// Creating a token canister with the factory requires one of the following:
    /// * the call must be made through a cycles wallet with enough cycles to cover the canister
    ///   expenses. The amount of provided cycles must be greater than `10^12`. Most of the cycles
    ///   will be added to the newly created canister balance, while some will be consumed by the
    ///   factory
    /// * the caller must transfer some amount of ICP to their subaccount into the ICP ledger factory account.
    ///   The subaccount id can be calculated like this:
    ///
    /// ```ignore
    /// let mut subaccount = [0u8; 32];
    /// let principal_id = caller_id.as_slice();
    /// subaccount[0] = principal_id.len().try_into().unwrap();
    /// subaccount[1..1 + principal_id.len()].copy_from_slice(principal_id);
    /// ```
    ///
    /// The amount of provided ICP must be greater than the `icp_fee` factory property. This value
    /// can be obtained by the `get_icp_fee` query method. The ICP fees are transferred to the
    /// principal designated by the factory controller. The canister is then created with some
    /// minimum amount of cycles.
    ///
    /// If the provided ICP amount is greater than required by the factory, extra ICP will not be
    /// consumed and can be used to create more canisters, or can be reclaimed by calling `refund_icp`
    /// method.
    #[update]
    pub async fn create_token(
        &self,
        info: Metadata,
        owner: Option<Principal>,
    ) -> Result<Principal, TokenFactoryError> {
        if info.name.is_empty() {
            return Err(TokenFactoryError::InvalidConfiguration(
                "name",
                "cannot be `None`",
            ));
        }

        if info.symbol.is_empty() {
            return Err(TokenFactoryError::InvalidConfiguration(
                "symbol",
                "cannot be `None`",
            ));
        }

        let key = info.name.clone();
        if self.state.borrow().tokens.contains_key(&key) {
            return Err(TokenFactoryError::AlreadyExists);
        }

        let caller = owner.unwrap_or_else(ic_canister::ic_kit::ic::caller);
        let actor = self
            .factory_state()
            .borrow()
            .consume_provided_cycles_or_icp(caller);
        let cycles = actor.await?;

        let create_token = {
            let state = self.state.borrow();
            let wasm = state
                .token_wasm
                .as_ref()
                .expect("token_wasm is not set in token state");

            self.factory_state()
                .borrow()
                .factory
                .create_with_cycles(wasm, (info,), cycles)
        };

        let canister = create_token
            .await
            .map_err(|e| TokenFactoryError::CanisterCreateFailed(e.1))?;
        let principal = canister.identity();

        self.factory_state()
            .borrow_mut()
            .factory
            .register(principal, canister);

        self.state.borrow_mut().tokens.insert(key, principal);

        Ok(principal)
    }

    /// Delete a token.
    /// The token must be owned by the caller.
    /// The token will be deleted from the factory and the removed from the canister registry.
    #[update]
    pub async fn forget_token(&self, name: String) -> Result<(), TokenFactoryError> {
        //    Check controller access
        self.factory_state()
            .borrow_mut()
            .check_controller_access()?;

        let token = self
            .get_token(name.clone())
            .await
            .ok_or(TokenFactoryError::FactoryError(FactoryError::NotFound))?;

        let drop_token_fut = self.factory_state().borrow_mut().factory.drop(token);
        drop_token_fut.await?;
        let _ = self.state.borrow_mut().tokens.remove(&name);
        let name = Principal::from_text(name).unwrap();
        self.factory_state().borrow_mut().factory.forget(&name)?;

        Ok(())
    }

    #[update]
    pub async fn upgrade(&mut self) -> Result<Vec<Principal>, FactoryError> {
        let wasm = self.state.borrow().token_wasm.clone();
        let result = FactoryCanister::upgrade(self, wasm).await;
        result
    }

    #[query]
    pub fn state_header(&self) -> CandidHeader {
        candid_header::<StableState>()
    }
}

impl PreUpdate for TokenFactoryCanister {}
impl FactoryCanister for TokenFactoryCanister {}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_token_bytecode_impl() {
        ic_canister::ic_kit::MockContext::new().inject();
        let factory = TokenFactoryCanister::init_instance();
        assert_eq!(factory.state.borrow().token_wasm, None);
        factory.set_token_bytecode(vec![12, 3]).await;
        assert_eq!(factory.state.borrow().token_wasm, Some(vec![12, 3]));
    }
}
