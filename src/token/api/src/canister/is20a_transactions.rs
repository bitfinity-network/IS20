use candid::Principal;
use ic_helpers::tokens::Tokens128;

use crate::canister::erc20_transactions::{charge_fee, transfer_balance};
use crate::canister::TokenCanisterAPI;
use crate::state::CanisterState;
use crate::types::{AccountIdentifier, Subaccount, TxError, TxId, TxReceipt};

// -----------------------ICRC_TRANSACTIONS----------------------------------- //

pub fn is20_transfer(
    canister: &impl TokenCanisterAPI,
    from_subaccount: Option<Subaccount>,
    to: AccountIdentifier,
    amount: Tokens128,
    fee_limit: Option<Tokens128>,
) -> TxReceipt {
    let from = AccountIdentifier::new(ic_canister::ic_kit::ic::caller(), from_subaccount);
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

#[cfg(test)]
mod tests {
    use ic_canister::ic_kit::mock_principals::{alice, bob};
    use ic_canister::ic_kit::MockContext;
    use ic_canister::Canister;

    use crate::mock::*;
    use crate::types::{AccountIdentifier, Metadata};

    use super::*;
    use rand::prelude::*;

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

        (context, canister)
    }

    fn test_canister() -> TokenCanisterMock {
        let (_, canister) = test_context();
        canister
    }
    // Method for generating random Subaccount.
    fn gen_subaccount() -> Subaccount {
        // generate a random subaccount
        let mut subaccount = Subaccount([0u8; 32]);
        rand::thread_rng().fill(&mut subaccount.0);
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
        let _ = is20_mint(&canister, alice_aid, Tokens128::from(500));
        assert_eq!(
            canister.balanceOf(alice(), Some(alice_subaccount)),
            Tokens128::from(500)
        );

        let _ = is20_transfer(
            &canister,
            Some(alice_subaccount),
            bob,
            Tokens128::from(200),
            None,
        )
        .unwrap();
        let _ = is20_transfer(&canister, None, bob, Tokens128::from(200), None).unwrap();
        assert_eq!(
            canister.balanceOf(alice(), Some(alice_subaccount)),
            Tokens128::from(300)
        );
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(800));
    }
}
