//! Module     : factory
//! Copyright  : 2022 InfinitySwap Team
//! Stability  : Experimental

use std::cell::RefCell;
use std::rc::Rc;

use crate::error::TokenFactoryError;
use crate::state::State;
use candid::{Nat, Principal};
use common::types::Metadata;
use ic_canister::{init, query, update, Canister};
use ic_helpers::factory::error::FactoryError;
use ic_helpers::factory::FactoryState;

mod inspect_message;

ic_helpers::extend_with_factory_api!(
    TokenFactoryCanister,
    state,
    crate::state::get_token_bytecode()
);

#[derive(Clone, Canister)]
pub struct TokenFactoryCanister {
    #[id]
    principal: Principal,

    #[state]
    state: Rc<RefCell<State>>,
}

#[allow(dead_code)]
impl TokenFactoryCanister {
    #[init]
    fn init(&self, controller: Principal, ledger_principal: Option<Principal>) {
        self.state.replace(State::new(controller, ledger_principal));
    }

    /// Returns the token, or None if it does not exist.
    #[query]
    async fn get_token(&self, name: String) -> Option<Principal> {
        self.state.borrow().factory.get(&name)
    }

    #[update]
    async fn set_token_bytecode(&self, bytecode: Vec<u8>) {
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
    async fn create_token(
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

        if self.state.borrow().factory.get(&key).is_some() {
            return Err(TokenFactoryError::AlreadyExists);
        }

        let caller = owner.unwrap_or_else(ic_kit::ic::caller);
        let actor = self.state.borrow().consume_provided_cycles_or_icp(caller);
        let cycles = actor.await?;

        let state_ref = &mut *self.state.borrow_mut();

        let wasm = state_ref
            .token_wasm
            .as_ref()
            .expect("token_wasm is not set in token state");

        let create_token = state_ref.factory.create_with_cycles(&wasm, (info,), cycles);

        let canister = create_token
            .await
            .map_err(|e| TokenFactoryError::CanisterCreateFailed(e.1))?;
        let principal = canister.identity();

        state_ref.factory.register(key, canister);

        Ok(principal)
    }

    /// Delete a token.
    /// The token must be owned by the caller.
    /// The token will be deleted from the factory and the removed from the canister registry.
    #[update]
    async fn delete_token(&self, name: String) -> Result<(), TokenFactoryError> {
        //    Check controller access
        let state_ref = &mut *self.state.borrow_mut();
        state_ref.check_controller_access()?;

        let token = self.get_token(name.clone()).await;

        if token.is_none() {
            return Err(TokenFactoryError::FactoryError(FactoryError::NotFound));
        }

        state_ref
            .factory()
            .drop(token.expect("token exists, checked above"))
            .await?;
        state_ref.factory_mut().forget(&name)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_token_bytecode_impl() {
        ic_kit::MockContext::new().inject();
        let factory = TokenFactoryCanister::init_instance();
        assert_eq!(factory.state.borrow().token_wasm, None);
        factory.set_token_bytecode(vec![12, 3]).await;
        assert_eq!(factory.state.borrow().token_wasm, Some(vec![12, 3]));
    }
}
