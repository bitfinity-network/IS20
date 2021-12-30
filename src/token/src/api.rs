use crate::api::dip20_meta::{get_metadata, history_size};
use crate::state::{AuctionHistory, Balances, BiddingState, State};
use crate::types::{Timestamp, TokenInfo};
use candid::{candid_method, Nat};
use common::types::Metadata;
use ic_cdk_macros::*;
use ic_kit::{ic, Principal};
use ic_storage::IcStorage;
use std::iter::FromIterator;

mod dip20_meta;
mod dip20_transactions;
mod inspect;
pub mod is20_auction;
mod is20_management;
mod is20_notify;
mod is20_transactions;

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

// These methods are not part of the standard and are added for convenience. They may be removed
// in future.

#[query(name = "getTokenInfo")]
#[candid_method(query, rename = "getTokenInfo")]
fn get_token_info() -> TokenInfo {
    let state = State::get();
    let state = state.borrow();
    let stats = state.stats();
    let balances = Balances::get();
    let balances = balances.borrow();

    TokenInfo {
        metadata: get_metadata(),
        feeTo: stats.fee_to,
        historySize: history_size(),
        deployTime: stats.deploy_time,
        holderNumber: balances.0.len(),
        cycles: ic::balance(),
    }
}

#[query(name = "getHolders")]
#[candid_method(query, rename = "getHolders")]
fn get_holders(start: usize, limit: usize) -> Vec<(Principal, Nat)> {
    let mut balance = Vec::new();
    let balances = Balances::get();
    let balances = balances.borrow();
    for (k, v) in &balances.0 {
        balance.push((*k, v.clone()));
    }
    balance.sort_by(|a, b| b.1.cmp(&a.1));
    let limit: usize = if start + limit > balance.len() {
        balance.len() - start
    } else {
        limit
    };
    balance[start..start + limit].to_vec()
}

#[query(name = "getAllowanceSize")]
#[candid_method(query, rename = "getAllowanceSize")]
fn get_allowance_size() -> usize {
    let mut size = 0;
    let state = State::get();
    let state = state.borrow();
    let allowances = state.allowances();
    for (_, v) in allowances.iter() {
        size += v.len();
    }
    size
}

#[query(name = "getUserApprovals")]
#[candid_method(query, rename = "getUserApprovals")]
fn get_user_approvals(who: Principal) -> Vec<(Principal, Nat)> {
    let state = State::get();
    let state = state.borrow();
    let allowances = state.allowances();
    match allowances.get(&who) {
        Some(allow) => Vec::from_iter(allow.clone().into_iter()),
        None => Vec::new(),
    }
}
