use super::TokenCanister;
use crate::canister::is20_auction::auction_principal;
use crate::state::{CanisterState, Balances};
use crate::types::{TxError, TxReceipt};
use candid::Nat;
use ic_cdk::export::Principal;
use std::collections::HashMap;

pub fn transfer(
    canister: &TokenCanister,
    to: Principal,
    value: Nat,
    fee_limit: Option<Nat>,
) -> TxReceipt {
    let from = ic_kit::ic::caller();
    let (fee, fee_to) = canister.state.borrow().state.stats.fee_info();
    if let Some(fee_limit) = fee_limit {
        if fee > fee_limit {
            return Err(TxError::FeeExceededLimit);
        }
    }

    let fee_ratio = canister.state.borrow().bidding_state.fee_ratio;

    {
        let balances = &mut canister.state.borrow_mut().balances;

        if balances.balance_of(&from) < value.clone() + fee.clone() {
            return Err(TxError::InsufficientBalance);
        }

        _charge_fee(balances, from, fee_to, fee.clone(), fee_ratio);
        _transfer(balances, from, to, value.clone());
    }

    let state = &mut canister.state.borrow_mut().state;
    let id = state.ledger_mut().transfer(from, to, value, fee);
    state.notifications.insert(id.clone());
    Ok(id)
}

pub fn transfer_from(
    canister: &TokenCanister,
    from: Principal,
    to: Principal,
    value: Nat,
) -> TxReceipt {
    let owner = ic_kit::ic::caller();
    let mut state = canister.state.borrow_mut();
    let CanisterState {
        ref mut state,
        ref mut balances,
        ref bidding_state,
        ..
    } = &mut *state;

    let from_allowance = state.allowance(from, owner);
    let (fee, fee_to) = state.stats.fee_info();
    let fee_ratio = bidding_state.fee_ratio;

    let value_with_fee = value.clone() + fee.clone();
    if from_allowance < value_with_fee {
        return Err(TxError::InsufficientAllowance);
    }

    {
        let from_balance = balances.balance_of(&from);
        if from_balance < value_with_fee {
            return Err(TxError::InsufficientBalance);
        }

        _charge_fee(balances, from, fee_to, fee.clone(), fee_ratio);
        _transfer(balances, from, to, value.clone());
    }

    let allowances = state.allowances_mut();
    match allowances.get(&from) {
        Some(inner) => {
            let result = inner.get(&owner).unwrap().clone();
            let mut temp = inner.clone();
            if result.clone() - value_with_fee.clone() != 0 {
                temp.insert(owner, result - value_with_fee);
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

pub fn approve(canister: &TokenCanister, spender: Principal, value: Nat) -> TxReceipt {
    let owner = ic_kit::ic::caller();
    let mut state = canister.state.borrow_mut();

    let CanisterState {
        ref mut state,
        ref mut bidding_state,
        ref mut balances,
        ..
    } = &mut *state;
        
    let (fee, fee_to) = state.stats.fee_info();
    let fee_ratio = bidding_state.fee_ratio;
    if balances.balance_of(&owner) < fee {
        return Err(TxError::InsufficientBalance);
    }

    _charge_fee(
        balances,
        owner,
        fee_to,
        fee.clone(),
        fee_ratio,
    );
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

pub fn mint(canister: &TokenCanister, to: Principal, amount: Nat) -> TxReceipt {
    let caller = ic_kit::ic::caller();
    {
        let balances = &mut canister.state.borrow_mut().balances;
        let to_balance = balances.balance_of(&to);
        balances.0.insert(to, to_balance + amount.clone());
    }

    let state = &mut canister.state.borrow_mut().state;
    state.stats.total_supply += amount.clone();
    let id = state.ledger_mut().mint(caller, to, amount);

    Ok(id)
}

pub fn burn(canister: &TokenCanister, amount: Nat) -> TxReceipt {
    let caller = ic_kit::ic::caller();
    {
        let mut state = canister.state.borrow_mut();
        let caller_balance = state.balances.balance_of(&caller);
        if caller_balance < amount {
            return Err(TxError::InsufficientBalance);
        }

        state.balances.0.insert(caller, caller_balance - amount.clone());
    }

    let state = &mut canister.state.borrow_mut().state;
    state.stats.total_supply -= amount.clone();

    let id = state.ledger_mut().burn(caller, amount);
    Ok(id)
}

pub fn _transfer(balances: &mut Balances, from: Principal, to: Principal, value: Nat) {
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

pub fn _charge_fee(
    balances: &mut Balances,
    user: Principal,
    fee_to: Principal,
    fee: Nat,
    fee_ratio: f64,
) {
    if fee > 0u32 {
        const INT_CONVERSION_K: u64 = 1_000_000_000_000;
        let auction_fee_amount =
            fee.clone() * (fee_ratio * INT_CONVERSION_K as f64) as u64 / INT_CONVERSION_K;
        let owner_fee_amount = fee - auction_fee_amount.clone();
        _transfer(balances, user, fee_to, owner_fee_amount);
        _transfer(balances, user, auction_principal(), auction_fee_amount);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Operation, TransactionStatus};
    use common::types::Metadata;
    use ic_kit::mock_principals::{alice, bob, john};
    use ic_kit::MockContext;
    use std::collections::HashSet;
    use std::iter::FromIterator;

    use crate::canister::MAX_TRANSACTION_QUERY_LEN;
    use ic_canister::Canister;

    fn test_canister() -> TokenCanister {
        MockContext::new().with_caller(alice()).inject();

        let canister = TokenCanister::init_instance();
        canister.init(Metadata {
            logo: "".to_string(),
            name: "".to_string(),
            symbol: "".to_string(),
            decimals: 8,
            totalSupply: Nat::from(1000),
            owner: alice(),
            fee: Nat::from(0),
            feeTo: alice(),
            isTestToken: None,
        });

        canister
    }
    #[test]
    fn transfer_without_fee() {
        let canister = test_canister();
        assert_eq!(Nat::from(1000), canister.balanceOf(alice()));

        assert!(transfer(&canister, bob(), Nat::from(100), None).is_ok());
        assert_eq!(canister.balanceOf(bob()), Nat::from(100));
        assert_eq!(canister.balanceOf(alice()), Nat::from(900));
    }

    #[test]
    fn transfer_with_fee() {
        let canister = test_canister();
        canister.state.borrow_mut().state.stats.fee = Nat::from(100);
        canister.state.borrow_mut().state.stats.fee_to = john();

        assert!(canister.transfer(bob(), Nat::from(200), None).is_ok());
        assert_eq!(canister.balanceOf(bob()), Nat::from(200));
        assert_eq!(canister.balanceOf(alice()), Nat::from(700));
        assert_eq!(canister.balanceOf(john()), Nat::from(100));
    }

    #[test]
    fn transfer_fee_exceeded() {
        let canister = test_canister();
        canister.state.borrow_mut().state.stats.fee = Nat::from(100);
        canister.state.borrow_mut().state.stats.fee_to = john();

        assert!(canister
            .transfer(bob(), Nat::from(200), Some(Nat::from(100)))
            .is_ok());
        assert_eq!(
            canister.transfer(bob(), Nat::from(200), Some(Nat::from(50))),
            Err(TxError::FeeExceededLimit)
        );
    }

    #[test]
    fn fees_with_auction_enabled() {
        let canister = test_canister();
        canister.state.borrow_mut().state.stats.fee = Nat::from(50);
        canister.state.borrow_mut().state.stats.fee_to = john();
        canister.state.borrow_mut().bidding_state.fee_ratio = 0.5;

        canister.transfer(bob(), Nat::from(100), None).unwrap();
        assert_eq!(canister.balanceOf(bob()), Nat::from(100));
        assert_eq!(canister.balanceOf(alice()), Nat::from(850));
        assert_eq!(canister.balanceOf(john()), Nat::from(25));
        assert_eq!(canister.balanceOf(auction_principal()), Nat::from(25));
    }

    #[test]
    fn transfer_insufficient_balance() {
        let canister = test_canister();
        assert_eq!(
            canister.transfer(bob(), Nat::from(1001), None),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice()), Nat::from(1000));
        assert_eq!(canister.balanceOf(bob()), Nat::from(0));
    }

    #[test]
    fn transfer_with_fee_insufficient_balance() {
        let canister = test_canister();
        canister.state.borrow_mut().state.stats.fee = Nat::from(100);
        canister.state.borrow_mut().state.stats.fee_to = john();

        assert_eq!(
            canister.transfer(bob(), Nat::from(950), None),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice()), Nat::from(1000));
        assert_eq!(canister.balanceOf(bob()), Nat::from(0));
    }

    #[test]
    fn transfer_wrong_caller() {
        let canister = test_canister();
        MockContext::new().with_caller(bob()).inject();
        assert_eq!(
            canister.transfer(bob(), Nat::from(100), None),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice()), Nat::from(1000));
        assert_eq!(canister.balanceOf(bob()), Nat::from(0));
    }

    #[test]
    fn transfer_saved_into_history() {
        let canister = test_canister();
        canister.state.borrow_mut().state.stats.fee = Nat::from(10);

        canister.transfer(bob(), Nat::from(1001), None).unwrap_err();
        assert_eq!(canister.historySize(), 1);

        const COUNT: usize = 5;
        let mut ts = ic_kit::ic::time().into();
        for i in 0..COUNT {
            let id = canister.transfer(bob(), Nat::from(100 + i), None).unwrap();
            assert_eq!(canister.historySize(), 2 + i);
            let tx = canister.getTransaction(id);
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
    fn mint_test_token() {
        let canister = test_canister();
        MockContext::new().with_caller(bob()).inject();
        assert_eq!(
            canister.mint(alice(), Nat::from(100)),
            Err(TxError::Unauthorized)
        );

        canister.state.borrow_mut().state.stats.is_test_token = true;

        assert!(canister.mint(alice(), Nat::from(2000)).is_ok());
        assert!(canister.mint(bob(), Nat::from(5000)).is_ok());
        assert_eq!(canister.balanceOf(alice()), Nat::from(3000));
        assert_eq!(canister.balanceOf(bob()), Nat::from(5000));
    }

    #[test]
    fn mint_by_owner() {
        let canister = test_canister();
        assert!(canister.mint(alice(), Nat::from(2000)).is_ok());
        assert!(canister.mint(bob(), Nat::from(5000)).is_ok());
        assert_eq!(canister.balanceOf(alice()), Nat::from(3000));
        assert_eq!(canister.balanceOf(bob()), Nat::from(5000));
        assert_eq!(canister.getMetadata().totalSupply, Nat::from(8000));
    }

    #[test]
    fn mint_saved_into_history() {
        let canister = test_canister();
        canister.state.borrow_mut().state.stats.fee = Nat::from(10);

        assert_eq!(canister.historySize(), 1);

        const COUNT: usize = 5;
        let mut ts = ic_kit::ic::time().into();
        for i in 0..COUNT {
            let id = canister.mint(bob(), Nat::from(100 + i)).unwrap();
            assert_eq!(canister.historySize(), 2 + i);
            let tx = canister.getTransaction(id);
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
        let canister = test_canister();
        assert!(canister.burn(Nat::from(100)).is_ok());
        assert_eq!(canister.balanceOf(alice()), Nat::from(900));
        assert_eq!(canister.getMetadata().totalSupply, Nat::from(900));
    }

    #[test]
    fn burn_too_much() {
        let canister = test_canister();
        assert_eq!(
            canister.burn(Nat::from(1001)),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice()), Nat::from(1000));
        assert_eq!(canister.getMetadata().totalSupply, Nat::from(1000));
    }

    #[test]
    fn burn_by_wrong_user() {
        let canister = test_canister();
        let context = MockContext::new().with_caller(bob()).inject();
        context.update_caller(bob());
        assert_eq!(
            canister.burn(Nat::from(100)),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice()), Nat::from(1000));
        assert_eq!(canister.getMetadata().totalSupply, Nat::from(1000));
    }

    #[test]
    fn burn_saved_into_history() {
        let canister = test_canister();
        canister.state.borrow_mut().state.stats.fee = Nat::from(10);

        canister.burn(Nat::from(1001)).unwrap_err();
        assert_eq!(canister.historySize(), 1);

        const COUNT: usize = 5;
        let mut ts = ic_kit::ic::time().into();
        for i in 0..COUNT {
            let id = canister.burn(Nat::from(100 + i)).unwrap();
            assert_eq!(canister.historySize(), 2 + i);
            let tx = canister.getTransaction(id);
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
        let canister = test_canister();
        let context = MockContext::new().with_caller(alice()).inject();
        assert!(canister.approve(bob(), Nat::from(500)).is_ok());
        context.update_caller(bob());

        assert!(canister
            .transferFrom(alice(), john(), Nat::from(100))
            .is_ok());
        assert_eq!(canister.balanceOf(alice()), Nat::from(900));
        assert_eq!(canister.balanceOf(john()), Nat::from(100));
        assert!(canister
            .transferFrom(alice(), john(), Nat::from(100))
            .is_ok());
        assert_eq!(canister.balanceOf(alice()), Nat::from(800));
        assert_eq!(canister.balanceOf(john()), Nat::from(200));
        assert!(canister
            .transferFrom(alice(), john(), Nat::from(300))
            .is_ok());

        assert_eq!(canister.balanceOf(alice()), Nat::from(500));
        assert_eq!(canister.balanceOf(bob()), Nat::from(0));
        assert_eq!(canister.balanceOf(john()), Nat::from(500));
    }

    #[test]
    fn insufficient_allowance() {
        let canister = test_canister();
        let context = MockContext::new().with_caller(alice()).inject();
        assert!(canister.approve(bob(), Nat::from(500)).is_ok());
        context.update_caller(bob());
        assert_eq!(
            canister.transferFrom(alice(), john(), Nat::from(600)),
            Err(TxError::InsufficientAllowance)
        );
        assert_eq!(canister.balanceOf(alice()), Nat::from(1000));
        assert_eq!(canister.balanceOf(john()), Nat::from(0));
    }

    #[test]
    fn transfer_from_without_approve() {
        let canister = test_canister();
        let context = MockContext::new().with_caller(alice()).inject();
        context.update_caller(bob());
        assert_eq!(
            canister.transferFrom(alice(), john(), Nat::from(600)),
            Err(TxError::InsufficientAllowance)
        );
        assert_eq!(canister.balanceOf(alice()), Nat::from(1000));
        assert_eq!(canister.balanceOf(john()), Nat::from(0));
    }

    #[test]
    fn transfer_from_saved_into_history() {
        let canister = test_canister();
        let context = MockContext::new().with_caller(alice()).inject();
        canister.state.borrow_mut().state.stats.fee = Nat::from(10);

        canister
            .transferFrom(bob(), john(), Nat::from(10))
            .unwrap_err();
        assert_eq!(canister.historySize(), 1);

        canister.approve(bob(), Nat::from(1000)).unwrap();
        context.update_caller(bob());

        const COUNT: usize = 5;
        let mut ts = ic_kit::ic::time().into();
        for i in 0..COUNT {
            let id = canister
                .transferFrom(alice(), john(), Nat::from(100 + i))
                .unwrap();
            assert_eq!(canister.historySize(), 3 + i);
            let tx = canister.getTransaction(id);
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
        let canister = test_canister();
        assert!(canister.approve(bob(), Nat::from(500)).is_ok());
        assert_eq!(
            canister.getUserApprovals(alice()),
            vec![(bob(), Nat::from(500))]
        );

        assert!(canister.approve(bob(), Nat::from(200)).is_ok());
        assert_eq!(
            canister.getUserApprovals(alice()),
            vec![(bob(), Nat::from(200))]
        );

        assert!(canister.approve(john(), Nat::from(1000)).is_ok());

        // Convert vectors to sets before comparing to make comparison unaffected by the element
        // order.
        assert_eq!(
            HashSet::<&(Principal, Nat)>::from_iter(canister.getUserApprovals(alice()).iter()),
            HashSet::from_iter(vec![(bob(), Nat::from(200)), (john(), Nat::from(1000))].iter())
        );
    }

    #[test]
    fn approve_over_balance() {
        let canister = test_canister();
        let context = MockContext::new().with_caller(alice()).inject();
        assert!(canister.approve(bob(), Nat::from(1500)).is_ok());
        context.update_caller(bob());
        assert!(canister
            .transferFrom(alice(), john(), Nat::from(500))
            .is_ok());
        assert_eq!(canister.balanceOf(alice()), Nat::from(500));
        assert_eq!(canister.balanceOf(john()), Nat::from(500));

        assert_eq!(
            canister.transferFrom(alice(), john(), Nat::from(600)),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice()), Nat::from(500));
        assert_eq!(canister.balanceOf(john()), Nat::from(500));
    }

    #[test]
    fn transfer_from_with_fee() {
        let canister = test_canister();
        canister.state.borrow_mut().state.stats.fee = Nat::from(100);
        canister.state.borrow_mut().state.stats.fee_to = bob();
        let context = MockContext::new().with_caller(alice()).inject();

        assert!(canister.approve(bob(), Nat::from(1500)).is_ok());
        assert_eq!(canister.balanceOf(bob()), Nat::from(100));
        context.update_caller(bob());

        assert!(canister
            .transferFrom(alice(), john(), Nat::from(300))
            .is_ok());
        assert_eq!(canister.balanceOf(bob()), Nat::from(200));
        assert_eq!(canister.balanceOf(alice()), Nat::from(500));
        assert_eq!(canister.balanceOf(john()), Nat::from(300));
    }

    #[test]
    fn approve_saved_into_history() {
        let canister = test_canister();
        canister.state.borrow_mut().state.stats.fee = Nat::from(10);
        assert_eq!(canister.historySize(), 1);

        const COUNT: usize = 5;
        let mut ts = ic_kit::ic::time().into();
        for i in 0..COUNT {
            let id = canister.approve(bob(), Nat::from(100 + i)).unwrap();
            assert_eq!(canister.historySize(), 2 + i);
            let tx = canister.getTransaction(id);
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

    #[test]
    fn get_transactions_test() {
        let canister = test_canister();
        const COUNT: usize = 5;
        for _ in 0..COUNT {
            canister.transfer(bob(), Nat::from(10), None).unwrap();
        }

        let txs = canister.getTransactions(Nat::from(0), Nat::from(2));
        assert_eq!(txs.len(), 2);
        assert_eq!(txs[1].index, Nat::from(1));

        let txs = canister.getTransactions(Nat::from(COUNT), Nat::from(2));
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].index, Nat::from(COUNT));
    }

    #[test]
    #[should_panic]
    fn get_transactions_over_limit() {
        let canister = test_canister();
        canister.getTransactions(Nat::from(0), Nat::from(MAX_TRANSACTION_QUERY_LEN + 1));
    }

    #[test]
    #[should_panic]
    fn get_transaction_not_existing() {
        let canister = test_canister();
        canister.getTransaction(Nat::from(2));
    }
}
