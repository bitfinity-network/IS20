use ic_cdk::export::Principal;
use ic_helpers::tokens::Tokens128;

use crate::canister::is20_auction::auction_principal;
use crate::principal::{CheckedPrincipal, Owner, TestNet, WithRecipient};
use crate::state::{Balances, CanisterState};
use crate::types::{TxError, TxReceipt};

use super::TokenCanister;

pub fn transfer(
    canister: &TokenCanister,
    caller: CheckedPrincipal<WithRecipient>,
    amount: Tokens128,
    fee_limit: Option<Tokens128>,
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

    if balances.balance_of(&caller.inner())
        < (amount + fee).ok_or_else(|| TxError::AmountOverflow)?
    {
        return Err(TxError::InsufficientBalance);
    }

    charge_fee(balances, caller.inner(), fee_to, fee, fee_ratio)
        .expect("never fails due to checks above");
    transfer_balance(balances, caller.inner(), caller.recipient(), amount)
        .expect("never fails due to checks above");

    let id = ledger.transfer(caller.inner(), caller.recipient(), amount, fee);
    Ok(id)
}

pub fn transfer_from(
    canister: &TokenCanister,
    caller: CheckedPrincipal<WithRecipient>,
    from: Principal,
    amount: Tokens128,
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

    let value_with_fee = (amount + fee).ok_or_else(|| TxError::AmountOverflow)?;
    if from_allowance < value_with_fee {
        return Err(TxError::InsufficientAllowance);
    }

    let from_balance = balances.balance_of(&from);
    if from_balance < value_with_fee {
        return Err(TxError::InsufficientBalance);
    }

    charge_fee(balances, from, fee_to, fee, fee_ratio).expect("never fails due to checks above");
    transfer_balance(balances, from, caller.recipient(), amount)
        .expect("never fails due to checks above");

    let allowances = state
        .allowances
        .get_mut(&from)
        .expect("allowance existing is checked above when check allowance sufficiency");
    let allowance = allowances
        .get_mut(&caller.inner())
        .expect("allowance existing is checked above when check allowance sufficiency");
    *allowance = (*allowance - value_with_fee).expect("allowance sufficiency checked above");

    if *allowance == Tokens128::from(0u128) {
        allowances.remove(&caller.inner());

        if allowances.is_empty() {
            state.allowances.remove(&from);
        }
    }

    let id = state
        .ledger
        .transfer_from(caller.inner(), from, caller.recipient(), amount, fee);
    Ok(id)
}

pub fn approve(
    canister: &TokenCanister,
    caller: CheckedPrincipal<WithRecipient>,
    amount: Tokens128,
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

    charge_fee(balances, caller.inner(), fee_to, fee, fee_ratio)
        .expect("never fails due to checks above");
    let amount_with_fee = (amount + fee).ok_or(TxError::AmountOverflow)?;

    if amount_with_fee == Tokens128::from(0u128) {
        if let Some(allowances) = state.allowances.get_mut(&caller.inner()) {
            allowances.remove(&caller.recipient());
            if allowances.is_empty() {
                state.allowances.remove(&caller.inner());
            }
        }
    } else {
        state
            .allowances
            .entry(caller.inner())
            .or_default()
            .insert(caller.recipient(), amount_with_fee);
    }

    let id = state
        .ledger
        .approve(caller.inner(), caller.recipient(), amount, fee);
    Ok(id)
}

fn mint(
    state: &mut CanisterState,
    caller: Principal,
    to: Principal,
    amount: Tokens128,
) -> TxReceipt {
    state.stats.total_supply =
        (state.stats.total_supply + amount).ok_or(TxError::AmountOverflow)?;
    let balance = state.balances.0.entry(to).or_default();
    let new_balance = (*balance + amount)
        .expect("balance cannot be larger than total_supply which is already checked");
    *balance = new_balance;

    let id = state.ledger.mint(caller, to, amount);

    Ok(id)
}

pub(crate) fn mint_test_token(
    state: &mut CanisterState,
    caller: CheckedPrincipal<TestNet>,
    to: Principal,
    amount: Tokens128,
) -> TxReceipt {
    mint(state, caller.inner(), to, amount)
}

pub(crate) fn mint_as_owner(
    state: &mut CanisterState,
    caller: CheckedPrincipal<Owner>,
    to: Principal,
    amount: Tokens128,
) -> TxReceipt {
    mint(state, caller.inner(), to, amount)
}

fn burn(
    state: &mut CanisterState,
    caller: Principal,
    from: Principal,
    amount: Tokens128,
) -> TxReceipt {
    match state.balances.0.get_mut(&from) {
        Some(balance) => {
            *balance = (*balance - amount).ok_or(TxError::InsufficientBalance)?;
            if *balance == Tokens128::from(0) {
                state.balances.0.remove(&from);
            }
        }
        None => return Err(TxError::InsufficientBalance),
    }

    state.stats.total_supply =
        (state.stats.total_supply - amount).expect("total supply cannot be less then user balance");

    let id = state.ledger.burn(caller, from, amount);
    Ok(id)
}

pub fn burn_own_tokens(state: &mut CanisterState, amount: Tokens128) -> TxReceipt {
    let caller = ic_canister::ic_kit::ic::caller();
    burn(state, caller, caller, amount)
}

pub fn burn_as_owner(
    state: &mut CanisterState,
    caller: CheckedPrincipal<Owner>,
    from: Principal,
    amount: Tokens128,
) -> TxReceipt {
    burn(state, caller.inner(), from, amount)
}

pub(crate) fn transfer_balance(
    balances: &mut Balances,
    from: Principal,
    to: Principal,
    amount: Tokens128,
) -> Result<(), TxError> {
    {
        let from_balance = balances
            .0
            .get_mut(&from)
            .ok_or(TxError::InsufficientBalance)?;
        *from_balance = (*from_balance - amount).ok_or(TxError::InsufficientBalance)?;
    }

    {
        let to_balance = balances.0.entry(to).or_default();
        *to_balance = (*to_balance + amount).expect(
            "never overflows since `from_balance + to_balance` is limited by `total_supply` amount",
        );
    }

    if *balances.0.get(&from).expect("checked above") == Tokens128::from(0) {
        balances.0.remove(&from);
    }

    Ok(())
}

pub(crate) fn charge_fee(
    balances: &mut Balances,
    user: Principal,
    fee_to: Principal,
    fee: Tokens128,
    fee_ratio: f64,
) -> Result<(), TxError> {
    // todo: check if this is enforced
    debug_assert!(fee_ratio >= 0.0 && fee_ratio <= 1.0);

    if fee == Tokens128::from(0) {
        return Ok(());
    }

    // todo: test and figure out overflows
    const INT_CONVERSION_K: u128 = 1_000_000_000_000;
    let auction_fee_amount = (fee * Tokens128::from((fee_ratio * INT_CONVERSION_K as f64) as u128)
        / INT_CONVERSION_K)
        .expect("never division by 0");
    let auction_fee_amount = auction_fee_amount
        .to_tokens128()
        .expect("fee is always greater");
    let owner_fee_amount = (fee - auction_fee_amount).expect("fee is always greater");
    transfer_balance(balances, user, fee_to, owner_fee_amount)?;
    transfer_balance(balances, user, auction_principal(), auction_fee_amount)?;

    Ok(())
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
            totalSupply: Tokens128::from(1000),
            owner: alice(),
            fee: Tokens128::from(0),
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
        assert_eq!(Tokens128::from(1000), canister.balanceOf(alice()));

        let caller = CheckedPrincipal::with_recipient(bob()).unwrap();
        assert!(transfer(&canister, caller, Tokens128::from(100), None).is_ok());
        assert_eq!(canister.balanceOf(bob()), Tokens128::from(100));
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(900));
    }

    #[test]
    fn transfer_with_fee() {
        let canister = test_canister();
        canister.state.borrow_mut().stats.fee = Tokens128::from(100);
        canister.state.borrow_mut().stats.fee_to = john();

        assert!(canister.transfer(bob(), Tokens128::from(200), None).is_ok());
        assert_eq!(canister.balanceOf(bob()), Tokens128::from(200));
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(700));
        assert_eq!(canister.balanceOf(john()), Tokens128::from(100));
    }

    #[test]
    fn transfer_fee_exceeded() {
        let canister = test_canister();
        canister.state.borrow_mut().stats.fee = Tokens128::from(100);
        canister.state.borrow_mut().stats.fee_to = john();

        assert!(canister
            .transfer(bob(), Tokens128::from(200), Some(Tokens128::from(100)))
            .is_ok());
        assert_eq!(
            canister.transfer(bob(), Tokens128::from(200), Some(Tokens128::from(50))),
            Err(TxError::FeeExceededLimit)
        );
    }

    #[test]
    fn fees_with_auction_enabled() {
        let canister = test_canister();
        canister.state.borrow_mut().stats.fee = Tokens128::from(50);
        canister.state.borrow_mut().stats.fee_to = john();
        canister.state.borrow_mut().bidding_state.fee_ratio = 0.5;

        canister
            .transfer(bob(), Tokens128::from(100), None)
            .unwrap();
        assert_eq!(canister.balanceOf(bob()), Tokens128::from(100));
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(850));
        assert_eq!(canister.balanceOf(john()), Tokens128::from(25));
        assert_eq!(canister.balanceOf(auction_principal()), Tokens128::from(25));
    }

    #[test]
    fn transfer_insufficient_balance() {
        let canister = test_canister();
        assert_eq!(
            canister.transfer(bob(), Tokens128::from(1001), None),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(1000));
        assert_eq!(canister.balanceOf(bob()), Tokens128::from(0));
    }

    #[test]
    fn transfer_with_fee_insufficient_balance() {
        let canister = test_canister();
        canister.state.borrow_mut().stats.fee = Tokens128::from(100);
        canister.state.borrow_mut().stats.fee_to = john();

        assert_eq!(
            canister.transfer(bob(), Tokens128::from(950), None),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(1000));
        assert_eq!(canister.balanceOf(bob()), Tokens128::from(0));
    }

    #[test]
    fn transfer_wrong_caller() {
        let canister = test_canister();
        MockContext::new().with_caller(bob()).inject();
        assert_eq!(
            canister.transfer(bob(), Tokens128::from(100), None),
            Err(TxError::SelfTransfer)
        );
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(1000));
        assert_eq!(canister.balanceOf(bob()), Tokens128::from(0));
    }

    #[test]
    fn transfer_saved_into_history() {
        let (ctx, canister) = test_context();
        canister.state.borrow_mut().stats.fee = Tokens128::from(10);

        canister
            .transfer(bob(), Tokens128::from(1001), None)
            .unwrap_err();
        assert_eq!(canister.historySize(), 1);

        const COUNT: u64 = 5;
        let mut ts = ic_canister::ic_kit::ic::time().into();
        for i in 0..COUNT {
            ctx.add_time(10);
            let id = canister
                .transfer(bob(), Tokens128::from(100 + i as u128), None)
                .unwrap();
            assert_eq!(canister.historySize(), 2 + i);
            let tx = canister.getTransaction(id);
            assert_eq!(tx.amount, Tokens128::from(100 + i as u128));
            assert_eq!(tx.fee, Tokens128::from(10));
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
            canister.mint(alice(), Tokens128::from(100)),
            Err(TxError::Unauthorized)
        );

        canister.state.borrow_mut().stats.is_test_token = true;

        assert!(canister.mint(alice(), Tokens128::from(2000)).is_ok());
        assert!(canister.mint(bob(), Tokens128::from(5000)).is_ok());
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(3000));
        assert_eq!(canister.balanceOf(bob()), Tokens128::from(5000));
    }

    #[test]
    fn mint_by_owner() {
        let canister = test_canister();
        assert!(canister.mint(alice(), Tokens128::from(2000)).is_ok());
        assert!(canister.mint(bob(), Tokens128::from(5000)).is_ok());
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(3000));
        assert_eq!(canister.balanceOf(bob()), Tokens128::from(5000));
        assert_eq!(canister.getMetadata().totalSupply, Tokens128::from(8000));
    }

    #[test]
    fn mint_saved_into_history() {
        let (ctx, canister) = test_context();
        canister.state.borrow_mut().stats.fee = Tokens128::from(10);

        assert_eq!(canister.historySize(), 1);

        const COUNT: u64 = 5;
        let mut ts = ic_canister::ic_kit::ic::time().into();
        for i in 0..COUNT {
            ctx.add_time(10);
            let id = canister
                .mint(bob(), Tokens128::from(100 + i as u128))
                .unwrap();
            assert_eq!(canister.historySize(), 2 + i);
            let tx = canister.getTransaction(id);
            assert_eq!(tx.amount, Tokens128::from(100 + i as u128));
            assert_eq!(tx.fee, Tokens128::from(0));
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
        assert!(canister.burn(None, Tokens128::from(100)).is_ok());
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(900));
        assert_eq!(canister.getMetadata().totalSupply, Tokens128::from(900));
    }

    #[test]
    fn burn_too_much() {
        let canister = test_canister();
        assert_eq!(
            canister.burn(None, Tokens128::from(1001)),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(1000));
        assert_eq!(canister.getMetadata().totalSupply, Tokens128::from(1000));
    }

    #[test]
    fn burn_by_wrong_user() {
        let canister = test_canister();
        let context = MockContext::new().with_caller(bob()).inject();
        context.update_caller(bob());
        assert_eq!(
            canister.burn(None, Tokens128::from(100)),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(1000));
        assert_eq!(canister.getMetadata().totalSupply, Tokens128::from(1000));
    }

    #[test]
    fn burn_from() {
        let canister = test_canister();
        let bob_balance = Tokens128::from(1000);
        canister.mint(bob(), bob_balance.clone()).unwrap();
        assert_eq!(canister.balanceOf(bob()), bob_balance);

        canister.burn(Some(bob()), Tokens128::from(100)).unwrap();
        assert_eq!(canister.balanceOf(bob()), Tokens128::from(900));

        assert_eq!(canister.getMetadata().totalSupply, Tokens128::from(1900));
    }

    #[test]
    fn burn_from_unauthorized() {
        let canister = test_canister();
        let context = MockContext::new().with_caller(bob()).inject();
        context.update_caller(bob());
        assert_eq!(
            canister.burn(Some(alice()), Tokens128::from(100)),
            Err(TxError::Unauthorized)
        );
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(1000));
        assert_eq!(canister.getMetadata().totalSupply, Tokens128::from(1000));
    }

    #[test]
    fn burn_saved_into_history() {
        let (ctx, canister) = test_context();
        canister.state.borrow_mut().stats.fee = Tokens128::from(10);

        canister.burn(None, Tokens128::from(1001)).unwrap_err();
        assert_eq!(canister.historySize(), 1);

        const COUNT: u64 = 5;
        let mut ts = ic_canister::ic_kit::ic::time().into();
        for i in 0..COUNT {
            ctx.add_time(10);
            let id = canister
                .burn(None, Tokens128::from(100 + i as u128))
                .unwrap();
            assert_eq!(canister.historySize(), 2 + i);
            let tx = canister.getTransaction(id);
            assert_eq!(tx.amount, Tokens128::from(100 + i as u128));
            assert_eq!(tx.fee, Tokens128::from(0));
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
        assert!(canister.approve(bob(), Tokens128::from(500)).is_ok());
        context.update_caller(bob());

        assert!(canister
            .transferFrom(alice(), john(), Tokens128::from(100))
            .is_ok());
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(900));
        assert_eq!(canister.balanceOf(john()), Tokens128::from(100));
        assert!(canister
            .transferFrom(alice(), john(), Tokens128::from(100))
            .is_ok());
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(800));
        assert_eq!(canister.balanceOf(john()), Tokens128::from(200));
        assert!(canister
            .transferFrom(alice(), john(), Tokens128::from(300))
            .is_ok());

        assert_eq!(canister.balanceOf(alice()), Tokens128::from(500));
        assert_eq!(canister.balanceOf(bob()), Tokens128::from(0));
        assert_eq!(canister.balanceOf(john()), Tokens128::from(500));
    }

    #[test]
    fn insufficient_allowance() {
        let canister = test_canister();
        let context = MockContext::new().with_caller(alice()).inject();
        assert!(canister.approve(bob(), Tokens128::from(500)).is_ok());
        context.update_caller(bob());
        assert_eq!(
            canister.transferFrom(alice(), john(), Tokens128::from(600)),
            Err(TxError::InsufficientAllowance)
        );
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(1000));
        assert_eq!(canister.balanceOf(john()), Tokens128::from(0));
    }

    #[test]
    fn transfer_from_without_approve() {
        let canister = test_canister();
        let context = MockContext::new().with_caller(alice()).inject();
        context.update_caller(bob());
        assert_eq!(
            canister.transferFrom(alice(), john(), Tokens128::from(600)),
            Err(TxError::InsufficientAllowance)
        );
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(1000));
        assert_eq!(canister.balanceOf(john()), Tokens128::from(0));
    }

    #[test]
    fn transfer_from_saved_into_history() {
        let (ctx, canister) = test_context();
        let context = MockContext::new().with_caller(alice()).inject();
        canister.state.borrow_mut().stats.fee = Tokens128::from(10);

        canister
            .transferFrom(bob(), john(), Tokens128::from(10))
            .unwrap_err();
        assert_eq!(canister.historySize(), 1);

        canister.approve(bob(), Tokens128::from(1000)).unwrap();
        context.update_caller(bob());

        const COUNT: u64 = 5;
        let mut ts = ic_canister::ic_kit::ic::time().into();
        for i in 0..COUNT {
            ctx.add_time(10);
            let id = canister
                .transferFrom(alice(), john(), Tokens128::from(100 + i as u128))
                .unwrap();
            assert_eq!(canister.historySize(), 3 + i);
            let tx = canister.getTransaction(id);
            assert_eq!(tx.caller, Some(bob()));
            assert_eq!(tx.amount, Tokens128::from(100 + i as u128));
            assert_eq!(tx.fee, Tokens128::from(10));
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
        assert!(canister.approve(bob(), Tokens128::from(500)).is_ok());
        assert_eq!(
            canister.getUserApprovals(alice()),
            vec![(bob(), Tokens128::from(500))]
        );

        assert!(canister.approve(bob(), Tokens128::from(200)).is_ok());
        assert_eq!(
            canister.getUserApprovals(alice()),
            vec![(bob(), Tokens128::from(200))]
        );

        assert!(canister.approve(john(), Tokens128::from(1000)).is_ok());

        // Convert vectors to sets before comparing to make comparison unaffected by the element
        // order.
        assert_eq!(
            HashSet::<&(Principal, Tokens128)>::from_iter(
                canister.getUserApprovals(alice()).iter()
            ),
            HashSet::from_iter(
                vec![
                    (bob(), Tokens128::from(200)),
                    (john(), Tokens128::from(1000))
                ]
                .iter()
            )
        );
    }

    #[test]
    fn approve_over_balance() {
        let canister = test_canister();
        let context = MockContext::new().with_caller(alice()).inject();
        assert!(canister.approve(bob(), Tokens128::from(1500)).is_ok());
        context.update_caller(bob());
        assert!(canister
            .transferFrom(alice(), john(), Tokens128::from(500))
            .is_ok());
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(500));
        assert_eq!(canister.balanceOf(john()), Tokens128::from(500));

        assert_eq!(
            canister.transferFrom(alice(), john(), Tokens128::from(600)),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(500));
        assert_eq!(canister.balanceOf(john()), Tokens128::from(500));
    }

    #[test]
    fn transfer_from_with_fee() {
        let canister = test_canister();
        canister.state.borrow_mut().stats.fee = Tokens128::from(100);
        canister.state.borrow_mut().stats.fee_to = bob();
        let context = MockContext::new().with_caller(alice()).inject();

        assert!(canister.approve(bob(), Tokens128::from(1500)).is_ok());
        assert_eq!(canister.balanceOf(bob()), Tokens128::from(100));
        context.update_caller(bob());

        assert!(canister
            .transferFrom(alice(), john(), Tokens128::from(300))
            .is_ok());
        assert_eq!(canister.balanceOf(bob()), Tokens128::from(200));
        assert_eq!(canister.balanceOf(alice()), Tokens128::from(500));
        assert_eq!(canister.balanceOf(john()), Tokens128::from(300));
    }

    #[test]
    fn approve_saved_into_history() {
        let (ctx, canister) = test_context();
        canister.state.borrow_mut().stats.fee = Tokens128::from(10);
        assert_eq!(canister.historySize(), 1);

        const COUNT: u64 = 5;
        let mut ts = ic_canister::ic_kit::ic::time().into();
        for i in 0..COUNT {
            ctx.add_time(10);
            let id = canister
                .approve(bob(), Tokens128::from(100 + i as u128))
                .unwrap();
            assert_eq!(canister.historySize(), 2 + i);
            let tx = canister.getTransaction(id);
            assert_eq!(tx.amount, Tokens128::from(100 + i as u128));
            assert_eq!(tx.fee, Tokens128::from(10));
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
            canister.transfer(bob(), Tokens128::from(10), None).unwrap();
        }

        canister.transfer(bob(), Tokens128::from(10), None).unwrap();
        canister.transfer(xtc(), Tokens128::from(10), None).unwrap();
        canister
            .transfer(john(), Tokens128::from(10), None)
            .unwrap();

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
        canister.getTransactions(None, (MAX_TRANSACTION_QUERY_LEN + 1) as usize, None);
    }

    #[test]
    #[should_panic]
    fn get_transaction_not_existing() {
        let canister = test_canister();
        canister.getTransaction(2);
    }

    #[test]
    fn get_transaction_count() {
        let canister = test_canister();
        const COUNT: usize = 10;
        for _ in 1..COUNT {
            canister.transfer(bob(), Tokens128::from(10), None).unwrap();
        }
        assert_eq!(canister.getUserTransactionCount(alice()), COUNT);
    }
}
