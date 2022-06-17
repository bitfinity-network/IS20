#![allow(dead_code)]

mod canister;
mod ledger;
mod principal;
mod state;
mod types;

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    use canister::is20_auction::{AuctionError, BiddingInfo};
    use ic_cdk::export::candid::Principal;
    use ic_helpers::tokens::Tokens128;
    use types::*;

    std::print!("{}", ic_canister::generate_idl!());
}
