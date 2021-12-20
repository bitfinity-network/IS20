use crate::state::State;
use crate::types::Timestamp;
use candid::{candid_method, Nat, Principal};
use ic_cdk_macros::*;
use ic_kit::ic;

#[cfg(not(any(target_arch = "wasm32", test)))]
use crate::api::is20_auction::{AuctionError, BiddingInfo};
#[cfg(not(any(target_arch = "wasm32", test)))]
use crate::types::{AuctionInfo, Metadata, TokenInfo, TxError, TxReceipt, TxRecord};

mod api;
mod common;
mod ledger;
mod state;
mod types;

#[cfg(test)]
pub mod tests;

// 10T cycles is an equivalent of approximately $10. This should be enough to last the canister
// for the default auction cycle, which is 1 day.
const DEFAULT_MIN_CYCLES: u64 = 10_000_000_000_000;

// 1 day in nanoseconds.
const DEFAULT_AUCTION_PERIOD: Timestamp = 24 * 60 * 60 * 1_000_000;

#[init]
#[candid_method(init)]
#[allow(clippy::too_many_arguments)]
pub fn init(info: Metadata) {
    let Metadata {
        logo,
        name,
        symbol,
        decimals,
        totalSupply: total_supply,
        owner,
        fee,
        feeTo: fee_to,
    } = info;
    let stats = State::get().stats_mut();

    stats.logo = logo;
    stats.name = name;
    stats.symbol = symbol;
    stats.decimals = decimals;
    stats.total_supply = total_supply.clone();
    stats.owner = owner;
    stats.fee = fee;
    stats.fee_to = fee_to;
    stats.deploy_time = ic::time();
    stats.min_cycles = DEFAULT_MIN_CYCLES;

    State::get().bidding_state_mut().auction_period = DEFAULT_AUCTION_PERIOD;

    let balances = State::get().balances_mut();
    balances.insert(owner, total_supply.clone());

    State::get().ledger_mut().mint(owner, owner, total_supply);
}

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    candid::export_service!();
    std::print!("{}", __export_service());
}

#[pre_upgrade]
fn pre_upgrade() {
    State::get().store();
}

#[post_upgrade]
fn post_upgrade() {
    State::load();
}
