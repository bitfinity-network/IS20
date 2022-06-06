use crate::canister::erc20_transactions::{_charge_fee, _transfer};
use crate::canister::TokenCanister;
use crate::principal::{CheckedPrincipal, WithRecipient};
use crate::state::CanisterState;
use crate::types::{TxError, TxReceipt};
use candid::{Nat, Principal};

/// Transfers `value` amount to the `to` principal, applying American style fee. This means, that
/// the recipient will receive `value - fee`, and the sender account will be reduced exactly by `value`.
///
/// Note, that the `value` cannot be less than the `fee` amount. If the value given is too small,
/// transaction will fail with `TxError::AmountTooSmall` error.
pub fn transfer_include_fee(
    canister: &TokenCanister,
    caller: CheckedPrincipal<WithRecipient>,
    value: Nat,
) -> TxReceipt {
    let CanisterState {
        ref mut balances,
        ref mut ledger,
        ref bidding_state,
        ref stats,
        ..
    } = *canister.state.borrow_mut();

    let (fee, fee_to) = stats.fee_info();
    let fee_ratio = bidding_state.fee_ratio;

    if value <= fee {
        return Err(TxError::AmountTooSmall);
    }

    if balances.balance_of(&caller.inner()) < value {
        return Err(TxError::InsufficientBalance);
    }

    _charge_fee(balances, caller.inner(), fee_to, fee.clone(), fee_ratio);
    _transfer(
        balances,
        caller.inner(),
        caller.recipient(),
        value.clone() - fee.clone(),
    );

    let id = ledger.transfer(caller.inner(), caller.recipient(), value, fee);
    Ok(id)
}

pub fn batch_transfer(
    canister: &TokenCanister,
    transfers: Vec<(Principal, Nat)>,
) -> Result<Vec<Nat>, TxError> {
    let from = ic_canister::ic_kit::ic::caller();
    let mut state = canister.state.borrow_mut();

    let total_value = transfers
        .iter()
        .map(|(_, value)| value.clone())
        .fold(Nat::from(0), |acc, val| acc + val);

    let CanisterState {
        ref mut balances,
        ref bidding_state,
        ref stats,
        ..
    } = &mut *state;

    let (fee, fee_to) = stats.fee_info();
    let fee_ratio = bidding_state.fee_ratio;

    let total_fee = fee.clone() * transfers.len() as u64;

    if balances.balance_of(&from) < total_value + total_fee {
        return Err(TxError::InsufficientBalance);
    }

    {
        for (to, value) in transfers.clone() {
            _charge_fee(balances, from, fee_to, fee.clone(), fee_ratio);
            _transfer(balances, from, to, value.clone());
        }
    }

    let id = state.ledger.batch_transfer(from, transfers, fee);
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::types::Metadata;
    use ic_canister::Canister;
    use ic_canister::ic_kit::mock_principals::{alice, bob, john, xtc};
    use ic_canister::ic_kit::MockContext;

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
    fn batch_transfer_without_fee() {
        let canister = test_canister();
        assert_eq!(Nat::from(1000), canister.balanceOf(alice()));
        let transfers = vec![(bob(), Nat::from(100)), (john(), Nat::from(200))];
        let receipt = canister.batchTransfer(transfers).unwrap();
        assert_eq!(receipt.len(), 2);
        assert_eq!(canister.balanceOf(alice()), Nat::from(700));
        assert_eq!(canister.balanceOf(bob()), Nat::from(100));
        assert_eq!(canister.balanceOf(john()), Nat::from(200));
    }

    #[test]
    fn batch_transfer_with_fee() {
        let canister = test_canister();
        let mut state = canister.state.borrow_mut();
        state.stats.fee = Nat::from(50);
        state.stats.fee_to = john();
        drop(state);
        assert_eq!(Nat::from(1000), canister.balanceOf(alice()));
        let transfers = vec![(bob(), Nat::from(100)), (xtc(), Nat::from(200))];
        let receipt = canister.batchTransfer(transfers).unwrap();
        assert_eq!(receipt.len(), 2);
        assert_eq!(canister.balanceOf(alice()), Nat::from(600));
        assert_eq!(canister.balanceOf(bob()), Nat::from(100));
        assert_eq!(canister.balanceOf(xtc()), Nat::from(200));
        assert_eq!(canister.balanceOf(john()), Nat::from(100));
    }

    #[test]
    fn batch_transfer_insufficient_balance() {
        let canister = test_canister();
        let transfers = vec![(bob(), Nat::from(500)), (john(), Nat::from(600))];
        let receipt = canister.batchTransfer(transfers);
        assert!(receipt.is_err());
        assert_eq!(receipt.unwrap_err(), TxError::InsufficientBalance);
        assert_eq!(canister.balanceOf(alice()), Nat::from(1000));
        assert_eq!(canister.balanceOf(bob()), Nat::from(0));
        assert_eq!(canister.balanceOf(john()), Nat::from(0));
    }

    #[test]
    fn transfer_without_fee() {
        let canister = test_canister();
        assert_eq!(Nat::from(1000), canister.balanceOf(alice()));

        assert!(canister.transferIncludeFee(bob(), Nat::from(100)).is_ok());
        assert_eq!(canister.balanceOf(bob()), Nat::from(100));
        assert_eq!(canister.balanceOf(alice()), Nat::from(900));
    }

    #[test]
    fn transfer_with_fee() {
        let canister = test_canister();

        let mut state = canister.state.borrow_mut();
        state.stats.fee = Nat::from(100);
        state.stats.fee_to = john();
        drop(state);

        assert!(canister.transferIncludeFee(bob(), Nat::from(200)).is_ok());
        assert_eq!(canister.balanceOf(bob()), Nat::from(100));
        assert_eq!(canister.balanceOf(alice()), Nat::from(800));
        assert_eq!(canister.balanceOf(john()), Nat::from(100));
    }

    #[test]
    fn transfer_insufficient_balance() {
        let canister = test_canister();
        assert_eq!(
            canister.transferIncludeFee(bob(), Nat::from(1001)),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice()), Nat::from(1000));
        assert_eq!(canister.balanceOf(bob()), Nat::from(0));
    }
}

