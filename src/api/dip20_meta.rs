use crate::common::check_caller_is_owner;
use crate::state::State;
use crate::types::{Metadata, TxRecord};
use candid::{candid_method, Nat};
use ic_cdk_macros::*;
use ic_kit::{ic, Principal};
use num_traits::cast::ToPrimitive;
use std::string::String;

const MAX_TRANSACTION_QUERY_LEN: usize = 1000;

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
        feeTo: s.fee_to,
    }
}

#[query(name = "historySize")]
#[candid_method(query, rename = "historySize")]
pub fn history_size() -> usize {
    let ledger = State::get().ledger();
    ledger.len()
}

#[query(name = "getTransaction")]
#[candid_method(query, rename = "getTransaction")]
pub fn get_transaction(id: Nat) -> TxRecord {
    let id =
        id.0.to_usize()
            .unwrap_or_else(|| ic::trap("Id is out of bounds"));
    State::get()
        .ledger()
        .0
        .get(id)
        .unwrap_or_else(|| ic::trap(&format!("Transaction {} does not exist", id)))
        .clone()
}

#[query(name = "getTransactions")]
#[candid_method(query, rename = "getTransactions")]
pub fn get_transactions(start: Nat, limit: Nat) -> Vec<TxRecord> {
    let start = start
        .0
        .to_usize()
        .unwrap_or_else(|| ic::trap("Start is out of bounds"));
    let limit = limit
        .0
        .to_usize()
        .unwrap_or_else(|| ic::trap("Limit is out of bounds"));
    if limit > MAX_TRANSACTION_QUERY_LEN {
        ic::trap(&format!(
            "Limit must be less then {}",
            MAX_TRANSACTION_QUERY_LEN
        ));
    }

    let ledger = State::get().ledger();
    ledger.0[start..(start + limit).min(ledger.len())].to_vec()
}

#[query(name = "logo")]
#[candid_method(query, rename = "logo")]
fn get_logo() -> String {
    let stats = State::get().stats();
    stats.logo.clone()
}

#[update(name = "setName")]
#[candid_method(update, rename = "setName")]
fn set_name(name: String) {
    check_caller_is_owner().unwrap();
    let stats = State::get().stats_mut();
    stats.name = name;
}

#[update(name = "setLogo")]
#[candid_method(update, rename = "setLogo")]
fn set_logo(logo: String) {
    check_caller_is_owner().unwrap();
    let stats = State::get().stats_mut();
    stats.logo = logo;
}

#[update(name = "setFee")]
#[candid_method(update, rename = "setFee")]
pub fn set_fee(fee: Nat) {
    check_caller_is_owner().unwrap();
    let stats = State::get().stats_mut();
    stats.fee = fee;
}

#[update(name = "setFeeTo")]
#[candid_method(update, rename = "setFeeTo")]
fn set_fee_to(fee_to: Principal) {
    check_caller_is_owner().unwrap();
    let stats = State::get().stats_mut();
    stats.fee_to = fee_to;
}

#[update(name = "setOwner")]
#[candid_method(update, rename = "setOwner")]
fn set_owner(owner: Principal) {
    check_caller_is_owner().unwrap();
    let stats = State::get().stats_mut();
    stats.owner = owner;
}

/// Returns an array of transaction records in range [start, start + limit) related to user `who`.
/// Unlike `getTransactions` function, the range [start, start + limit) for `getUserTransactions`
/// is not the global range of all transactions. The range [start, start + limit) here pertains to
/// the transactions of user who. Implementations are allowed to return less TxRecords than
/// requested to fend off DoS attacks.
#[query(name = "getUserTransactions")]
#[candid_method(query, rename = "getUserTransactions")]
fn get_user_transactions(who: Principal, start: Nat, limit: Nat) -> Vec<TxRecord> {
    let mut transactions = vec![];

    let start = start.0.to_usize().unwrap_or(usize::MAX);
    let limit = limit.0.to_usize().unwrap_or(usize::MAX);
    if limit > MAX_TRANSACTION_QUERY_LEN {
        ic::trap(&format!(
            "Limit must be less then {}",
            MAX_TRANSACTION_QUERY_LEN
        ));
    }

    for tx in State::get().ledger().0.iter().skip(start).take(limit) {
        if tx.from == who || tx.to == who || tx.caller == Some(who) {
            transactions.push(tx.clone());
        }
    }

    transactions
}

/// Returns total number of transactions related to the user `who`.
#[query(name = "getUserTransactionAmount")]
#[candid_method(query, rename = "getUserTransactionAmount")]
fn get_user_transaction_amount(who: Principal) -> Nat {
    let mut amount = Nat::from(0);
    for tx in &State::get().ledger().0 {
        if tx.from == who || tx.to == who || tx.caller == Some(who) {
            amount += tx.amount.clone();
        }
    }

    amount
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::dip20_transactions::transfer;
    use crate::tests::init_context;
    use ic_kit::mock_principals::bob;

    #[test]
    fn get_transactions_test() {
        init_context();
        const COUNT: usize = 5;
        for _ in 0..COUNT {
            transfer(bob(), Nat::from(10)).unwrap();
        }

        let txs = get_transactions(Nat::from(0), Nat::from(2));
        assert_eq!(txs.len(), 2);
        assert_eq!(txs[1].index, Nat::from(1));

        let txs = get_transactions(Nat::from(COUNT), Nat::from(2));
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].index, Nat::from(COUNT));
    }

    #[test]
    #[should_panic]
    fn get_transactions_over_limit() {
        init_context();
        get_transactions(Nat::from(0), Nat::from(MAX_TRANSACTION_QUERY_LEN + 1));
    }

    #[test]
    #[should_panic]
    fn get_transaction_not_existing() {
        init_context();
        get_transaction(Nat::from(2));
    }
}
