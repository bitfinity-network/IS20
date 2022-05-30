#![allow(dead_code)]

mod canister;
mod ledger;
mod state;
mod types;

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    use crate::state::Metrics;
    use canister::is20_auction::{AuctionError, BiddingInfo};
    use common::types::Metadata;
    use ic_cdk::export::candid::{Nat, Principal};
    use types::*;

    std::print!("{}", ic_canister::generate_idl!());
}
