#![allow(dead_code)]

mod canister;
mod core;
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
    use token::canister::TokenCanister;
    use token::core::ISTokenCanister;
    use types::*;

    let trait_idl = <TokenCanister as ISTokenCanister>::get_idl();

    let result = candid::bindings::candid::compile(&trait_idl.env.env, &Some(trait_idl.actor));
    print!("{result}");
}
