use crate::api::dip20_transactions::transfer;
use crate::state::State;
use crate::types::{TxError, TxReceipt};
use candid::{candid_method, Nat, Principal};
use ic_cdk_macros::update;
use ic_storage::IcStorage;

/// Transfers the `value` amount of tokens to the `to` principal, using European style fee (the fee
/// amount is added to the `value` amount, so the total amount reduced from the sender balance is
/// `value + fee`).
///
/// If the `limit` value is provided and the amount `value + fee` exceeds this limit, an error
/// `InsufficientAllowance` will be returned. This parameter can be used to guard against sudden
/// large fee value change by the owner.
#[update(name = "transferWithFee")]
#[candid_method(update, rename = "transferWithFee")]
fn transfer_with_fee(to: Principal, value: Nat, limit: Option<Nat>) -> TxReceipt {
    let state = State::get();
    let state = state.borrow();
    let stats = state.stats();
    let fee = stats.fee.clone();
    if let Some(limit) = limit {
        if value.clone() + fee.clone() > limit {
            return Err(TxError::InsufficientAllowance);
        }
    }

    drop(state);

    transfer(to, value + fee)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::dip20_meta::balance_of;
    use crate::tests::canister_init_with_fee;
    use crate::types::TxError;
    use ic_kit::mock_principals::{alice, bob, john};
    use ic_kit::MockContext;

    #[test]
    fn transfer_with_fee_test() {
        MockContext::new().with_caller(alice()).inject();
        canister_init_with_fee();

        assert!(transfer_with_fee(bob(), Nat::from(200), None).is_ok());
        assert_eq!(balance_of(bob()), Nat::from(200));
        assert_eq!(balance_of(alice()), Nat::from(700));
        assert_eq!(balance_of(john()), Nat::from(100));
    }

    #[test]
    fn transfer_with_fee_not_enough() {
        MockContext::new().with_caller(alice()).inject();
        canister_init_with_fee();

        assert_eq!(
            transfer_with_fee(bob(), Nat::from(950), None),
            Err(TxError::InsufficientBalance)
        );
    }

    #[test]
    fn transfer_with_fee_limit() {
        MockContext::new().with_caller(alice()).inject();
        canister_init_with_fee();

        assert_eq!(
            transfer_with_fee(bob(), Nat::from(100), Some(Nat::from(150))),
            Err(TxError::InsufficientAllowance)
        );

        assert!(transfer_with_fee(bob(), Nat::from(100), Some(Nat::from(200))).is_ok());
    }
}
