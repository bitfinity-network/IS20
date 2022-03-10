//! Module     : factory
//! Copyright  : 2021 InfinitySwap Team
//! Stability  : Experimental

use crate::error::TokenFactoryError;
use crate::state::{get_token_bytecode, State};
use candid::{candid_method, Nat, Principal};
use common::types::Metadata;
use ic_cdk_macros::*;
use ic_helpers::factory::error::FactoryError;
use ic_helpers::factory::FactoryState;
use ic_storage::IcStorage;

ic_helpers::init_factory_api!(State, crate::state::get_token_bytecode());

#[init]
#[candid_method(init)]
fn init(controller: Principal, ledger_principal: Option<Principal>) {
    State::new(controller, ledger_principal).set_global_to_self();
}

/// Returns the token, or None if it does not exist.
#[query(name = "get_token")]
#[candid_method(query, rename = "get_token")]
async fn get_token(name: String) -> Option<Principal> {
    State::get().borrow().factory.get(&name)
}

#[update(name = "set_token_bytecode")]
#[candid_method(update, rename = "set_token_bytecode")]
async fn set_token_bytecode(bytecode: Vec<u8>) {
    if State::get().borrow().token_wasm.is_some() {
        ic_cdk::api::call::reject("token bytecode is already set");
        return;
    }

    let state = State::get();

    state.borrow_mut().token_wasm.replace(bytecode);
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
#[update(name = "create_token")]
#[candid_method(update, rename = "create_token")]
pub async fn create_token(
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

    let state = State::get();
    let key = info.name.clone();

    if state.borrow().factory.get(&key).is_some() {
        return Err(TokenFactoryError::AlreadyExists);
    }

    let caller = owner.unwrap_or_else(ic_cdk::api::caller);
    let actor = state.borrow().consume_provided_cycles_or_icp(caller);
    let cycles = actor.await?;

    let create_token =
        state
            .borrow()
            .factory
            .create_with_cycles(get_token_bytecode(), (info,), cycles);

    let canister = create_token
        .await
        .map_err(|e| TokenFactoryError::CanisterCreateFailed(e.1))?;
    let principal = canister.identity();
    state.borrow_mut().factory.register(key, canister);

    Ok(principal)
}
