//! This file contains api methods, that are part of IS20 additions to the DIP20 standard,
//! and concern token canister management.

use crate::state::State;
use candid::{candid_method, Principal};
use ic_cdk_macros::query;
use ic_storage::IcStorage;

#[query(name = "owner")]
#[candid_method(query, rename = "owner")]
fn owner() -> Principal {
    let state = State::get();
    let state = state.borrow();
    state.stats().owner
}
