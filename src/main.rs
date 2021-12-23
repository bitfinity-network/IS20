use crate::state::{AuctionHistory, Balances, BiddingState, State};
use crate::types::{Metadata, Timestamp};
use candid::candid_method;
use ic_cdk_macros::*;
use ic_kit::ic;
use ic_storage::IcStorage;

#[cfg(not(any(target_arch = "wasm32", test)))]
use crate::api::is20_auction::{AuctionError, BiddingInfo};
#[cfg(not(any(target_arch = "wasm32", test)))]
use crate::types::{AuctionInfo, TokenInfo, TxError, TxReceipt, TxRecord};
#[cfg(not(any(target_arch = "wasm32", test)))]
use candid::{Nat, Principal};

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
    let state = State::get();
    let mut state = state.borrow_mut();
    let stats = state.stats_mut();

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

    let bidding_state = BiddingState::get();
    bidding_state.borrow_mut().auction_period = DEFAULT_AUCTION_PERIOD;

    let balances = Balances::get();
    balances.borrow_mut().0.insert(owner, total_supply.clone());

    state.ledger_mut().mint(owner, owner, total_supply);
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
    let state = State::get();
    let balances = Balances::get();
    let bidding_state = BiddingState::get();
    let auction_history = AuctionHistory::get();

    ic_cdk::storage::stable_save((
        &*state.borrow(),
        &*balances.borrow(),
        &*bidding_state.borrow(),
        &*auction_history.borrow(),
    ))
    .unwrap();
}

#[post_upgrade]
fn post_upgrade() {
    let (state, balances, bidding_state, auction_history) =
        ic_cdk::storage::stable_restore().unwrap();
    *State::get().borrow_mut() = state;
    *Balances::get().borrow_mut() = balances;
    *BiddingState::get().borrow_mut() = bidding_state;
    *AuctionHistory::get().borrow_mut() = auction_history;
}
