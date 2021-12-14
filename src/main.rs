use crate::state::State;
use crate::types::{Metadata, Operation, TokenInfo, TransactionStatus, TxReceipt, TxRecord};
use candid::{candid_method, Nat, Principal};
use ic_cdk_macros::*;
use ic_kit::ic;

mod api;
mod ledger;
mod state;
mod types;

#[init]
#[candid_method(init)]
// todo: This should be refactored to use a struct
#[allow(clippy::too_many_arguments)]
fn init(
    logo: String,
    name: String,
    symbol: String,
    decimals: u8,
    total_supply: Nat,
    owner: Principal,
    fee: Nat,
    fee_to: Principal,
) {
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
    let balances = State::get().balances_mut();
    balances.insert(owner, total_supply);
    let ledger = State::get().ledger_mut();
    ledger.push(TxRecord {
        caller: Some(owner),
        index: Default::default(),
        from: owner,
        to: owner,
        amount: Default::default(),
        fee: Nat::from(0),
        timestamp: Default::default(),
        status: TransactionStatus::Succeeded,
        operation: Operation::Approve,
    });
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
