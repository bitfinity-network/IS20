use crate::api::dip20_meta::{allowance, balance_of};
use crate::api::is20_auction::auction_principal;
use crate::state::{Balances, BiddingState, State};
use crate::types::{TxError, TxReceipt};
use crate::utils::check_caller_is_owner;
use candid::{candid_method, Nat};
use ic_cdk_macros::*;
use ic_kit::{ic, Principal};
use ic_storage::IcStorage;
use std::collections::HashMap;

#[update(name = "transfer")]
#[candid_method(update)]
pub fn transfer(to: Principal, value: Nat) -> TxReceipt {
    let from = ic::caller();
    let state = State::get();
    let mut state = state.borrow_mut();
    let stats = state.stats();
    let fee = stats.fee.clone();
    let bidding_state = BiddingState::get();
    let fee_ratio = bidding_state.borrow().fee_ratio;

    if value <= fee {
        return Err(TxError::AmountTooSmall);
    }

    if balance_of(from) < value {
        return Err(TxError::InsufficientBalance);
    }

    _charge_fee(from, stats.fee_to, fee.clone(), fee_ratio);
    _transfer(from, to, value.clone() - fee.clone());

    let id = state.ledger_mut().transfer(from, to, value, fee);
    Ok(id)
}

#[update(name = "transferFrom")]
#[candid_method(update, rename = "transferFrom")]
pub fn transfer_from(from: Principal, to: Principal, value: Nat) -> TxReceipt {
    let owner = ic::caller();
    let from_allowance = allowance(from, owner);
    let state = State::get();
    let mut state = state.borrow_mut();
    let stats = state.stats();
    let fee = stats.fee.clone();
    let bidding_state = BiddingState::get();
    let fee_ratio = bidding_state.borrow().fee_ratio;

    if value < fee {
        return Err(TxError::AmountTooSmall);
    }

    if from_allowance < value {
        return Err(TxError::InsufficientAllowance);
    }

    let from_balance = balance_of(from);
    if from_balance < value {
        return Err(TxError::InsufficientBalance);
    }

    _charge_fee(from, stats.fee_to, fee.clone(), fee_ratio);
    _transfer(from, to, value.clone() - fee.clone());

    let allowances = state.allowances_mut();
    match allowances.get(&from) {
        Some(inner) => {
            let result = inner.get(&owner).unwrap().clone();
            let mut temp = inner.clone();
            if result.clone() - value.clone() != 0 {
                temp.insert(owner, result - value.clone());
                allowances.insert(from, temp);
            } else {
                temp.remove(&owner);
                if temp.is_empty() {
                    allowances.remove(&from);
                } else {
                    allowances.insert(from, temp);
                }
            }
        }
        None => {
            panic!()
        }
    }

    let id = state
        .ledger_mut()
        .transfer_from(owner, from, to, value, fee);
    Ok(id)
}

#[update(name = "approve")]
#[candid_method(update)]
pub fn approve(spender: Principal, value: Nat) -> TxReceipt {
    let owner = ic::caller();
    let state = State::get();
    let mut state = state.borrow_mut();
    let stats = state.stats();
    let fee = stats.fee.clone();
    let bidding_state = BiddingState::get();
    let fee_ratio = bidding_state.borrow().fee_ratio;
    if balance_of(owner) < stats.fee.clone() {
        return Err(TxError::InsufficientBalance);
    }
    _charge_fee(owner, stats.fee_to, fee.clone(), fee_ratio);
    let v = value.clone() + fee.clone();
    let allowances = state.allowances_mut();
    match allowances.get(&owner) {
        Some(inner) => {
            let mut temp = inner.clone();
            if v != 0 {
                temp.insert(spender, v);
                allowances.insert(owner, temp);
            } else {
                temp.remove(&spender);
                if temp.is_empty() {
                    allowances.remove(&owner);
                } else {
                    allowances.insert(owner, temp);
                }
            }
        }
        None => {
            if v != 0 {
                let mut inner = HashMap::new();
                inner.insert(spender, v);
                let allowances = state.allowances_mut();
                allowances.insert(owner, inner);
            }
        }
    }

    let id = state.ledger_mut().approve(owner, spender, value, fee);
    Ok(id)
}

#[update(name = "mint")]
#[candid_method(update, rename = "mint")]
pub fn mint(to: Principal, amount: Nat) -> TxReceipt {
    check_caller_is_owner()?;

    let caller = ic::caller();
    let state = State::get();
    let mut state = state.borrow_mut();
    let stats = state.stats_mut();
    let to_balance = balance_of(to);

    let balances = Balances::get();
    let mut balances = balances.borrow_mut();
    balances.0.insert(to, to_balance + amount.clone());
    stats.total_supply += amount.clone();

    let id = state.ledger_mut().mint(caller, to, amount);
    Ok(id)
}

#[update(name = "burn")]
#[candid_method(update, rename = "burn")]
pub fn burn(amount: Nat) -> TxReceipt {
    let caller = ic::caller();
    let state = State::get();
    let mut state = state.borrow_mut();
    let stats = state.stats_mut();
    let caller_balance = balance_of(caller);
    if caller_balance < amount {
        return Err(TxError::InsufficientBalance);
    }
    let balances = Balances::get();
    balances
        .borrow_mut()
        .0
        .insert(caller, caller_balance - amount.clone());
    stats.total_supply -= amount.clone();

    let id = state.ledger_mut().burn(caller, amount);
    Ok(id)
}

pub fn _transfer(from: Principal, to: Principal, value: Nat) {
    let balances = Balances::get();
    let mut balances = balances.borrow_mut();
    let from_balance = balances.balance_of(&from);
    let from_balance_new = from_balance - value.clone();
    if from_balance_new != 0 {
        balances.0.insert(from, from_balance_new);
    } else {
        balances.0.remove(&from);
    }
    let to_balance = balances.balance_of(&to);
    let to_balance_new = to_balance + value;
    if to_balance_new != 0 {
        balances.0.insert(to, to_balance_new);
    }
}

fn _charge_fee(user: Principal, fee_to: Principal, fee: Nat, fee_ratio: f64) {
    if fee > 0u32 {
        const INT_CONVERSION_K: u64 = 1_000_000_000_000;
        let auction_fee_amount =
            fee.clone() * (fee_ratio * INT_CONVERSION_K as f64) as u64 / INT_CONVERSION_K;
        let owner_fee_amount = fee - auction_fee_amount.clone();
        _transfer(user, fee_to, owner_fee_amount);
        _transfer(user, auction_principal(), auction_fee_amount);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::dip20_meta::{get_metadata, get_transaction, history_size, set_fee};
    use crate::api::get_user_approvals;
    use crate::tests::{canister_init_with_fee, init_context};
    use crate::types::{Operation, TransactionStatus};
    use common::types::Metadata;
    use ic_kit::mock_principals::{alice, bob, john};
    use ic_kit::MockContext;
    use std::collections::HashSet;
    use std::iter::FromIterator;

    #[test]
    fn transfer_without_fee() {
        init_context();
        assert_eq!(Nat::from(1000), balance_of(alice()));

        assert!(transfer(bob(), Nat::from(100)).is_ok());
        assert_eq!(balance_of(bob()), Nat::from(100));
        assert_eq!(balance_of(alice()), Nat::from(900));
    }

    #[test]
    fn transfer_with_fee() {
        MockContext::new().with_caller(alice()).inject();
        canister_init_with_fee();

        assert!(transfer(bob(), Nat::from(200)).is_ok());
        assert_eq!(balance_of(bob()), Nat::from(100));
        assert_eq!(balance_of(alice()), Nat::from(800));
        assert_eq!(balance_of(john()), Nat::from(100));
    }

    #[test]
    fn fees_with_auction_enabled() {
        MockContext::new().with_caller(alice()).inject();

        crate::api::init(Metadata {
            logo: "".to_string(),
            name: "".to_string(),
            symbol: "".to_string(),
            decimals: 8,
            totalSupply: Nat::from(1000),
            owner: alice(),
            fee: Nat::from(50),
            feeTo: john(),
        });

        let bidding_state = BiddingState::get();
        bidding_state.borrow_mut().fee_ratio = 0.5;
        transfer(bob(), Nat::from(100)).unwrap();
        assert_eq!(balance_of(bob()), Nat::from(50));
        assert_eq!(balance_of(alice()), Nat::from(900));
        assert_eq!(balance_of(john()), Nat::from(25));
        assert_eq!(balance_of(auction_principal()), Nat::from(25));
    }

    #[test]
    fn transfer_insufficient_balance() {
        init_context();
        assert_eq!(
            transfer(bob(), Nat::from(1001)),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(balance_of(alice()), Nat::from(1000));
        assert_eq!(balance_of(bob()), Nat::from(0));
    }

    #[test]
    fn transfer_wrong_caller() {
        let context = init_context();
        context.update_caller(bob());
        assert_eq!(
            transfer(bob(), Nat::from(100)),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(balance_of(alice()), Nat::from(1000));
        assert_eq!(balance_of(bob()), Nat::from(0));
    }

    #[test]
    fn transfer_saved_into_history() {
        init_context();
        set_fee(Nat::from(10));

        transfer(bob(), Nat::from(1001)).unwrap_err();
        assert_eq!(history_size(), 1);

        const COUNT: usize = 5;
        let mut ts = ic::time().into();
        for i in 0..COUNT {
            let id = transfer(bob(), Nat::from(100 + i)).unwrap();
            assert_eq!(history_size(), 2 + i);
            let tx = get_transaction(id);
            assert_eq!(tx.amount, Nat::from(100 + i));
            assert_eq!(tx.fee, Nat::from(10));
            assert_eq!(tx.operation, Operation::Transfer);
            assert_eq!(tx.status, TransactionStatus::Succeeded);
            assert_eq!(tx.index, i + 1);
            assert_eq!(tx.from, alice());
            assert_eq!(tx.to, bob());
            assert!(ts < tx.timestamp);
            ts = tx.timestamp;
        }
    }

    #[test]
    fn mint_by_owner() {
        init_context();
        assert!(mint(alice(), Nat::from(2000)).is_ok());
        assert!(mint(bob(), Nat::from(5000)).is_ok());
        assert_eq!(balance_of(alice()), Nat::from(3000));
        assert_eq!(balance_of(bob()), Nat::from(5000));
        assert_eq!(get_metadata().totalSupply, Nat::from(8000));
    }

    #[test]
    fn mint_not_by_owner() {
        let context = init_context();
        context.update_caller(bob());
        assert_eq!(mint(alice(), Nat::from(100)), Err(TxError::Unauthorized));
    }

    #[test]
    fn mint_saved_into_history() {
        init_context();
        set_fee(Nat::from(10));

        assert_eq!(history_size(), 1);

        const COUNT: usize = 5;
        let mut ts = ic::time().into();
        for i in 0..COUNT {
            let id = mint(bob(), Nat::from(100 + i)).unwrap();
            assert_eq!(history_size(), 2 + i);
            let tx = get_transaction(id);
            assert_eq!(tx.amount, Nat::from(100 + i));
            assert_eq!(tx.fee, Nat::from(0));
            assert_eq!(tx.operation, Operation::Mint);
            assert_eq!(tx.status, TransactionStatus::Succeeded);
            assert_eq!(tx.index, i + 1);
            assert_eq!(tx.from, alice());
            assert_eq!(tx.to, bob());
            assert!(ts < tx.timestamp);
            ts = tx.timestamp;
        }
    }

    #[test]
    fn burn_by_owner() {
        init_context();
        assert!(burn(Nat::from(100)).is_ok());
        assert_eq!(balance_of(alice()), Nat::from(900));
        assert_eq!(get_metadata().totalSupply, Nat::from(900));
    }

    #[test]
    fn burn_too_much() {
        init_context();
        assert_eq!(burn(Nat::from(1001)), Err(TxError::InsufficientBalance));
        assert_eq!(balance_of(alice()), Nat::from(1000));
        assert_eq!(get_metadata().totalSupply, Nat::from(1000));
    }

    #[test]
    fn burn_by_wrong_user() {
        let context = init_context();
        context.update_caller(bob());
        assert_eq!(burn(Nat::from(100)), Err(TxError::InsufficientBalance));
        assert_eq!(balance_of(alice()), Nat::from(1000));
        assert_eq!(get_metadata().totalSupply, Nat::from(1000));
    }

    #[test]
    fn burn_saved_into_history() {
        init_context();
        set_fee(Nat::from(10));

        burn(Nat::from(1001)).unwrap_err();
        assert_eq!(history_size(), 1);

        const COUNT: usize = 5;
        let mut ts = ic::time().into();
        for i in 0..COUNT {
            let id = burn(Nat::from(100 + i)).unwrap();
            assert_eq!(history_size(), 2 + i);
            let tx = get_transaction(id);
            assert_eq!(tx.amount, Nat::from(100 + i));
            assert_eq!(tx.fee, Nat::from(0));
            assert_eq!(tx.operation, Operation::Burn);
            assert_eq!(tx.status, TransactionStatus::Succeeded);
            assert_eq!(tx.index, i + 1);
            assert_eq!(tx.from, alice());
            assert_eq!(tx.to, alice());
            assert!(ts < tx.timestamp);
            ts = tx.timestamp;
        }
    }

    #[test]
    fn transfer_from_with_approve() {
        let context = init_context();
        assert!(approve(bob(), Nat::from(500)).is_ok());
        context.update_caller(bob());
        assert!(transfer_from(alice(), john(), Nat::from(100)).is_ok());
        assert_eq!(balance_of(alice()), Nat::from(900));
        assert_eq!(balance_of(john()), Nat::from(100));
        assert!(transfer_from(alice(), john(), Nat::from(100)).is_ok());
        assert_eq!(balance_of(alice()), Nat::from(800));
        assert_eq!(balance_of(john()), Nat::from(200));
        assert!(transfer_from(alice(), john(), Nat::from(300)).is_ok());

        assert_eq!(balance_of(alice()), Nat::from(500));
        assert_eq!(balance_of(bob()), Nat::from(0));
        assert_eq!(balance_of(john()), Nat::from(500));
    }

    #[test]
    fn insufficient_allowance() {
        let context = init_context();
        assert!(approve(bob(), Nat::from(500)).is_ok());
        context.update_caller(bob());
        assert_eq!(
            transfer_from(alice(), john(), Nat::from(600)),
            Err(TxError::InsufficientAllowance)
        );
        assert_eq!(balance_of(alice()), Nat::from(1000));
        assert_eq!(balance_of(john()), Nat::from(0));
    }

    #[test]
    fn transfer_from_without_approve() {
        let context = init_context();
        context.update_caller(bob());
        assert_eq!(
            transfer_from(alice(), john(), Nat::from(600)),
            Err(TxError::InsufficientAllowance)
        );
        assert_eq!(balance_of(alice()), Nat::from(1000));
        assert_eq!(balance_of(john()), Nat::from(0));
    }

    #[test]
    fn transfer_from_saved_into_history() {
        let context = init_context();
        set_fee(Nat::from(10));

        transfer_from(bob(), john(), Nat::from(10)).unwrap_err();
        assert_eq!(history_size(), 1);

        approve(bob(), Nat::from(1000)).unwrap();
        context.update_caller(bob());

        const COUNT: usize = 5;
        let mut ts = ic::time().into();
        for i in 0..COUNT {
            let id = transfer_from(alice(), john(), Nat::from(100 + i)).unwrap();
            assert_eq!(history_size(), 3 + i);
            let tx = get_transaction(id);
            assert_eq!(tx.caller, Some(bob()));
            assert_eq!(tx.amount, Nat::from(100 + i));
            assert_eq!(tx.fee, Nat::from(10));
            assert_eq!(tx.operation, Operation::TransferFrom);
            assert_eq!(tx.status, TransactionStatus::Succeeded);
            assert_eq!(tx.index, i + 2);
            assert_eq!(tx.from, alice());
            assert_eq!(tx.to, john());
            assert!(ts < tx.timestamp);
            ts = tx.timestamp;
        }
    }

    #[test]
    fn multiple_approves() {
        init_context();
        assert!(approve(bob(), Nat::from(500)).is_ok());
        assert_eq!(get_user_approvals(alice()), vec![(bob(), Nat::from(500))]);

        assert!(approve(bob(), Nat::from(200)).is_ok());
        assert_eq!(get_user_approvals(alice()), vec![(bob(), Nat::from(200))]);

        assert!(approve(john(), Nat::from(1000)).is_ok());

        // Convert vectors to sets before comparing to make comparison unaffected by the element
        // order.
        assert_eq!(
            HashSet::<&(Principal, Nat)>::from_iter(get_user_approvals(alice()).iter()),
            HashSet::from_iter(vec![(bob(), Nat::from(200)), (john(), Nat::from(1000))].iter())
        );
    }

    #[test]
    fn approve_over_balance() {
        let context = init_context();
        assert!(approve(bob(), Nat::from(1500)).is_ok());
        context.update_caller(bob());
        assert!(transfer_from(alice(), john(), Nat::from(500)).is_ok());
        assert_eq!(balance_of(alice()), Nat::from(500));
        assert_eq!(balance_of(john()), Nat::from(500));

        assert_eq!(
            transfer_from(alice(), john(), Nat::from(600)),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(balance_of(alice()), Nat::from(500));
        assert_eq!(balance_of(john()), Nat::from(500));
    }

    #[test]
    fn transfer_from_with_fee() {
        let context = MockContext::new().with_caller(alice()).inject();

        crate::api::init(Metadata {
            logo: "".to_string(),
            name: "".to_string(),
            symbol: "".to_string(),
            decimals: 8,
            totalSupply: Nat::from(1000),
            owner: alice(),
            fee: Nat::from(100),
            feeTo: bob(),
        });
        assert!(approve(bob(), Nat::from(1500)).is_ok());
        assert_eq!(balance_of(bob()), Nat::from(100));
        context.update_caller(bob());

        assert!(transfer_from(alice(), john(), Nat::from(300)).is_ok());
        assert_eq!(balance_of(bob()), Nat::from(200));
        assert_eq!(balance_of(alice()), Nat::from(600));
        assert_eq!(balance_of(john()), Nat::from(200));
    }

    #[test]
    fn approve_saved_into_history() {
        init_context();
        assert_eq!(history_size(), 1);
        set_fee(Nat::from(10));

        const COUNT: usize = 5;
        let mut ts = ic::time().into();
        for i in 0..COUNT {
            let id = approve(bob(), Nat::from(100 + i)).unwrap();
            assert_eq!(history_size(), 2 + i);
            let tx = get_transaction(id);
            assert_eq!(tx.amount, Nat::from(100 + i));
            assert_eq!(tx.fee, Nat::from(10));
            assert_eq!(tx.operation, Operation::Approve);
            assert_eq!(tx.status, TransactionStatus::Succeeded);
            assert_eq!(tx.index, i + 1);
            assert_eq!(tx.from, alice());
            assert_eq!(tx.to, bob());
            assert!(ts < tx.timestamp);
            ts = tx.timestamp;
        }
    }
}
