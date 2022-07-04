use ic_helpers::tokens::Tokens128;

use crate::canister::erc20_transactions::{charge_fee, transfer_balance};
use crate::state::CanisterState;
use crate::types::{
    BatchTransferArgs, Subaccount, TokenHolder, TokenReceiver, TxError, TxId, TxReceipt,
};

use super::TokenCanisterAPI;

/// Transfers `value` amount to the `to` principal, applying American style fee. This means, that
/// the recipient will receive `value - fee`, and the sender account will be reduced exactly by `value`.
///
/// Note, that the `value` cannot be less than the `fee` amount. If the value given is too small,
/// transaction will fail with `TxError::AmountTooSmall` error.
pub fn transfer_include_fee(
    canister: &impl TokenCanisterAPI,
    from: TokenHolder,
    to: TokenReceiver,
    amount: Tokens128,
) -> TxReceipt {
    let caller = ic_canister::ic_kit::ic::caller();
    let state = canister.state();
    let mut state = state.borrow_mut();
    let CanisterState {
        ref mut balances,
        ref mut ledger,
        ref bidding_state,
        ref stats,
        ..
    } = *state;

    let (fee, fee_to) = stats.fee_info();
    let fee_ratio = bidding_state.fee_ratio;

    if amount <= fee {
        return Err(TxError::AmountTooSmall);
    }

    if balances.balance_of(&from) < amount {
        return Err(TxError::InsufficientBalance);
    }
    let fee_to = TokenReceiver::from(fee_to);

    charge_fee(balances, from, fee_to, fee, fee_ratio).expect("never fails due to checks above");
    transfer_balance(
        balances,
        from,
        to,
        (amount - fee).expect("amount > fee is checked above"),
    )
    .expect("never fails due to checks above");

    let id = ledger.transfer(from, to, amount, fee, caller);
    Ok(id)
}

pub fn batch_transfer(
    canister: &impl TokenCanisterAPI,
    from_subaccount: Option<Subaccount>,
    transfers: Vec<BatchTransferArgs>,
) -> Result<Vec<TxId>, TxError> {
    let caller = ic_canister::ic_kit::ic::caller();
    let from = TokenHolder::new(caller, from_subaccount);
    let state = canister.state();
    let mut state = state.borrow_mut();

    let mut total_value = Tokens128::from(0u128);
    for target in transfers.iter() {
        total_value = (total_value + target.amount).ok_or(TxError::AmountOverflow)?;
    }

    let CanisterState {
        ref mut balances,
        ref bidding_state,
        ref stats,
        ..
    } = &mut *state;

    let (fee, fee_to) = stats.fee_info();
    let fee_ratio = bidding_state.fee_ratio;

    let total_fee = (fee * transfers.len())
        .to_tokens128()
        .ok_or(TxError::AmountOverflow)?;

    if balances.balance_of(&from) < (total_value + total_fee).ok_or(TxError::AmountOverflow)? {
        return Err(TxError::InsufficientBalance);
    }

    let fee_to = TokenReceiver::from(fee_to);

    {
        for x in transfers.clone() {
            let value = x.amount;
            let to = TokenReceiver::new(x.reciever.to, x.reciever.to_subaccount);
            charge_fee(balances, from, fee_to, fee, fee_ratio)
                .expect("never fails due to checks above");
            transfer_balance(balances, from, to, value).expect("never fails due to checks above");
        }
    }

    let id = state.ledger.batch_transfer(from, transfers, fee, caller);
    Ok(id)
}

#[cfg(test)]
mod tests {
    use ic_canister::ic_kit::mock_principals::{alice, bob, john, xtc};
    use ic_canister::ic_kit::MockContext;
    use ic_canister::Canister;

    use crate::mock::TokenCanisterMock;
    use crate::types::{BatchAccount, Metadata};

    use super::*;

    fn test_canister() -> TokenCanisterMock {
        MockContext::new().with_caller(alice()).inject();

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

        canister
    }

    #[test]
    fn batch_transfer_without_fee() {
        let canister = test_canister();
        assert_eq!(Tokens128::from(1000), canister.balanceOf(alice(), None));
        let transfer1 = BatchTransferArgs {
            reciever: BatchAccount {
                to: bob(),
                to_subaccount: None,
            },
            amount: Tokens128::from(100),
        };
        let transfer2 = BatchTransferArgs {
            reciever: BatchAccount {
                to: john(),
                to_subaccount: None,
            },
            amount: Tokens128::from(200),
        };
        let receipt = canister
            .batchTransfer(None, vec![transfer1, transfer2])
            .unwrap();
        assert_eq!(receipt.len(), 2);
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(700));
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(100));
        assert_eq!(canister.balanceOf(john(), None), Tokens128::from(200));
    }

    #[test]
    fn batch_transfer_with_fee() {
        let canister = test_canister();
        let mut state = canister.state.borrow_mut();
        state.stats.fee = Tokens128::from(50);
        state.stats.fee_to = john();
        drop(state);
        assert_eq!(Tokens128::from(1000), canister.balanceOf(alice(), None));
        let transfer1 = BatchTransferArgs {
            reciever: BatchAccount {
                to: bob(),
                to_subaccount: None,
            },
            amount: Tokens128::from(100),
        };
        let transfer2 = BatchTransferArgs {
            reciever: BatchAccount {
                to: xtc(),
                to_subaccount: None,
            },
            amount: Tokens128::from(200),
        };
        let receipt = canister
            .batchTransfer(None, vec![transfer1, transfer2])
            .unwrap();
        assert_eq!(receipt.len(), 2);
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(600));
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(100));
        assert_eq!(canister.balanceOf(xtc(), None), Tokens128::from(200));
        assert_eq!(canister.balanceOf(john(), None), Tokens128::from(100));
    }

    #[test]
    fn batch_transfer_insufficient_balance() {
        let canister = test_canister();

        let transfer1 = BatchTransferArgs {
            reciever: BatchAccount {
                to: bob(),
                to_subaccount: None,
            },
            amount: Tokens128::from(500),
        };
        let transfer2 = BatchTransferArgs {
            reciever: BatchAccount {
                to: john(),
                to_subaccount: None,
            },
            amount: Tokens128::from(600),
        };
        let receipt = canister.batchTransfer(None, vec![transfer1, transfer2]);
        assert!(receipt.is_err());
        assert_eq!(receipt.unwrap_err(), TxError::InsufficientBalance);
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(1000));
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(0));
        assert_eq!(canister.balanceOf(john(), None), Tokens128::from(0));
    }

    #[test]
    fn transfer_without_fee() {
        let canister = test_canister();
        assert_eq!(Tokens128::from(1000), canister.balanceOf(alice(), None));

        assert!(canister
            .transferIncludeFee(bob(), None, None, Tokens128::from(100))
            .is_ok());
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(100));
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(900));
    }

    #[test]
    fn transfer_with_fee() {
        let canister = test_canister();

        let mut state = canister.state.borrow_mut();
        state.stats.fee = Tokens128::from(100);
        state.stats.fee_to = john();
        drop(state);

        assert!(canister
            .transferIncludeFee(bob(), None, None, Tokens128::from(200))
            .is_ok());
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(100));
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(800));
        assert_eq!(canister.balanceOf(john(), None), Tokens128::from(100));
    }

    #[test]
    fn transfer_insufficient_balance() {
        let canister = test_canister();
        assert_eq!(
            canister.transferIncludeFee(bob(), None, None, Tokens128::from(1001)),
            Err(TxError::InsufficientBalance)
        );
        assert_eq!(canister.balanceOf(alice(), None), Tokens128::from(1000));
        assert_eq!(canister.balanceOf(bob(), None), Tokens128::from(0));
    }
}
