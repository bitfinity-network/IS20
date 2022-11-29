#![cfg_attr(coverage_nightly, feature(no_coverage))]
pub mod canister;

/// This is a marker added to the token wasm to distinguish it from other canisters
#[cfg(feature = "export-api")]
#[no_mangle]
pub static TOKEN_CANISTER_MARKER: &str = "IS20_TOKEN_CANISTER";

pub fn idl() -> String {
    use crate::canister::TokenCanister;
    use canister_sdk::{ic_auction::api::Auction, ic_canister::Idl, ic_helpers::tokens::Tokens128};
    use token_api::canister::TokenCanisterAPI;
    use token_api::state::config::Metadata;

    let canister_idl = canister_sdk::ic_canister::generate_idl!();
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
        let idl = idl();
        let methods = [
            "icrc1_balance_of",
            "decimals",
            "get_holders",
            "get_token_info",
            "get_transaction",
            "get_transactions",
            "get_user_transaction_count",
            "history_size",
            "icrc1_name",
            "owner",
            "icrc1_symbol",
            "icrc1_total_supply",
            "is_test_token",
            "set_fee",
            "set_fee_to",
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
