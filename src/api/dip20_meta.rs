use crate::state::State;
use crate::types::Metadata;
use candid::{candid_method, Nat};
use ic_cdk_macros::*;
use ic_kit::{ic, Principal};
use std::string::String;

#[query(name = "name")]
#[candid_method(query)]
fn name() -> String {
    let stats = State::get().stats();
    stats.name.clone()
}

#[query(name = "symbol")]
#[candid_method(query)]
fn symbol() -> String {
    let stats = State::get().stats();
    stats.symbol.clone()
}

#[query(name = "decimals")]
#[candid_method(query)]
fn decimals() -> u8 {
    let stats = State::get().stats();
    stats.decimals
}

#[query(name = "totalSupply")]
#[candid_method(query, rename = "totalSupply")]
fn total_supply() -> Nat {
    let stats = State::get().stats();
    stats.total_supply.clone()
}

#[query(name = "balanceOf")]
#[candid_method(query, rename = "balanceOf")]
pub fn balance_of(id: Principal) -> Nat {
    let balances = State::get().balances();
    match balances.get(&id) {
        Some(balance) => balance.clone(),
        None => Nat::from(0),
    }
}

#[query(name = "allowance")]
#[candid_method(query)]
pub fn allowance(owner: Principal, spender: Principal) -> Nat {
    let allowances = State::get().allowances();
    match allowances.get(&owner) {
        Some(inner) => match inner.get(&spender) {
            Some(value) => value.clone(),
            None => Nat::from(0),
        },
        None => Nat::from(0),
    }
}

#[query(name = "getMetadata")]
#[candid_method(query, rename = "getMetadata")]
pub fn get_metadata() -> Metadata {
    let s = State::get().stats();
    Metadata {
        logo: s.logo.clone(),
        name: s.name.clone(),
        symbol: s.symbol.clone(),
        decimals: s.decimals,
        totalSupply: s.total_supply.clone(),
        owner: s.owner,
        fee: s.fee.clone(),
    }
}

#[query(name = "historySize")]
#[candid_method(query, rename = "historySize")]
pub fn history_size() -> usize {
    let ledger = State::get().ledger();
    ledger.len()
}

// todo: getTransaction

// todo: getTransactions

#[query(name = "logo")]
#[candid_method(query, rename = "logo")]
fn get_logo() -> String {
    let stats = State::get().stats();
    stats.logo.clone()
}

#[update(name = "setName")]
#[candid_method(update, rename = "setName")]
fn set_name(name: String) {
    let stats = State::get().stats_mut();
    assert_eq!(ic::caller(), stats.owner);
    stats.name = name;
}

#[update(name = "setLogo")]
#[candid_method(update, rename = "setLogo")]
fn set_logo(logo: String) {
    let stats = State::get().stats_mut();
    assert_eq!(ic::caller(), stats.owner);
    stats.logo = logo;
}

#[update(name = "setFee")]
#[candid_method(update, rename = "setFee")]
fn set_fee(fee: Nat) {
    let stats = State::get().stats_mut();
    assert_eq!(ic::caller(), stats.owner);
    stats.fee = fee;
}

#[update(name = "setFeeTo")]
#[candid_method(update, rename = "setFeeTo")]
fn set_fee_to(fee_to: Principal) {
    let stats = State::get().stats_mut();
    assert_eq!(ic::caller(), stats.owner);
    stats.fee_to = fee_to;
}

#[update(name = "setOwner")]
#[candid_method(update, rename = "setOwner")]
fn set_owner(owner: Principal) {
    let stats = State::get().stats_mut();
    assert_eq!(ic::caller(), stats.owner);
    stats.owner = owner;
}
