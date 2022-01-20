//! Module     : factory
//! Copyright  : 2021 InfinitySwap Team
//! Stability  : Experimental

use crate::state::{get_token_bytecode, State};
use candid::{candid_method, Principal};
use common::types::Metadata;
use ic_cdk_macros::*;
use ic_storage::IcStorage;

ic_helpers::init_factory_api!(State, crate::state::get_token_bytecode());

/// Returns the token, or None if it does not exist.
#[query(name = "get_token")]
#[candid_method(query, rename = "get_token")]
async fn get_token(name: String) -> Option<Principal> {
    State::get().borrow().factory.get(&name)
}

/// Creates a new token.
#[update(name = "create_token")]
#[candid_method(update, rename = "create_token")]
pub async fn create_token(info: Metadata) -> Option<Principal> {
    if info.name.is_empty() || info.symbol.is_empty() {
        return None;
    }

    let state = State::get();
    let key = info.name.clone();

    let create_token = {
        let state = state.borrow();
        let factory = &state.factory;
        if let Some(existing) = factory.get(&key) {
            return Some(existing);
        }

        factory.create(get_token_bytecode(), (info,))
    };

    let canister = create_token.await.ok()?;
    let principal = canister.identity();
    state.borrow_mut().factory.register(key, canister);

    Some(principal)
}
