//! This file contains api methods, that are part of IS20 additions to the DIP20 standard,
//! and concern token canister management.

use crate::state::State;
use candid::{candid_method, Principal};
use ic_cdk_macros::query;

#[query(name = "owner")]
#[candid_method(query, rename = "owner")]
fn owner() -> Principal {
    let stats = State::get().stats();
    stats.owner
}
