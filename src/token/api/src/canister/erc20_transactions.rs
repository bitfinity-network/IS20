use ic_cdk::export::Principal;
use ic_helpers::tokens::Tokens128;

use crate::canister::is20_auction::auction_principal;
use crate::principal::{CheckedPrincipal, Owner, SenderRecipient, TestNet, WithRecipient};
use crate::state::{Balances, CanisterState};
use crate::types::{
    AccountIdentifier, CheckedIdentifier, OwnerAid, Subaccount, TestNetAid, TxError, TxReceipt,
    WithAidRecipient,
};

use super::TokenCanisterAPI;

pub fn icrc1_transfer(
    canister: &impl TokenCanisterAPI,
    caller: CheckedPrincipal<WithRecipient>,
    from_subaccount: Option<Subaccount>,
    to_subaccount: Option<Subaccount>,
    amount: Tokens128,
    fee_limit: Option<Tokens128>,
) -> TxReceipt {
    let from = AccountIdentifier::new(caller.inner(), from_subaccount);
    let to = AccountIdentifier::new(caller.recipient(), to_subaccount);
    let state = canister.state();
    let mut state = state.borrow_mut();
    let CanisterState {
        ref mut balances,
        ref bidding_state,
        ref stats,
        ..
    } = &mut *state;

    let (fee, fee_to) = stats.fee_info();
    let fee_ratio = bidding_state.fee_ratio;

    if let Some(fee_limit) = fee_limit {
        if fee > fee_limit {
            return Err(TxError::FeeExceededLimit);
        }
    }

    if balances.balance_of(&from) < (amount + fee).ok_or(TxError::AmountOverflow)? {
        return Err(TxError::InsufficientBalance);
    }

    let fee_to = AccountIdentifier::from(fee_to);

    charge_fee(balances, from, fee_to, fee, fee_ratio).expect("never fails due to checks above");

    transfer_balance(balances, from, to, amount).expect("never fails due to checks above");

    let id = state.ledger.transfer(from, to, amount, fee);
    Ok(id)
}

pub fn is20_transfer(
    canister: &impl TokenCanisterAPI,
    caller: CheckedIdentifier<WithAidRecipient>,
    amount: Tokens128,
    fee_limit: Option<Tokens128>,
) -> TxReceipt {
    let from = caller.inner();
    let to = caller.recipient();
    let state = canister.state();
    let mut state = state.borrow_mut();
    let CanisterState {
        ref mut balances,
        ref bidding_state,
        ref stats,
        ..
    } = &mut *state;

    let (fee, fee_to) = stats.fee_info();
    let fee_ratio = bidding_state.fee_ratio;

    if let Some(fee_limit) = fee_limit {
        if fee > fee_limit {
            return Err(TxError::FeeExceededLimit);
        }
    }

    if balances.balance_of(&from) < (amount + fee).ok_or(TxError::AmountOverflow)? {
        return Err(TxError::InsufficientBalance);
    }

    let fee_to = AccountIdentifier::from(fee_to);

    charge_fee(balances, from, fee_to, fee, fee_ratio).expect("never fails due to checks above");

    transfer_balance(balances, from, to, amount).expect("never fails due to checks above");

    let id = state.ledger.transfer(from, to, amount, fee);
    Ok(id)
}

pub fn icrc1_transfer_from(
    canister: &impl TokenCanisterAPI,
    caller: CheckedPrincipal<SenderRecipient>,
    from_subaccount: Option<Subaccount>,
    to_subaccount: Option<Subaccount>,
    amount: Tokens128,
) -> TxReceipt {
    let from = AccountIdentifier::new(caller.from(), from_subaccount);
    let to = AccountIdentifier::new(caller.to(), to_subaccount);
    let state = canister.state();
    let mut state = state.borrow_mut();
    let from_allowance = state.allowance(from, caller.inner().into());
    let CanisterState {
        ref mut balances,
        ref bidding_state,
        ref stats,
        ..
    } = &mut *state;

    let (fee, fee_to) = stats.fee_info();
    let fee_ratio = bidding_state.fee_ratio;

    let value_with_fee = (amount + fee).ok_or(TxError::AmountOverflow)?;
    if from_allowance < value_with_fee {
        return Err(TxError::InsufficientAllowance);
    }

    let from_balance = balances.balance_of(&from);
    if from_balance < value_with_fee {
        return Err(TxError::InsufficientBalance);
    }
    let fee_to = AccountIdentifier::from(fee_to);

    charge_fee(balances, from, fee_to, fee, fee_ratio).expect("never fails due to checks above");

    transfer_balance(balances, from, to, amount).expect("never fails due to checks above");

    let allowances = state
        .allowances
        .get_mut(&from)
        .expect("allowance existing is checked above when check allowance sufficiency");
    let allowance = allowances
        .get_mut(&caller.inner().into())
        .expect("allowance existing is checked above when check allowance sufficiency");
    *allowance = (*allowance - value_with_fee).expect("allowance sufficiency checked above");

    if *allowance == Tokens128::from(0u128) {
        allowances.remove(&caller.inner().into());

        if allowances.is_empty() {
            state.allowances.remove(&from);
        }
    }

    let id = state
        .ledger
        .transfer_from(from, to, amount, fee, caller.inner().into());
    Ok(id)
}

pub fn is20_transfer_from(
    canister: &impl TokenCanisterAPI,
    from: AccountIdentifier,
    to: AccountIdentifier,
    amount: Tokens128,
) -> TxReceipt {
    let caller = ic_canister::ic_kit::ic::caller();
    let state = canister.state();
    let mut state = state.borrow_mut();
    let from_allowance = state.allowance(from, ic_canister::ic_kit::ic::caller().into());
    let CanisterState {
        ref mut balances,
        ref bidding_state,
        ref stats,
        ..
    } = &mut *state;

    let (fee, fee_to) = stats.fee_info();
    let fee_ratio = bidding_state.fee_ratio;

    let value_with_fee = (amount + fee).ok_or(TxError::AmountOverflow)?;
    if from_allowance < value_with_fee {
        return Err(TxError::InsufficientAllowance);
    }

    let from_balance = balances.balance_of(&from);
    if from_balance < value_with_fee {
        return Err(TxError::InsufficientBalance);
    }
    let fee_to = AccountIdentifier::from(fee_to);

    charge_fee(balances, from, fee_to, fee, fee_ratio).expect("never fails due to checks above");

    transfer_balance(balances, from, to, amount).expect("never fails due to checks above");

    let allowances = state
        .allowances
        .get_mut(&from)
        .expect("allowance existing is checked above when check allowance sufficiency");
    let allowance = allowances
        .get_mut(&caller.into())
        .expect("allowance existing is checked above when check allowance sufficiency");
    *allowance = (*allowance - value_with_fee).expect("allowance sufficiency checked above");

    if *allowance == Tokens128::from(0u128) {
        allowances.remove(&caller.into());

        if allowances.is_empty() {
            state.allowances.remove(&from);
        }
    }

    let id = state
        .ledger
        .transfer_from(from, to, amount, fee, caller.into());
    Ok(id)
}

pub fn approve(
    canister: &impl TokenCanisterAPI,
    owner: AccountIdentifier,
    spender: AccountIdentifier,
    amount: Tokens128,
) -> TxReceipt {
    let state = canister.state();
    let mut state = state.borrow_mut();
    let CanisterState {
        ref mut bidding_state,
        ref mut balances,
        ref stats,
        ..
    } = &mut *state;

    let (fee, fee_to) = stats.fee_info();
    let fee_ratio = bidding_state.fee_ratio;
    if balances.balance_of(&owner) < fee {
        return Err(TxError::InsufficientBalance);
    }
    let fee_to = AccountIdentifier::from(fee_to);

    charge_fee(balances, owner, fee_to, fee, fee_ratio).expect("never fails due to checks above");
    let amount_with_fee = (amount + fee).ok_or(TxError::AmountOverflow)?;

    if amount_with_fee == Tokens128::from(0u128) {
        if let Some(allowances) = state.allowances.get_mut(&owner) {
            allowances.remove(&spender);
            if allowances.is_empty() {
                state.allowances.remove(&owner);
            }
        }
    } else {
        state
            .allowances
            .entry(owner)
            .or_default()
            .insert(spender, amount_with_fee);
    }

    let id = state.ledger.approve(owner, spender, amount, fee);
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

    let balance = state
        .balances
        .0
        .entry(AccountIdentifier::from(to))
        .or_default();
    let new_balance = (*balance + amount)
        .expect("balance cannot be larger than total_supply which is already checked");
    *balance = new_balance;

    let id = state.ledger.mint(caller.into(), to.into(), amount);

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

pub(crate) fn is20_mint_test_token(
    canister: &impl TokenCanisterAPI,
    caller: CheckedIdentifier<TestNetAid>,
    amount: Tokens128,
) -> TxReceipt {
    is20_mint(canister, caller.inner(), amount)
}

pub(crate) fn mint_as_owner(
    state: &mut CanisterState,
    caller: CheckedPrincipal<Owner>,
    to: Principal,
    amount: Tokens128,
) -> TxReceipt {
    mint(state, caller.inner(), to, amount)
}

pub(crate) fn is20_mint_as_owner(
    canister: &impl TokenCanisterAPI,
    caller: CheckedIdentifier<OwnerAid>,
    amount: Tokens128,
) -> TxReceipt {
    is20_mint(canister, caller.inner(), amount)
}

fn burn(
    state: &mut CanisterState,
    caller: Principal,
    from: Principal,
    amount: Tokens128,
) -> TxReceipt {
    match state
        .balances
        .0
        .get_mut(&AccountIdentifier::new(from, None))
    {
        Some(balance) => {
            *balance = (*balance - amount).ok_or(TxError::InsufficientBalance)?;
            if *balance == Tokens128::ZERO {
                state.balances.0.remove(&AccountIdentifier::from(from));
            }
        }
        None => {
            if !amount.is_zero() {
                return Err(TxError::InsufficientBalance);
            }
        }
    }

    state.stats.total_supply =
        (state.stats.total_supply - amount).expect("total supply cannot be less then user balance");

    let id = state.ledger.burn(caller.into(), from.into(), amount);
    Ok(id)
}

pub fn is20_mint(
    canister: &impl TokenCanisterAPI,
    to: AccountIdentifier,
    amount: Tokens128,
) -> TxReceipt {
    let state = canister.state();
    let mut state = state.borrow_mut();
    let caller = ic_canister::ic_kit::ic::caller();

    state.stats.total_supply =
        (state.stats.total_supply + amount).ok_or(TxError::AmountOverflow)?;

    let balance = state.balances.0.entry(to).or_default();
    let new_balance = (*balance + amount)
        .expect("balance cannot be larger than total_supply which is already checked");
    *balance = new_balance;

    let id = state.ledger.mint(caller.into(), to, amount);

    Ok(id)
}

pub fn is20_burn(
    canister: &impl TokenCanisterAPI,
    from: AccountIdentifier,
    amount: Tokens128,
) -> TxReceipt {
    let state = canister.state();
    let mut state = state.borrow_mut();
    match state.balances.0.get_mut(&from) {
        Some(balance) => {
            *balance = (*balance - amount).ok_or(TxError::InsufficientBalance)?;
            if *balance == Tokens128::ZERO {
                state.balances.0.remove(&from);
            }
        }
        None => {
            if !amount.is_zero() {
                return Err(TxError::InsufficientBalance);
            }
        }
    }

    state.stats.total_supply =
        (state.stats.total_supply - amount).expect("total supply cannot be less then user balance");

    let id = state.ledger.burn(from, from, amount);
    Ok(id)
}

pub fn burn_own_tokens(state: &mut CanisterState, amount: Tokens128) -> TxReceipt {
    let caller = ic_canister::ic_kit::ic::caller();
    burn(state, caller, caller, amount)
}

pub fn is20_burn_own_tokens(canister: &impl TokenCanisterAPI, amount: Tokens128) -> TxReceipt {
    let caller = ic_canister::ic_kit::ic::caller();
    is20_burn(canister, caller.into(), amount)
}

pub fn burn_as_owner(
    state: &mut CanisterState,
    caller: CheckedPrincipal<Owner>,
    from: Principal,
    amount: Tokens128,
) -> TxReceipt {
    burn(state, caller.inner(), from, amount)
}
pub fn is20_burn_as_owner(
    canister: &impl TokenCanisterAPI,
    caller: CheckedIdentifier<OwnerAid>,
    amount: Tokens128,
) -> TxReceipt {
    is20_burn(canister, caller.inner(), amount)
}

pub(crate) fn transfer_balance(
    balances: &mut Balances,
    from: AccountIdentifier,
    to: AccountIdentifier,
    amount: Tokens128,
) -> Result<(), TxError> {
    if amount == Tokens128::ZERO {
        return Ok(());
    }

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
    user: AccountIdentifier,
    fee_to: AccountIdentifier,
    fee: Tokens128,
    fee_ratio: f64,
) -> Result<(), TxError> {
    // todo: check if this is enforced
    debug_assert!((0.0..=1.0).contains(&fee_ratio));

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
    transfer_balance(
        balances,
        user,
        AccountIdentifier::from(auction_principal()),
        auction_fee_amount,
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::iter::FromIterator;

    use ic_canister::ic_kit::mock_principals::{alice, bob, john, xtc};
    use ic_canister::ic_kit::MockContext;
    use ic_canister::Canister;
    use rand::prelude::*;

    use crate::mock::*;
    use crate::types::AccountIdentifier;
    use crate::types::{Metadata, Operation, TransactionStatus};

    use super::*;

    fn test_context() -> (&'static MockContext, TokenCanisterMock) {
        let context = MockContext::new().with_caller(alice()).inject();

        let canister = TokenCanisterMock::init_instance();
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

        // This is to make tests that don't rely on auction state
        // pass, because since we are running auction state on each
        // endpoint call, it affects `BiddingInfo.fee_ratio` that is
        // used for charging fees in `approve` endpoint.
        canister.state.borrow_mut().stats.min_cycles = 0;

        (context, canister)
    }

    fn test_canister() -> TokenCanisterMock {
        let (_, canister) = test_context();
        canister
    }

    #[test]
    fn transfer_without_fee() {
        let canister = test_canister();
        assert_eq!(Tokens128::from(1000), canister.balanceOf(alice(), None));
        // let _ = canister.icrc1_transfer(bob(), None, Tokens128::from(100), None, None);
        let _ = canister.icrc1_transfer(None, bob(), None, Tokens128::from(100), None);
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(100));
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(900));
    }

    #[test]
    fn transfer_with_fee() {
        let canister = test_canister();
        canister.state().borrow_mut().stats.fee = Tokens128::from(100);
        canister.state().borrow_mut().stats.fee_to = john();

        assert!(canister
            .icrc1_transfer(None, bob(), None, Tokens128::from(200), None)
            .is_ok());
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(200));
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(700));
        assert_eq!(canister.balanceOf(john(), None), Tokens128::from(100));
    }

    #[test]
    fn transfer_fee_exceeded() {
        let canister = test_canister();
        canister.state().borrow_mut().stats.fee = Tokens128::from(100);
        canister.state().borrow_mut().stats.fee_to = john();

        assert!(canister
            .icrc1_transfer(
                None,
                bob(),
                None,
                Tokens128::from(200),
                Some(Tokens128::from(100)),
            )
            .is_ok());
        assert_eq!(
            canister.icrc1_transfer(
                None,
                bob(),
                None,
                Tokens128::from(200),
                Some(Tokens128::from(50)),
            ),
            Err(TxError::FeeExceededLimit)
        );
    }

    #[test]
    fn fees_with_auction_enabled() {
        let canister = test_canister();
        canister.state().borrow_mut().stats.fee = Tokens128::from(50);
        canister.state().borrow_mut().stats.fee_to = john();
        canister.state().borrow_mut().stats.min_cycles = crate::types::DEFAULT_MIN_CYCLES;
        canister.state().borrow_mut().bidding_state.fee_ratio = 0.5;

        canister
            .icrc1_transfer(None, bob(), None, Tokens128::from(100), None)
            .unwrap();
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(100));
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(850));
        assert_eq!(canister.balanceOf(john(), None), Tokens128::from(25));
        assert_eq!(
            canister.balanceOf(auction_principal(), None),
            Tokens128::from(25)
        );
    }

    #[test]
    fn transfer_insufficient_balance() {
        let canister = test_canister();
        assert_eq!(
            canister.icrc1_transfer(None, bob(), None, Tokens128::from(1001), None),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(1000));
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(0));
    }

    #[test]
    fn transfer_with_fee_insufficient_balance() {
        let canister = test_canister();
        canister.state().borrow_mut().stats.fee = Tokens128::from(100);
        canister.state().borrow_mut().stats.fee_to = john();

        assert_eq!(
            canister.icrc1_transfer(None, bob(), None, Tokens128::from(950), None),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(1000));
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(0));
    }

    #[test]
    fn transfer_wrong_caller() {
        let canister = test_canister();
        MockContext::new().with_caller(bob()).inject();
        assert_eq!(
            canister.icrc1_transfer(None, bob(), None, Tokens128::from(100), None),
            Err(TxError::SelfTransfer)
        );
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(1000));
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(0));
    }

    #[test]
    fn transfer_saved_into_history() {
        let (ctx, canister) = test_context();
        canister.state().borrow_mut().stats.fee = Tokens128::from(10);

        canister
            .icrc1_transfer(None, bob(), None, Tokens128::from(1001), None)
            .unwrap_err();
        assert_eq!(canister.historySize(), 1);

        const COUNT: u64 = 5;
        let mut ts = ic_canister::ic_kit::ic::time();
        for i in 0..COUNT {
            ctx.add_time(10);
            let id = canister
                .icrc1_transfer(None, bob(), None, Tokens128::from(100 + i as u128), None)
                .unwrap();
            assert_eq!(canister.historySize(), 2 + i);
            let tx = canister.getTransaction(id);
            assert_eq!(tx.amount, Tokens128::from(100 + i as u128));
            assert_eq!(tx.fee, Tokens128::from(10));
            assert_eq!(tx.operation, Operation::Transfer);
            assert_eq!(tx.status, TransactionStatus::Succeeded);
            assert_eq!(tx.index, i + 1);
            assert_eq!(tx.from, alice().into());
            assert_eq!(tx.to, bob().into());
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

        canister.state().borrow_mut().stats.is_test_token = true;

        assert!(canister.mint(alice(), Tokens128::from(2000)).is_ok());
        assert!(canister.mint(bob(), Tokens128::from(5000)).is_ok());
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(3000));
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(5000));
    }

    #[test]
    fn mint_by_owner() {
        let canister = test_canister();
        assert!(canister.mint(alice(), Tokens128::from(2000)).is_ok());
        assert!(canister.mint(bob(), Tokens128::from(5000)).is_ok());
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(3000));
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(5000));
        assert_eq!(canister.getMetadata().totalSupply, Tokens128::from(8000));
    }

    #[test]
    fn mint_saved_into_history() {
        let (ctx, canister) = test_context();
        canister.state().borrow_mut().stats.fee = Tokens128::from(10);

        assert_eq!(canister.historySize(), 1);

        const COUNT: u64 = 5;
        let mut ts = ic_canister::ic_kit::ic::time();
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
            assert_eq!(tx.from, alice().into());
            assert_eq!(tx.to, bob().into());

            assert!(ts < tx.timestamp);
            ts = tx.timestamp;
        }
    }

    #[test]
    fn burn_by_owner() {
        let canister = test_canister();
        assert!(canister.burn(None, Tokens128::from(100)).is_ok());
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(900));
        assert_eq!(canister.getMetadata().totalSupply, Tokens128::from(900));
    }

    #[test]
    fn burn_too_much() {
        let canister = test_canister();
        assert_eq!(
            canister.burn(None, Tokens128::from(1001)),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(1000));
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
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(1000));
        assert_eq!(canister.getMetadata().totalSupply, Tokens128::from(1000));
    }

    #[test]
    fn burn_from() {
        let canister = test_canister();
        let bob_balance = Tokens128::from(1000);
        canister.mint(bob(), bob_balance).unwrap();
        assert_eq!(canister.balanceOf(bob(), None), bob_balance);

        canister.burn(Some(bob()), Tokens128::from(100)).unwrap();
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(900));

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
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(1000));
        assert_eq!(canister.getMetadata().totalSupply, Tokens128::from(1000));
    }

    #[test]
    fn burn_saved_into_history() {
        let (ctx, canister) = test_context();
        canister.state().borrow_mut().stats.fee = Tokens128::from(10);

        canister.burn(None, Tokens128::from(1001)).unwrap_err();
        assert_eq!(canister.historySize(), 1);

        const COUNT: u64 = 5;
        let mut ts = ic_canister::ic_kit::ic::time();
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
            assert_eq!(tx.to, alice().into());
            assert_eq!(tx.from, alice().into());
            assert!(ts < tx.timestamp);
            ts = tx.timestamp;
        }
    }

    #[test]
    fn transfer_from_with_approve() {
        let canister = test_canister();
        let context = MockContext::new().with_caller(alice()).inject();
        assert!(canister
            .approve(bob(), None, Tokens128::from(500), None)
            .is_ok());
        context.update_caller(bob());

        assert!(canister
            .icrc1_transferFrom(alice(), None, john(), None, Tokens128::from(100))
            .is_ok());
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(900));
        assert_eq!(canister.balanceOf(john(), None), Tokens128::from(100));
        assert!(canister
            .icrc1_transferFrom(alice(), None, john(), None, Tokens128::from(100))
            .is_ok());
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(800));
        assert_eq!(canister.balanceOf(john(), None), Tokens128::from(200));
        assert!(canister
            .icrc1_transferFrom(alice(), None, john(), None, Tokens128::from(300))
            .is_ok());

        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(500));
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(0));
        assert_eq!(canister.balanceOf(john(), None), Tokens128::from(500));
    }

    #[test]
    fn insufficient_allowance() {
        let canister = test_canister();
        let context = MockContext::new().with_caller(alice()).inject();
        assert!(canister
            .approve(bob(), None, Tokens128::from(500), None)
            .is_ok());
        context.update_caller(bob());
        assert_eq!(
            canister.icrc1_transferFrom(alice(), None, john(), None, Tokens128::from(600)),
            Err(TxError::InsufficientAllowance)
        );
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(1000));
        assert_eq!(canister.balanceOf(john(), None), Tokens128::from(0));
    }

    #[test]
    fn transfer_from_without_approve() {
        let canister = test_canister();
        let context = MockContext::new().with_caller(alice()).inject();
        context.update_caller(bob());
        assert_eq!(
            canister.icrc1_transferFrom(alice(), None, john(), None, Tokens128::from(600)),
            Err(TxError::InsufficientAllowance)
        );
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(1000));
        assert_eq!(canister.balanceOf(john(), None), Tokens128::from(0));
    }

    #[test]
    fn transfer_from_saved_into_history() {
        let (ctx, canister) = test_context();
        let context = MockContext::new().with_caller(alice()).inject();
        canister.state().borrow_mut().stats.fee = Tokens128::from(10);

        canister
            .icrc1_transferFrom(bob(), None, john(), None, Tokens128::from(10))
            .unwrap_err();
        assert_eq!(canister.historySize(), 1);

        canister
            .approve(bob(), None, Tokens128::from(1000), None)
            .unwrap();
        context.update_caller(bob());

        const COUNT: u64 = 5;
        let mut ts = ic_canister::ic_kit::ic::time();
        for i in 0..COUNT {
            ctx.add_time(10);
            let id = canister
                .icrc1_transferFrom(
                    alice(),
                    None,
                    john(),
                    None,
                    Tokens128::from(100 + i as u128),
                )
                .unwrap();
            assert_eq!(canister.historySize(), 3 + i);
            let tx = canister.getTransaction(id);
            assert_eq!(tx.caller, Some(AccountIdentifier::from(bob())));
            assert_eq!(tx.amount, Tokens128::from(100 + i as u128));
            assert_eq!(tx.fee, Tokens128::from(10));
            assert_eq!(tx.operation, Operation::TransferFrom);
            assert_eq!(tx.status, TransactionStatus::Succeeded);
            assert_eq!(tx.index, i + 2);

            assert_eq!(tx.from, alice().into());
            assert_eq!(tx.to, john().into());
            assert!(ts < tx.timestamp);
            ts = tx.timestamp;
        }
    }

    #[test]
    fn multiple_approves() {
        let canister = test_canister();
        assert!(canister
            .approve(bob(), None, Tokens128::from(500), None)
            .is_ok());
        assert_eq!(
            canister.getUserApprovals(alice(), None),
            vec![(AccountIdentifier::from(bob()), Tokens128::from(500))]
        );

        assert!(canister
            .approve(bob(), None, Tokens128::from(200), None)
            .is_ok());
        assert_eq!(
            canister.getUserApprovals(alice(), None),
            vec![(AccountIdentifier::from(bob()), Tokens128::from(200))]
        );

        assert!(canister
            .approve(john(), None, Tokens128::from(1000), None)
            .is_ok());

        // Convert vectors to sets before comparing to make comparison unaffected by the element
        // order.
        assert_eq!(
            HashSet::<&(AccountIdentifier, Tokens128)>::from_iter(
                canister.getUserApprovals(alice(), None).iter()
            ),
            HashSet::from_iter(
                vec![
                    (bob().into(), Tokens128::from(200)),
                    (john().into(), Tokens128::from(1000)),
                ]
                .iter()
            )
        );
    }

    #[test]
    fn approve_over_balance() {
        let canister = test_canister();
        let context = MockContext::new().with_caller(alice()).inject();
        assert!(canister
            .approve(bob(), None, Tokens128::from(1500), None)
            .is_ok());

        context.update_caller(bob());
        let result = canister.icrc1_transferFrom(alice(), None, john(), None, Tokens128::from(500));
        println!("{:?}", result);
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(500));
        assert_eq!(canister.balanceOf(john(), None), Tokens128::from(500));

        assert_eq!(
            canister.icrc1_transferFrom(alice(), None, john(), None, Tokens128::from(600)),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(500));
        assert_eq!(canister.balanceOf(john(), None), Tokens128::from(500));
    }

    #[test]
    fn transfer_from_with_fee() {
        let canister = test_canister();
        canister.state().borrow_mut().stats.fee = Tokens128::from(100);
        canister.state().borrow_mut().stats.fee_to = bob();
        let context = MockContext::new().with_caller(alice()).inject();

        let result = canister.approve(bob(), None, Tokens128::from(1500), None);
        println!("{:?}", result);
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(100));
        context.update_caller(bob());

        assert!(canister
            .icrc1_transferFrom(alice(), None, john(), None, Tokens128::from(300))
            .is_ok());
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(200));
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(500));
        assert_eq!(canister.balanceOf(john(), None), Tokens128::from(300));
    }

    #[test]
    fn approve_saved_into_history() {
        let (ctx, canister) = test_context();
        canister.state().borrow_mut().stats.fee = Tokens128::from(10);
        assert_eq!(canister.historySize(), 1);

        const COUNT: u64 = 5;
        let mut ts = ic_canister::ic_kit::ic::time();
        for i in 0..COUNT {
            ctx.add_time(10);
            let id = canister
                .approve(bob(), None, Tokens128::from(100 + i as u128), None)
                .unwrap();
            assert_eq!(canister.historySize(), 2 + i);
            let tx = canister.getTransaction(id);
            assert_eq!(tx.amount, Tokens128::from(100 + i as u128));
            assert_eq!(tx.fee, Tokens128::from(10));
            assert_eq!(tx.operation, Operation::Approve);
            assert_eq!(tx.status, TransactionStatus::Succeeded);
            assert_eq!(tx.index, i + 1);

            assert_eq!(tx.from, alice().into());
            assert_eq!(tx.to, bob().into());
            assert!(ts < tx.timestamp);
            ts = tx.timestamp;
        }
    }

    #[test]
    fn get_transactions_test() {
        let canister = test_canister();

        for _ in 1..=5 {
            canister
                .icrc1_transfer(None, bob(), None, Tokens128::from(10), None)
                .unwrap();
        }

        canister
            .icrc1_transfer(None, bob(), None, Tokens128::from(10), None)
            .unwrap();
        canister
            .icrc1_transfer(None, xtc(), None, Tokens128::from(10), None)
            .unwrap();
        canister
            .icrc1_transfer(None, john(), None, Tokens128::from(10), None)
            .unwrap();

        assert_eq!(
            canister.getTransactions(None, None, 10, None).result.len(),
            9
        );
        assert_eq!(
            canister
                .getTransactions(None, None, 10, Some(3))
                .result
                .len(),
            4
        );
        assert_eq!(
            canister
                .getTransactions(Some(bob()), None, 10, None)
                .result
                .len(),
            6
        );
        assert_eq!(
            canister
                .getTransactions(Some(xtc()), None, 5, None)
                .result
                .len(),
            1
        );
        assert_eq!(
            canister
                .getTransactions(Some(alice()), None, 10, Some(5))
                .result
                .len(),
            6
        );
        assert_eq!(canister.getTransactions(None, None, 5, None).next, Some(3));
        assert_eq!(
            canister
                .getTransactions(Some(alice()), None, 3, Some(5))
                .next,
            Some(2)
        );
        assert_eq!(
            canister.getTransactions(Some(bob()), None, 3, Some(2)).next,
            None
        );

        for _ in 1..=10 {
            canister
                .icrc1_transfer(None, bob(), None, Tokens128::from(10), None)
                .unwrap();
        }

        let txn = canister.getTransactions(None, None, 5, None);
        assert_eq!(txn.result[0].index, 18);
        assert_eq!(txn.result[1].index, 17);
        assert_eq!(txn.result[2].index, 16);
        assert_eq!(txn.result[3].index, 15);
        assert_eq!(txn.result[4].index, 14);
        let txn2 = canister.getTransactions(None, None, 5, txn.next);
        assert_eq!(txn2.result[0].index, 13);
        assert_eq!(txn2.result[1].index, 12);
        assert_eq!(txn2.result[2].index, 11);
        assert_eq!(txn2.result[3].index, 10);
        assert_eq!(txn2.result[4].index, 9);
        assert_eq!(
            canister.getTransactions(None, None, 5, txn.next).next,
            Some(8)
        );
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
            canister
                .icrc1_transfer(None, bob(), None, Tokens128::from(10), None)
                .unwrap();
        }
        assert_eq!(canister.getUserTransactionCount(alice(), None), COUNT);
    }

    // Method for generating random Subaccount.
    fn gen_subaccount() -> Subaccount {
        // generate a random subaccount
        let mut subaccount = Subaccount([0u8; 32]);
        thread_rng().fill(&mut subaccount.0);
        subaccount
    }

    //     Test ICRC_TRANSFER
    #[test]
    fn is20_transfer_test() {
        let canister = test_canister();

        let bob_subaccount = gen_subaccount();
        let bob = AccountIdentifier::new(bob(), Some(bob_subaccount));

        let alice_subaccount = gen_subaccount();
        let alice_aid = AccountIdentifier::new(alice(), Some(alice_subaccount));
        let _ = canister.is20_mint(alice_aid, Tokens128::from(500)).unwrap();
        assert_eq!(
            canister.balanceOf(alice(), Some(alice_subaccount)),
            Tokens128::from(500)
        );

        let _ = canister
            .is20_transfer(Some(alice_subaccount), bob, Tokens128::from(200), None)
            .unwrap();
        let _ = canister
            .is20_transfer(None, bob, Tokens128::from(200), None)
            .unwrap();
        assert_eq!(
            canister.balanceOf(alice(), Some(alice_subaccount)),
            Tokens128::from(300)
        );
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(800));
    }
}

#[cfg(test)]
mod proptests {
    use ic_canister::ic_kit::MockContext;
    use ic_canister::Canister;
    use proptest::collection::vec;
    use proptest::prelude::*;
    use proptest::sample::Index;

    use crate::mock::*;
    use crate::types::Metadata;

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Action {
        Mint {
            minter: Principal,
            recipient: Principal,
            amount: Tokens128,
        },
        Burn(Tokens128, Principal),
        TransferWithFee {
            from: Principal,
            to: Principal,
            amount: Tokens128,
        },
        TransferWithoutFee {
            from: Principal,
            to: Principal,
            amount: Tokens128,
            fee_limit: Option<Tokens128>,
        },
        TransferFrom {
            caller: Principal,
            from: Principal,
            to: Principal,
            amount: Tokens128,
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
                make_tokens128(),
                select_principal(principals.clone()),
                select_principal(principals.clone()),
            )
                .prop_map(|(amount, minter, recipient)| Action::Mint {
                    minter,
                    recipient,
                    amount
                }),
            // Burn
            (make_tokens128(), select_principal(principals.clone()))
                .prop_map(|(amount, principal)| Action::Burn(amount, principal)),
            // With fee
            (
                select_principal(principals.clone()),
                select_principal(principals.clone()),
                make_tokens128()
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
                make_tokens128(),
                make_option(),
            )
                .prop_map(|(from, to, amount, fee_limit)| {
                    Action::TransferWithoutFee {
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
                select_principal(principals),
                make_tokens128()
            )
                .prop_map(|(principal, from, to, amount)| {
                    Action::TransferFrom {
                        caller: principal,
                        from,
                        to,
                        amount,
                    }
                })
        ]
    }

    fn make_option() -> impl Strategy<Value = Option<Tokens128>> {
        prop_oneof![Just(None), (make_tokens128()).prop_map(Some)]
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
        fn make_tokens128() (num in "[0-9]{1,10}") -> Tokens128 {
            Tokens128::from(u128::from_str_radix(&num, 10).unwrap())
        }
    }
    prop_compose! {
        fn make_canister() (
            logo in any::<String>(),
            name in any::<String>(),
            symbol in any::<String>(),
            decimals in any::<u8>(),
            total_supply in make_tokens128(),
            fee in make_tokens128(),
            principals in vec(make_principal(), 1..7),
            owner_idx in any::<Index>(),
            fee_to_idx in any::<Index>(),
        )-> (TokenCanisterMock, Vec<Principal>) {
            // pick two random principals (they could very well be the same principal twice)
            let owner = principals[owner_idx.index(principals.len())];
            let fee_to = principals[fee_to_idx.index(principals.len())];
            MockContext::new().with_caller(owner).inject();
            let meta = Metadata {
                logo,
                name,
                symbol,
                decimals,
                totalSupply: total_supply,
                owner,
                fee,
                feeTo: fee_to,
                isTestToken: None,
            };
            let canister = TokenCanisterMock::init_instance();
            canister.init(meta);
            // This is to make tests that don't rely on auction state
            // pass, because since we are running auction state on each
            // endpoint call, it affects `BiddingInfo.fee_ratio` that is
            // used for charging fees in `approve` endpoint.
            canister.state.borrow_mut().stats.min_cycles = 0;
            (canister, principals)
        }
    }
    fn canister_and_actions() -> impl Strategy<Value = (TokenCanisterMock, Vec<Action>)> {
        make_canister().prop_flat_map(|(canister, principals)| {
            let actions = vec(make_action(principals), 1..7);
            (Just(canister), actions)
        })
    }
    proptest! {
        #[test]
        fn generic_proptest((canister, actions) in canister_and_actions()) {
            let mut total_minted = Tokens128::ZERO;
            let mut total_burned = Tokens128::ZERO;
            let starting_supply = canister.totalSupply();
            for action in actions {
                use Action::*;
                match action {
                    Mint { minter, recipient, amount } => {
                        MockContext::new().with_caller(minter).inject();
                        let original = canister.totalSupply();
                        let res = canister.mint(recipient, amount);
                        let expected = if minter == canister.owner() {
                            total_minted = (total_minted + amount).unwrap();
                            assert!(matches!(res, Ok(_)));
                            (original + amount).unwrap()
                        } else {
                            assert_eq!(res, Err(TxError::Unauthorized));
                            original
                        };
                        assert_eq!(expected, canister.totalSupply());
                    },
                    Burn(amount, burner) => {
                        MockContext::new().with_caller(burner).inject();
                        let original = canister.totalSupply();
                        let balance = canister.balanceOf(burner,None);
                        let res = canister.burn(Some(burner), amount);
                        if balance < amount {
                            prop_assert_eq!(res, Err(TxError::InsufficientBalance));
                            prop_assert_eq!(original, canister.totalSupply());
                        } else {
                            prop_assert!(matches!(res, Ok(_)), "Burn error: {:?}. Balance: {}, amount: {}", res, balance, amount);
                            prop_assert_eq!((original - amount).unwrap(), canister.totalSupply());
                            total_burned = (total_burned + amount).unwrap();
                        }
                    },
                    TransferFrom { caller, from, to, amount } => {
                        MockContext::new().with_caller(caller).inject();
                        let from_balance = canister.balanceOf(from,None);
                        let to_balance = canister.balanceOf(to,None);
                        let (fee , _) = canister.state().borrow().stats.fee_info();
                        let amount_with_fee = (fee + amount).unwrap();
                        let res = canister.icrc1_transferFrom(from, None,to,None, amount);
                        let _ = canister.approve(from, None,amount,None);
                        let from_allowance = canister.allowance(from,None, caller,None);
                        if from == to {
                            prop_assert_eq!(res, Err(TxError::SelfTransfer));
                            return Ok(());
                        }

                        if from_allowance < amount_with_fee {
                            prop_assert_eq!(res, Err(TxError::InsufficientAllowance));
                            return Ok(());
                        }

                        if from_balance < amount_with_fee {
                            prop_assert_eq!(res, Err(TxError::InsufficientBalance));
                            prop_assert_eq!(from_balance, canister.balanceOf(from,None));

                            return Ok(());
                        }

                        prop_assert!(matches!(res, Ok(_)));
                        prop_assert_eq!((from_balance - amount_with_fee).unwrap(), canister.balanceOf(from,None));
                        prop_assert_eq!((to_balance + amount).unwrap(), canister.balanceOf(to,None));
                    },
                    TransferWithoutFee{from,to,amount,fee_limit} => {
                        MockContext::new().with_caller(from).inject();
                        let from_balance = canister.balanceOf(from, None);
                        let to_balance = canister.balanceOf(to, None);
                        let (fee , fee_to) = canister.state().borrow().stats.fee_info();
                        let amount_with_fee = (amount + fee).unwrap();
                        let res = canister.icrc1_transfer(None, to, None,amount, fee_limit);

                        if to == from {
                            prop_assert_eq!(res, Err(TxError::SelfTransfer));
                            return Ok(())
                        }

                        if let Some(fee_limit) = fee_limit {
                            if fee_limit < fee {
                                prop_assert_eq!(res, Err(TxError::FeeExceededLimit));
                                return Ok(())
                            }
                        }

                        if from_balance < amount_with_fee {
                            prop_assert_eq!(res, Err(TxError::InsufficientBalance));
                            return Ok(())
                        }

                        if fee_to == from  {
                            prop_assert!(matches!(res, Ok(_)));
                            prop_assert_eq!((from_balance - amount).unwrap(), canister.balanceOf(from, None));
                            return Ok(());
                        }

                        if fee_to == to  {
                            prop_assert!(matches!(res, Ok(_)));
                            prop_assert_eq!(((to_balance + amount).unwrap() + fee).unwrap(), canister.balanceOf(to,None));
                            return Ok(());
                        }

                        prop_assert!(matches!(res, Ok(_)));
                        prop_assert_eq!((from_balance - amount_with_fee).unwrap(), canister.balanceOf(from, None));
                        prop_assert_eq!((to_balance + amount).unwrap(), canister.balanceOf(to, None));

                    }
                    TransferWithFee { from, to, amount } => {
                        MockContext::new().with_caller(from).inject();
                        let from_balance = canister.balanceOf(from,None);
                        let to_balance = canister.balanceOf(to,None);
                        let (fee , fee_to) = canister.state().borrow().stats.fee_info();
                        let res = canister.icrc1_transferIncludeFee(None, to, None, amount);

                        if to == from {
                            prop_assert_eq!(res, Err(TxError::SelfTransfer));
                            return Ok(())
                        }

                        if amount <= fee  {
                            prop_assert_eq!(res, Err(TxError::AmountTooSmall));
                            return Ok(());
                        }
                        if from_balance < amount {
                            prop_assert_eq!(res, Err(TxError::InsufficientBalance));
                            return Ok(());
                        }

                        // Sometimes the fee can be sent `to` or `from`
                        if fee_to == from  {
                            prop_assert_eq!(((from_balance - amount).unwrap() + fee).unwrap(), canister.balanceOf(from,None));
                            return Ok(());
                        }

                        if fee_to == to  {
                            prop_assert_eq!((to_balance + amount).unwrap(), canister.balanceOf(to,None));
                            return Ok(());
                        }

                        prop_assert!(matches!(res, Ok(_)));
                        prop_assert_eq!(((to_balance + amount).unwrap() - fee).unwrap(), canister.balanceOf(to,None));
                        prop_assert_eq!((from_balance - amount).unwrap(), canister.balanceOf(from,None));

                    }
                }
            }
            prop_assert_eq!(((total_minted + starting_supply).unwrap() - total_burned).unwrap(), canister.totalSupply());
        }
    }
}
