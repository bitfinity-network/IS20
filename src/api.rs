use crate::api::dip20_meta::{get_metadata, history_size};
use crate::state::{Balances, State};
use crate::types::TokenInfo;
use candid::{candid_method, Nat};
use ic_cdk_macros::*;
use ic_kit::{ic, Principal};
use ic_storage::IcStorage;
use std::iter::FromIterator;

mod dip20_meta;
mod dip20_transactions;
pub mod is20_auction;
mod is20_management;
mod is20_notify;

// This methods are not part of the standard and are added for convenience. They may be removed
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
