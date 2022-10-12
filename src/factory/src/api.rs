//! Module     : factory
//! Copyright  : 2022 InfinitySwap Team
//! Stability  : Experimental

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::state::StableState;
use crate::{error::TokenFactoryError, state::State};
use candid::Principal;
use canister_sdk::ic_factory::DEFAULT_ICP_FEE;
use canister_sdk::ic_metrics::{Metrics, MetricsStorage};
use canister_sdk::{
    ic_canister::{
        init, post_upgrade, pre_upgrade, query, update, Canister, MethodType, PreUpdate,
    },
    ic_factory::{
        api::{FactoryCanister, UpgradeResult},
        error::FactoryError,
        FactoryConfiguration, FactoryState,
    },
    ic_helpers::{
        candid_header::{candid_header, CandidHeader},
        tokens::Tokens128,
    },
    ic_kit::ic,
    ic_storage,
};
use token::state::config::Metadata;

const DEFAULT_LEDGER_PRINCIPAL: Principal = Principal::from_slice(&[0, 0, 0, 0, 0, 0, 0, 2, 1, 1]);

#[cfg(feature = "export-api")]
mod inspect_message;

#[derive(Clone, Canister)]
#[canister_no_upgrade_methods]
pub struct TokenFactoryCanister {
    #[id]
    principal: Principal,

    #[state]
    pub state: Rc<RefCell<State>>,
}

impl Metrics for TokenFactoryCanister {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        <MetricsStorage as ic_storage::IcStorage>::get()
    }
}
impl PreUpdate for TokenFactoryCanister {
    fn pre_update(&self, _method_name: &str, _method_type: MethodType) {
        self.update_metrics();
    }
}

#[allow(dead_code)]
impl TokenFactoryCanister {
    #[query]
    fn git_tag(&self) -> &'static str {
        option_env!("GIT_TAG").unwrap_or("NOT_FOUND")
    }

    #[pre_upgrade]
    fn pre_upgrade(&self) {
        let token_factory_state = self.state.replace(State::default());
        let base_factory_state = self.factory_state().replace(FactoryState::default());

        if let Err(err) = ic_storage::stable::write(&StableState {
            token_factory_state,
            base_factory_state,
        }) {
            ic::trap(&format!(
                "Error while serializing state to the stable storage: {err}"
            ));
        }
    }

    #[post_upgrade]
    fn post_upgrade(&self) {
        let stable_state = ic_storage::stable::read::<StableState>().unwrap_or_else(|err| {
            ic::trap(&format!(
                "Error while deserializing state from the stable storage: {err}",
            ));
        });
        let StableState {
            token_factory_state,
            base_factory_state,
        } = stable_state;

        self.state.replace(token_factory_state);
        self.factory_state().replace(base_factory_state);
    }

    #[init]
    pub fn init(&self, controller: Principal, ledger_principal: Option<Principal>) {
        let ledger = ledger_principal.unwrap_or(DEFAULT_LEDGER_PRINCIPAL);

        let factory_configuration =
            FactoryConfiguration::new(ledger, DEFAULT_ICP_FEE, controller, controller);

        self.factory_state()
            .replace(FactoryState::new(factory_configuration));
    }

    /// Returns the token, or None if it does not exist.
    #[query]
    pub async fn get_token(&self, name: String) -> Option<Principal> {
        self.state.borrow().tokens.get(&name).copied()
    }

    #[update]
    pub async fn set_token_bytecode(&self, bytecode: Vec<u8>) -> Result<u32, FactoryError> {
        let state_header = candid_header::<()>();
        self.state.borrow_mut().token_wasm = Some(bytecode.clone());
        self.set_canister_code::<()>(bytecode, state_header)
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
        amount: Tokens128,
        controller: Option<Principal>,
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

        let caller = canister_sdk::ic_kit::ic::caller();
        let principal = self
            .create_canister((info, amount), controller, Some(caller))
            .await?;
        self.state.borrow_mut().tokens.insert(key, principal);

        Ok(principal)
    }

    #[update]
    pub async fn forget_token(&self, name: String) -> Result<(), TokenFactoryError> {
        let canister_id = self
            .get_token(name.clone())
            .await
            .ok_or(TokenFactoryError::FactoryError(FactoryError::NotFound))?;

        self.drop_canister(canister_id, None).await?;
        self.state.borrow_mut().tokens.remove(&name);

        Ok(())
    }

    #[update]
    pub async fn upgrade(&mut self) -> Result<HashMap<Principal, UpgradeResult>, FactoryError> {
        self.upgrade_canister::<()>().await
    }

    #[query]
    pub fn state_header(&self) -> CandidHeader {
        candid_header::<StableState>()
    }
}

impl FactoryCanister for TokenFactoryCanister {
    fn factory_state(&self) -> Rc<RefCell<FactoryState>> {
        use canister_sdk::ic_storage::IcStorage;
        FactoryState::get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_principal() {
        const LEDGER: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai";
        let original_principal = Principal::from_text(LEDGER).unwrap();
        assert_eq!(DEFAULT_LEDGER_PRINCIPAL, original_principal);
    }
}
