use crate::api::dip20_meta::balance_of;
use crate::api::dip20_transactions::{_charge_fee, _transfer};
use crate::state::{BiddingState, State};
use crate::types::{TxError, TxReceipt};
use candid::{candid_method, Nat, Principal};
use ic_cdk_macros::update;
use ic_kit::ic;
use ic_storage::IcStorage;

/// Transfers `value` amount to the `to` principal, applying American style fee. This means, that
/// the recipient will receive `value - fee`, and the sender account will be reduced exactly by `value`.
///
/// Note, that the `value` cannot be less than the `fee` amount. If the value given is too small,
/// transaction will fail with `TxError::AmountTooSmall` error.
#[update(name = "transferIncludeFee")]
#[candid_method(update, rename = "transferIncludeFee")]
fn transfer_include_fee(to: Principal, value: Nat) -> TxReceipt {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{init_context, init_with_fee};
    use ic_kit::mock_principals::{alice, bob, john};
    use ic_kit::MockContext;

    #[test]
    fn transfer_without_fee() {
        init_context();
        assert_eq!(Nat::from(1000), balance_of(alice()));

        assert!(transfer_include_fee(bob(), Nat::from(100)).is_ok());
        assert_eq!(balance_of(bob()), Nat::from(100));
        assert_eq!(balance_of(alice()), Nat::from(900));
    }

    #[test]
    fn transfer_with_fee() {
        MockContext::new().with_caller(alice()).inject();
        init_with_fee();

        assert!(transfer_include_fee(bob(), Nat::from(200)).is_ok());
        assert_eq!(balance_of(bob()), Nat::from(100));
        assert_eq!(balance_of(alice()), Nat::from(800));
        assert_eq!(balance_of(john()), Nat::from(100));
    }

    #[test]
    fn transfer_insufficient_balance() {
        init_context();
        assert_eq!(
            transfer_include_fee(bob(), Nat::from(1001)),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(balance_of(alice()), Nat::from(1000));
        assert_eq!(balance_of(bob()), Nat::from(0));
    }
}
