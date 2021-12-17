use crate::api::dip20_meta::{get_metadata, history_size};
use crate::state::State;
use crate::types::TokenInfo;
use candid::{candid_method, Nat};
use ic_cdk_macros::*;
use ic_kit::{ic, Principal};
use std::iter::FromIterator;

mod dip20_meta;
mod dip20_transactions;
pub mod is20_auction;
mod is20_management;
mod is20_notify;

// todo: stats?

// todo: guard against cycle depletion

// todo: setFeeRatio and bidding mechanism

// ******* Methods not from any standard *******

#[query(name = "owner")]
#[candid_method(query)]
fn owner() -> Principal {
    let stats = State::get().stats();
    stats.owner
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
