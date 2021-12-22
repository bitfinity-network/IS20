use crate::api::dip20_meta::{get_metadata, history_size};
use crate::state::State;
use crate::types::Timestamp;
use crate::types::TokenInfo;
use candid::{candid_method, Nat};
use common::types::Metadata;
use ic_cdk_macros::*;
use ic_kit::{ic, Principal};
use std::iter::FromIterator;

mod dip20_meta;
mod dip20_transactions;
pub mod is20_auction;
mod is20_management;
mod is20_notify;

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

#[query(name = "getTokenInfo")]
#[candid_method(query, rename = "getTokenInfo")]
fn get_token_info() -> TokenInfo {
    let stats = State::get().stats().clone();
    let balance = State::get().balances();

    TokenInfo {
        metadata: get_metadata(),
        feeTo: stats.fee_to,
        historySize: history_size(),
        deployTime: stats.deploy_time,
        holderNumber: balance.len(),
        cycles: ic::balance(),
    }
}

#[query(name = "getHolders")]
#[candid_method(query, rename = "getHolders")]
fn get_holders(start: usize, limit: usize) -> Vec<(Principal, Nat)> {
    let mut balance = Vec::new();
    for (k, v) in State::get().balances() {
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
    let allowances = State::get().allowances();
    for (_, v) in allowances.iter() {
        size += v.len();
    }
    size
}

#[query(name = "getUserApprovals")]
#[candid_method(query, rename = "getUserApprovals")]
fn get_user_approvals(who: Principal) -> Vec<(Principal, Nat)> {
    let allowances = State::get().allowances();
    match allowances.get(&who) {
        Some(allow) => Vec::from_iter(allow.clone().into_iter()),
        None => Vec::new(),
    }
}

#[pre_upgrade]
fn pre_upgrade() {
    State::get().store();
}

#[post_upgrade]
fn post_upgrade() {
    State::load();
}
