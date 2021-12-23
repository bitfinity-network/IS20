//! Module     : factory
//! Copyright  : 2021 InfinitySwap Team
//! Stability  : Experimental

use super::state::State;
use crate::types::TokenKey;
use candid::{candid_method, Principal};
use common::types::Metadata;
use ic_cdk_macros::*;

ic_helpers::init_api!(State);

/// Returns the token, or None if it does not exist.
#[query(name = "get_token")]
#[candid_method(query, rename = "get_token")]
async fn get_token(name: String, symbol: String) -> Option<Principal> {
    State::get().factory.get(&(name, symbol).into())
}

/// Creates a new token.
#[update(name = "create_token")]
#[candid_method(update, rename = "create_token")]
pub async fn create_token(info: Metadata) -> Option<Principal> {
    if info.name.is_empty() == info.symbol.is_empty() {
        return None;
    }

    let key = TokenKey::new(info.name.clone(), info.symbol.clone());
    State::get()
        .factory
        .create(key, State::wasm(), (info,))
        .await
        .ok()
}
