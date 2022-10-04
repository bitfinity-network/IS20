#![allow(dead_code)]
#![cfg_attr(coverage_nightly, feature(no_coverage))]

use canister_sdk::ic_canister;

mod canister;

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    let result = get_canister_idl();
    print!("{result}");
}

fn get_canister_idl() -> String {
    use crate::canister::TokenCanister;
    use canister_sdk::{
        ic_auction::api::Auction,
        ic_canister::Idl,
        ic_helpers::{candid_header::CandidHeader, tokens::Tokens128},
    };
    use token_api::canister::TokenCanisterAPI;
    use token_api::state::stats::Metadata;

    let canister_idl = ic_canister::generate_idl!();
    let auction_idl = <TokenCanister as Auction>::get_idl();
    let mut trait_idl = <TokenCanister as TokenCanisterAPI>::get_idl();
    trait_idl.merge(&canister_idl);
    trait_idl.merge(&auction_idl);

    candid::bindings::candid::compile(&trait_idl.env.env, &Some(trait_idl.actor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use coverage_helper::test;

    #[test]
    fn generated_idl_contains_all_methods() {
        let idl = get_canister_idl();
        let methods = [
            "icrc1_balance_of",
            "decimals",
            "get_holders",
            "get_token_info",
            "get_transaction",
            "get_transactions",
            "get_user_transaction_count",
            "history_size",
            "logo",
            "icrc1_name",
            "owner",
            "icrc1_symbol",
            "icrc1_total_supply",
            "is_test_token",
            "set_fee",
            "set_fee_to",
            "set_logo",
            "set_name",
            "set_symbol",
            "set_owner",
            "mint",
            "burn",
            "bid_cycles",
            "run_auction",
            "bidding_info",
            "auction_info",
            "set_auction_period",
            "set_controller",
            "set_min_cycles",
        ];

        for method in methods {
            assert!(
                idl.contains(method),
                "IDL string doesn't contain method \"{method}\"\nidl: {}",
                idl
            );
        }
    }
}
