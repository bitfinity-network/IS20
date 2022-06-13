use std::collections::HashMap;

use candid::Nat;
use ic_cdk::export::Principal;

use crate::canister::is20_auction::auction_principal;
use crate::principal::{CheckedPrincipal, Owner, TestNet, WithRecipient};
use crate::state::{Balances, CanisterState};
use crate::types::{TxError, TxReceipt};

use super::TokenCanister;

pub fn transfer(
    canister: &TokenCanister,
    caller: CheckedPrincipal<WithRecipient>,
    value: Nat,
    fee_limit: Option<Nat>,
) -> TxReceipt {
    let CanisterState {
        ref mut balances,
        ref mut ledger,
        ref stats,
        ref bidding_state,
        ..
    } = *canister.state.borrow_mut();

    let (fee, fee_to) = stats.fee_info();
    let fee_ratio = bidding_state.fee_ratio;

    if let Some(fee_limit) = fee_limit {
        if fee > fee_limit {
            return Err(TxError::FeeExceededLimit);
        }
    }

    if balances.balance_of(&caller.inner()) < value.clone() + fee.clone() {
        return Err(TxError::InsufficientBalance);
    }

    _charge_fee(balances, caller.inner(), fee_to, fee.clone(), fee_ratio);
    _transfer(balances, caller.inner(), caller.recipient(), value.clone());

    let id = ledger.transfer(caller.inner(), caller.recipient(), value, fee);
    Ok(id)
}

pub fn transfer_from(
    canister: &TokenCanister,
    caller: CheckedPrincipal<WithRecipient>,
    from: Principal,
    value: Nat,
) -> TxReceipt {
    let mut state = canister.state.borrow_mut();
    let from_allowance = state.allowance(from, caller.inner());
    let CanisterState {
        ref mut balances,
        ref bidding_state,
        ref stats,
        ..
    } = &mut *state;

    let (fee, fee_to) = stats.fee_info();
    let fee_ratio = bidding_state.fee_ratio;

    let value_with_fee = value.clone() + fee.clone();
    if from_allowance < value_with_fee {
        return Err(TxError::InsufficientAllowance);
    }

    let from_balance = balances.balance_of(&from);
    if from_balance < value_with_fee {
        return Err(TxError::InsufficientBalance);
    }

    _charge_fee(balances, from, fee_to, fee.clone(), fee_ratio);
    _transfer(balances, from, caller.recipient(), value.clone());

    let allowances = &mut state.allowances;
    match allowances.get(&from) {
        Some(inner) => {
            let result = inner.get(&caller.inner()).unwrap().clone();
            let mut temp = inner.clone();
            if result.clone() - value_with_fee.clone() != 0 {
                temp.insert(caller.inner(), result - value_with_fee);
                allowances.insert(from, temp);
            } else {
                temp.remove(&caller.inner());
                if temp.is_empty() {
                    allowances.remove(&from);
                } else {
                    allowances.insert(from, temp);
                }
            }
        }
        None => panic!(),
    }

    let id = state
        .ledger
        .transfer_from(caller.inner(), from, caller.recipient(), value, fee);
    Ok(id)
}

pub fn approve(
    canister: &TokenCanister,
    caller: CheckedPrincipal<WithRecipient>,
    value: Nat,
) -> TxReceipt {
    let mut state = canister.state.borrow_mut();

    let CanisterState {
        ref mut bidding_state,
        ref mut balances,
        ref stats,
        ..
    } = &mut *state;

    let (fee, fee_to) = stats.fee_info();
    let fee_ratio = bidding_state.fee_ratio;
    if balances.balance_of(&caller.inner()) < fee {
        return Err(TxError::InsufficientBalance);
    }

    _charge_fee(balances, caller.inner(), fee_to, fee.clone(), fee_ratio);
    let v = value.clone() + fee.clone();

    match state.allowances.get(&caller.inner()) {
        Some(inner) => {
            let mut temp = inner.clone();
            if v != 0 {
                temp.insert(caller.recipient(), v);
                state.allowances.insert(caller.inner(), temp);
            } else {
                temp.remove(&caller.recipient());
                if temp.is_empty() {
                    state.allowances.remove(&caller.inner());
                } else {
                    state.allowances.insert(caller.inner(), temp);
                }
            }
        }
        None if v != 0 => {
            let mut inner = HashMap::new();
            inner.insert(caller.recipient(), v);
            state.allowances.insert(caller.inner(), inner);
        }
        None => {}
    }

    let id = state
        .ledger
        .approve(caller.inner(), caller.recipient(), value, fee);
    Ok(id)
}

fn mint(canister: &TokenCanister, caller: Principal, to: Principal, amount: Nat) -> TxReceipt {
    {
        let balances = &mut canister.state.borrow_mut().balances;
        let to_balance = balances.balance_of(&to);
        balances.0.insert(to, to_balance + amount.clone());
    }

    let mut state = canister.state.borrow_mut();
    state.stats.total_supply += amount.clone();
    let id = state.ledger.mint(caller, to, amount);

    Ok(id)
}

pub(crate) fn mint_test_token(
    canister: &TokenCanister,
    caller: CheckedPrincipal<TestNet>,
    to: Principal,
    amount: Nat,
) -> TxReceipt {
    mint(canister, caller.inner(), to, amount)
}

pub(crate) fn mint_as_owner(
    canister: &TokenCanister,
    caller: CheckedPrincipal<Owner>,
    to: Principal,
    amount: Nat,
) -> TxReceipt {
    mint(canister, caller.inner(), to, amount)
}

fn burn(canister: &TokenCanister, caller: Principal, from: Principal, amount: Nat) -> TxReceipt {
    {
        let mut state = canister.state.borrow_mut();
        let balance = state.balances.balance_of(&from);
        if balance < amount {
            return Err(TxError::InsufficientBalance);
        }

        state.balances.0.insert(from, balance - amount.clone());
    }

    let mut state = canister.state.borrow_mut();
    state.stats.total_supply -= amount.clone();

    let id = state.ledger.burn(caller, from, amount);
    Ok(id)
}

pub fn burn_own_tokens(canister: &TokenCanister, amount: Nat) -> TxReceipt {
    let caller = ic_canister::ic_kit::ic::caller();
    burn(canister, caller, caller, amount)
}

pub fn burn_as_owner(
    canister: &TokenCanister,
    caller: CheckedPrincipal<Owner>,
    from: Principal,
    amount: Nat,
) -> TxReceipt {
    burn(canister, caller.inner(), from, amount)
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
    use ic_canister::ic_kit::mock_principals::{alice, bob, john, xtc};
    use ic_canister::ic_kit::MockContext;
    use std::collections::HashSet;
    use std::iter::FromIterator;

    use crate::canister::MAX_TRANSACTION_QUERY_LEN;
    use ic_canister::Canister;

    fn test_context() -> (&'static MockContext, TokenCanister) {
        let context = MockContext::new().with_caller(alice()).inject();

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

        (context, canister)
    }

    fn test_canister() -> TokenCanister {
        let (_, canister) = test_context();
        canister
    }

    #[test]
    fn transfer_without_fee() {
        let canister = test_canister();
        assert_eq!(Nat::from(1000), canister.balanceOf(alice()));

        let caller = CheckedPrincipal::with_recipient(bob()).unwrap();
        assert!(transfer(&canister, caller, Nat::from(100), None).is_ok());
        assert_eq!(canister.balanceOf(bob()), Nat::from(100));
        assert_eq!(canister.balanceOf(alice()), Nat::from(900));
    }

    #[test]
    fn transfer_with_fee() {
        let canister = test_canister();
        canister.state.borrow_mut().stats.fee = Nat::from(100);
        canister.state.borrow_mut().stats.fee_to = john();

        assert!(canister.transfer(bob(), Nat::from(200), None).is_ok());
        assert_eq!(canister.balanceOf(bob()), Nat::from(200));
        assert_eq!(canister.balanceOf(alice()), Nat::from(700));
        assert_eq!(canister.balanceOf(john()), Nat::from(100));
    }

    #[test]
    fn transfer_fee_exceeded() {
        let canister = test_canister();
        canister.state.borrow_mut().stats.fee = Nat::from(100);
        canister.state.borrow_mut().stats.fee_to = john();

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
        canister.state.borrow_mut().stats.fee = Nat::from(50);
        canister.state.borrow_mut().stats.fee_to = john();
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
        canister.state.borrow_mut().stats.fee = Nat::from(100);
        canister.state.borrow_mut().stats.fee_to = john();

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
            Err(TxError::SelfTransfer)
        );
        assert_eq!(canister.balanceOf(alice()), Nat::from(1000));
        assert_eq!(canister.balanceOf(bob()), Nat::from(0));
    }

    #[test]
    fn transfer_saved_into_history() {
        let (ctx, canister) = test_context();
        canister.state.borrow_mut().stats.fee = Nat::from(10);

        canister.transfer(bob(), Nat::from(1001), None).unwrap_err();
        assert_eq!(canister.historySize(), 1);

        const COUNT: usize = 5;
        let mut ts = ic_canister::ic_kit::ic::time().into();
        for i in 0..COUNT {
            ctx.add_time(10);
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
            canister.mint(alice(), Nat::from(100u32)),
            Err(TxError::Unauthorized)
        );

        canister.state.borrow_mut().stats.is_test_token = true;

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
        let (ctx, canister) = test_context();
        canister.state.borrow_mut().stats.fee = Nat::from(10);

        assert_eq!(canister.historySize(), 1);

        const COUNT: usize = 5;
        let mut ts = ic_canister::ic_kit::ic::time().into();
        for i in 0..COUNT {
            ctx.add_time(10);
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
        assert!(canister.burn(None, Nat::from(100)).is_ok());
        assert_eq!(canister.balanceOf(alice()), Nat::from(900));
        assert_eq!(canister.getMetadata().totalSupply, Nat::from(900));
    }

    #[test]
    fn burn_too_much() {
        let canister = test_canister();
        assert_eq!(
            canister.burn(None, Nat::from(1001)),
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
            canister.burn(None, Nat::from(100)),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice()), Nat::from(1000));
        assert_eq!(canister.getMetadata().totalSupply, Nat::from(1000));
    }

    #[test]
    fn burn_from() {
        let canister = test_canister();
        let bob_balance = Nat::from(1000);
        canister.mint(bob(), bob_balance.clone()).unwrap();
        assert_eq!(canister.balanceOf(bob()), bob_balance);

        canister.burn(Some(bob()), Nat::from(100)).unwrap();
        assert_eq!(canister.balanceOf(bob()), Nat::from(900));

        assert_eq!(canister.getMetadata().totalSupply, Nat::from(1900));
    }

    #[test]
    fn burn_from_unauthorized() {
        let canister = test_canister();
        let context = MockContext::new().with_caller(bob()).inject();
        context.update_caller(bob());
        assert_eq!(
            canister.burn(Some(alice()), Nat::from(100)),
            Err(TxError::Unauthorized)
        );
        assert_eq!(canister.balanceOf(alice()), Nat::from(1000));
        assert_eq!(canister.getMetadata().totalSupply, Nat::from(1000));
    }

    #[test]
    fn burn_saved_into_history() {
        let (ctx, canister) = test_context();
        canister.state.borrow_mut().stats.fee = Nat::from(10);

        canister.burn(None, Nat::from(1001)).unwrap_err();
        assert_eq!(canister.historySize(), 1);

        const COUNT: usize = 5;
        let mut ts = ic_canister::ic_kit::ic::time().into();
        for i in 0..COUNT {
            ctx.add_time(10);
            let id = canister.burn(None, Nat::from(100 + i)).unwrap();
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
        let (ctx, canister) = test_context();
        let context = MockContext::new().with_caller(alice()).inject();
        canister.state.borrow_mut().stats.fee = Nat::from(10);

        canister
            .transferFrom(bob(), john(), Nat::from(10))
            .unwrap_err();
        assert_eq!(canister.historySize(), 1);

        canister.approve(bob(), Nat::from(1000)).unwrap();
        context.update_caller(bob());

        const COUNT: usize = 5;
        let mut ts = ic_canister::ic_kit::ic::time().into();
        for i in 0..COUNT {
            ctx.add_time(10);
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
        canister.state.borrow_mut().stats.fee = Nat::from(100);
        canister.state.borrow_mut().stats.fee_to = bob();
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
        let (ctx, canister) = test_context();
        canister.state.borrow_mut().stats.fee = Nat::from(10);
        assert_eq!(canister.historySize(), 1);

        const COUNT: usize = 5;
        let mut ts = ic_canister::ic_kit::ic::time().into();
        for i in 0..COUNT {
            ctx.add_time(10);
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

        for _ in 1..5 {
            canister.transfer(bob(), Nat::from(10), None).unwrap();
        }

        canister.transfer(bob(), Nat::from(10), None).unwrap();
        canister.transfer(xtc(), Nat::from(10), None).unwrap();
        canister.transfer(john(), Nat::from(10), None).unwrap();

        assert_eq!(canister.getTransactions(None, 10, None).result.len(), 8);
        assert_eq!(canister.getTransactions(None, 10, Some(3)).result.len(), 4);

        assert_eq!(
            canister.getTransactions(Some(bob()), 5, None).result.len(),
            5
        );
        assert_eq!(
            canister.getTransactions(Some(xtc()), 5, None).result.len(),
            1
        );
        assert_eq!(
            canister
                .getTransactions(Some(alice()), 10, Some(5))
                .result
                .len(),
            6
        );
        assert_eq!(canister.getTransactions(None, 5, None).next, Some(2));
        assert_eq!(
            canister.getTransactions(Some(alice()), 3, Some(5)).next,
            Some(2)
        );
        assert_eq!(canister.getTransactions(Some(bob()), 3, Some(2)).next, None);
    }

    #[test]
    #[should_panic]
    fn get_transactions_over_limit() {
        let canister = test_canister();
        canister.getTransactions(None, (MAX_TRANSACTION_QUERY_LEN + 1) as u32, None);
    }

    #[test]
    #[should_panic]
    fn get_transaction_not_existing() {
        let canister = test_canister();
        canister.getTransaction(Nat::from(2));
    }

    #[test]
    fn get_transaction_count() {
        let canister = test_canister();
        const COUNT: usize = 10;
        for _ in 1..COUNT {
            canister.transfer(bob(), Nat::from(10), None).unwrap();
        }
        assert_eq!(canister.getUserTransactionCount(alice()), Nat::from(COUNT));
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use common::types::Metadata;
    use ic_canister::ic_kit::MockContext;
    use ic_canister::Canister;
    use proptest::collection::vec;
    use proptest::prelude::*;
    use proptest::sample::Index;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Action {
        Mint {
            minter: Principal,
            recipient: Principal,
            amount: Nat,
        },
        Burn(Nat, Principal),
        TransferWithFee {
            from: Principal,
            to: Principal,
            amount: Nat,
        },
        TransferWithoutFee {
            caller: Principal,
            from: Principal,
            to: Principal,
            amount: Nat,
            fee_limit: Option<Nat>,
        },
        TransferFrom {
            caller: Principal,
            from: Principal,
            to: Principal,
            on_behalf_of: Principal,
            amount: Nat,
        },
    }

    prop_compose! {
        fn select_principal(p: Vec<Principal>) (index in any::<Index>()) -> Principal {
            let i = index.index(p.len());
            p[i]
        }

    }

    fn make_action(principals: Vec<Principal>) -> impl Strategy<Value = Action> {
        prop_oneof![
            // Mint
            (
                make_nat(),
                select_principal(principals.clone()),
                select_principal(principals.clone()),
            )
                .prop_map(|(amount, minter, recipient)| Action::Mint {
                    minter,
                    recipient,
                    amount
                }),
            // Burn
            (make_nat(), select_principal(principals.clone()))
                .prop_map(|(amount, principal)| Action::Burn(amount, principal)),
            // With fee
            (
                select_principal(principals.clone()),
                select_principal(principals.clone()),
                make_nat()
            )
                .prop_map(|(from, to, amount)| Action::TransferWithFee {
                    from,
                    to,
                    amount
                }),
            // Without fee
            (
                select_principal(principals.clone()),
                select_principal(principals.clone()),
                select_principal(principals.clone()),
                make_nat(),
                make_option(),
            )
                .prop_map(|(caller, from, to, amount, fee_limit)| {
                    Action::TransferWithoutFee {
                        caller,
                        from,
                        to,
                        amount,
                        fee_limit,
                    }
                }),
            // Transfer from
            (
                select_principal(principals.clone()),
                select_principal(principals.clone()),
                select_principal(principals.clone()),
                select_principal(principals),
                make_nat()
            )
                .prop_map(|(principal, from, to, on_behalf_of, amount)| {
                    Action::TransferFrom {
                        caller: principal,
                        from,
                        to,
                        on_behalf_of,
                        amount,
                    }
                })
        ]
    }

    fn make_option() -> impl Strategy<Value = Option<Nat>> {
        prop_oneof![Just(None), (make_nat()).prop_map(Some)]
    }

    fn make_principal() -> BoxedStrategy<Principal> {
        (any::<[u8; 29]>().prop_map(|mut bytes| {
            // Make sure the last byte is more than four as the last byte carries special
            // meaning
            bytes[28] = bytes[28].saturating_add(5);
            bytes
        }))
        .prop_map(|bytes| Principal::from_slice(&bytes))
        .boxed()
    }

    prop_compose! {
        fn make_nat() (num in "[0-9]{1,4}") -> Nat {
            let nat = Nat::parse(num.as_bytes()).unwrap();
            nat
        }
    }
    prop_compose! {
        fn make_canister() (
            logo in any::<String>(),
            name in any::<String>(),
            symbol in any::<String>(),
            decimals in any::<u8>(),
            totalSupply in make_nat(),
            fee in make_nat(),
            principals in vec(make_principal(), 1..7),
            owner_idx in any::<Index>(),
            fee_to_idx in any::<Index>(),
        )-> (TokenCanister, Vec<Principal>) {
            // pick two random principals (they could very well be the same principal twice)
            let owner = principals[owner_idx.index(principals.len())];
            let fee_to = principals[fee_to_idx.index(principals.len())];
            MockContext::new().with_caller(owner).inject();
            let meta = Metadata {
                logo,
                name,
                symbol,
                decimals,
                totalSupply,
                owner,
                fee,
                feeTo: fee_to,
                isTestToken: None,
            };
            let canister = TokenCanister::init_instance();
            canister.init(meta);
            (canister, principals)
        }
    }
    fn canister_and_actions() -> impl Strategy<Value = (TokenCanister, Vec<Action>)> {
        make_canister().prop_flat_map(|(canister, principals)| {
            let actions = vec(make_action(principals), 1..7);
            (Just(canister), actions)
        })
    }
    proptest! {
        #[test]
        fn generic_proptest((canister, actions) in canister_and_actions()) {
            let mut total_minted = Nat::from(0);
            let mut total_burned = Nat::from(0);
            let starting_supply = canister.totalSupply();
            for action in actions {
                use Action::*;
                match action {
                    Mint { minter, recipient, amount } => {
                        MockContext::new().with_caller(minter).inject();
                        let original = canister.totalSupply();
                        let res = canister.mint(recipient, amount.clone());
                        let expected = if minter == canister.owner() {
                            total_minted += amount.clone();
                            assert!(matches!(res, Ok(_)));
                            original + amount
                        } else {
                            assert_eq!(res, Err(TxError::Unauthorized));
                            original
                        };
                        assert_eq!(expected, canister.totalSupply());
                    },
                    Burn(amount, burner) => {
                        MockContext::new().with_caller(burner).inject();
                        let original = canister.totalSupply();
                        let balance = canister.balanceOf(burner);
                        let res = canister.burn(Some(burner), amount.clone());
                        if balance < amount {
                            prop_assert_eq!(res, Err(TxError::InsufficientBalance));
                            prop_assert_eq!(original, canister.totalSupply());
                        } else {
                            prop_assert!(matches!(res, Ok(_)));
                            prop_assert_eq!(original.clone() - amount.clone(), canister.totalSupply());
                            total_burned += amount.clone();
                        }
                    },
                    TransferFrom { caller, from, to, on_behalf_of, amount } => {
                        // Transfers value amount of tokens from user from to user to,
                        // this method allows canister smart contracts to transfer tokens on your behalf,
                        // it returns a TxReceipt which contains the transaction index or an error message.
                        //
                        // If the fee is set, the from principal is charged with the fee. In this case,
                        // the maximum amount that the caller can request to transfer is allowance - fee.
                        MockContext::new().with_caller(caller).inject();
                        let from_balance = canister.balanceOf(from);
                        let to_balance = canister.balanceOf(to);
                        // let res = canister.transferFrom(from, to, amount);
                    },
                    TransferWithoutFee{caller,from,to,amount,fee_limit} => {
                        MockContext::new().with_caller(from).inject();
                        let from_balance = canister.balanceOf(from);
                        let to_balance = canister.balanceOf(to);
                        let fee = canister.state.borrow().stats.fee.clone();
                        let amount_with_fee = amount.clone() + fee.clone();
                        let res = canister.transfer(to, amount.clone(), fee_limit.clone());
                        // The balance of the caller is reduced by `value + fee` amount.

                        if to == from {
                            prop_assert_eq!(res, Err(TxError::SelfTransfer));
                            return Ok(())
                        }



                        if let Some(fee_limit) = fee_limit.clone() {
                            if fee_limit.clone() < fee.clone() {
                                prop_assert_eq!(res, Err(TxError::FeeExceededLimit));
                                return Ok(())
                            }
                        }

                        if from_balance < amount_with_fee {
                            prop_assert_eq!(res, Err(TxError::InsufficientBalance));
                            return Ok(())
                        }

                        println!("-------------------------------------------------");
                        println!("starting balance of from : {from_balance}");
                        println!("starting balance of to : {to_balance}");
                        println!("amount                   : {}", amount.clone());
                        println!("fee                      : {}", fee.clone());
                        println!("fee limit                      : {:?}", fee_limit.clone());
                        println!("current: balance of from : {}", canister.balanceOf(from));
                        println!("current: balance of to : {}", canister.balanceOf(to));
                        println!("RESULT : {:?}", res);
                        println!("-------------------------------------------------");

                        // prop_assert_eq!(to_balance + amount.clone(), canister.balanceOf(to));
                        // prop_assert_eq!(from_balance - amount.clone() - fee.clone(), canister.balanceOf(from));
                        prop_assert!(matches!(res, Ok(_)));



                    }
                    TransferWithFee { from, to, amount } => {
                        MockContext::new().with_caller(from).inject();
                        let from_balance = canister.balanceOf(from);
                        let to_balance = canister.balanceOf(to);
                        let fee = canister.state.borrow().stats.fee.clone();
                        let res = canister.transferIncludeFee(to, amount.clone());

                         if to == from {
                            prop_assert_eq!(res, Err(TxError::SelfTransfer));
                            return Ok(())
                        }

                        if amount.clone() <= fee.clone()  {
                            prop_assert_eq!(res, Err(TxError::AmountTooSmall));
                            return Ok(());
                        }
                        if from_balance < amount.clone(){
                            prop_assert_eq!(res, Err(TxError::InsufficientBalance));
                            return Ok(());
                        }



                        prop_assert!(matches!(res, Ok(_)));
                        // prop_assert_eq!(to_balance.clone() + amount.clone() - fee.clone(), canister.balanceOf(to));
                        // prop_assert_eq!(from_balance.clone() - amount.clone(), canister.balanceOf(from));

                    }
                }
            }
            prop_assert_eq!(total_minted.clone() + starting_supply - total_burned.clone(), canister.totalSupply());
        }
    }
}
