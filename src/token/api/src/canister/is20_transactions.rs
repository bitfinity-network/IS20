use canister_sdk::ic_helpers::tokens::Tokens128;
use canister_sdk::ic_kit::ic;
#[cfg(feature = "claim")]
use canister_sdk::ledger::{AccountIdentifier, Subaccount as SubaccountIdentifier};
use ic_exports::Principal;

use super::auction_account;
use super::icrc1_transfer::{PERMITTED_DRIFT, TX_WINDOW};
use crate::account::{AccountInternal, CheckedAccount, Subaccount, WithRecipient};
use crate::error::TxError;
use crate::principal::{CheckedPrincipal, Owner, TestNet};
use crate::state::balances::{Balances, LocalBalances, StableBalances};
use crate::state::config::{FeeRatio, TokenConfig};
use crate::state::ledger::{BatchTransferArgs, LedgerData, TransferArgs, TxReceipt};
use crate::tx_record::TxId;

pub fn is20_transfer(
    caller: CheckedAccount<WithRecipient>,
    transfer: &TransferArgs,
    auction_fee_ratio: f64,
) -> TxReceipt {
    let from = caller.inner();
    let to = caller.recipient();
    let created_at_time = validate_and_get_tx_ts(from.owner, transfer)?;
    let TransferArgs { amount, memo, .. } = transfer;

    let stats = TokenConfig::get_stable();
    let (fee, fee_to) = stats.fee_info();

    if let Some(requested_fee) = transfer.fee {
        if fee != requested_fee {
            return Err(TxError::BadFee { expected_fee: fee });
        }
    }

    transfer_internal(
        &mut StableBalances,
        from,
        to,
        *amount,
        fee,
        fee_to.into(),
        FeeRatio::new(auction_fee_ratio),
    )?;

    let id = LedgerData::transfer(from, to, *amount, fee, *memo, created_at_time);
    Ok(id.into())
}

pub(crate) fn transfer_internal(
    balances: &mut impl Balances,
    from: AccountInternal,
    to: AccountInternal,
    amount: Tokens128,
    fee: Tokens128,
    fee_to: AccountInternal,
    auction_fee_ratio: FeeRatio,
) -> Result<(), TxError> {
    if amount.is_zero() {
        return Err(TxError::AmountTooSmall);
    }

    // We use `updates` structure because sometimes from or to can be equal to fee_to or even to
    // auction_account, so we must take a carefull approach.
    let mut updates = LocalBalances::from_iter([
        (from, balances.balance_of(&from)),
        (to, balances.balance_of(&to)),
        (fee_to, balances.balance_of(&fee_to)),
        (auction_account(), balances.balance_of(&auction_account())),
    ]);

    // If `amount + fee` overflows max `Tokens128` value, the balance cannot be larger than this
    // value, so we can safely return `InsufficientFunds` error.
    let amount_with_fee = (amount + fee).ok_or(TxError::InsufficientFunds {
        balance: updates.balance_of(&from),
    })?;

    let updated_from_balance =
        (updates.balance_of(&from) - amount_with_fee).ok_or(TxError::InsufficientFunds {
            balance: updates.balance_of(&from),
        })?;
    updates.insert(from, updated_from_balance);

    let updated_to_balance = (updates.balance_of(&to) + amount).ok_or(TxError::AmountOverflow)?;
    updates.insert(to, updated_to_balance);

    let (owner_fee, auction_fee) = auction_fee_ratio.get_value(fee);

    let updated_fee_to_balance =
        (updates.balance_of(&fee_to) + owner_fee).ok_or(TxError::AmountOverflow)?;
    updates.insert(fee_to, updated_fee_to_balance);

    let updated_auction_balance =
        (updates.balance_of(&auction_account()) + auction_fee).ok_or(TxError::AmountOverflow)?;
    updates.insert(auction_account(), updated_auction_balance);

    // At this point all the checks are done and no further errors are possible, so we modify the
    // canister state only at this point.
    balances.apply_updates(updates.list_balances(0, usize::MAX));

    Ok(())
}

fn validate_and_get_tx_ts(caller: Principal, transfer_args: &TransferArgs) -> Result<u64, TxError> {
    let now = ic::time();
    let from = AccountInternal::new(caller, transfer_args.from_subaccount);
    let to = transfer_args.to.into();

    let created_at_time = match transfer_args.created_at_time {
        Some(created_at_time) => {
            if now.saturating_sub(created_at_time) > TX_WINDOW {
                return Err(TxError::TooOld {
                    allowed_window_nanos: TX_WINDOW,
                });
            }

            if created_at_time.saturating_sub(now) > PERMITTED_DRIFT {
                return Err(TxError::CreatedInFuture { ledger_time: now });
            }

            let txs = LedgerData::list_transactions();
            for tx in txs.iter().rev() {
                if now.saturating_sub(tx.timestamp) > TX_WINDOW + PERMITTED_DRIFT {
                    break;
                }

                if tx.timestamp == created_at_time
                    && AccountInternal::from(tx.from) == from
                    && AccountInternal::from(tx.to) == to
                    && tx.memo == transfer_args.memo
                    && tx.amount == transfer_args.amount
                    && tx.fee == transfer_args.fee.unwrap_or(tx.fee)
                {
                    return Err(TxError::Duplicate {
                        duplicate_of: tx.index,
                    });
                }
            }

            created_at_time
        }

        None => now,
    };

    Ok(created_at_time)
}

pub fn mint(caller: Principal, to: AccountInternal, amount: Tokens128) -> TxReceipt {
    let total_supply = StableBalances.total_supply();
    if (total_supply + amount).is_none() {
        // If we allow to mint more then Tokens128::MAX then simple operations such as getting
        // total supply or token stats will panic, So we add this check to prevent this.
        return Err(TxError::AmountOverflow);
    }

    let balance = StableBalances.balance_of(&to);
    let new_balance = (balance + amount).ok_or(TxError::AmountOverflow)?;
    StableBalances.insert(to, new_balance);

    let id = LedgerData::mint(caller.into(), to, amount);

    Ok(id.into())
}

pub fn mint_test_token(
    caller: CheckedPrincipal<TestNet>,
    to: Principal,
    to_subaccount: Option<Subaccount>,
    amount: Tokens128,
) -> TxReceipt {
    mint(
        caller.inner(),
        AccountInternal::new(to, to_subaccount),
        amount,
    )
}

pub fn mint_as_owner(
    caller: CheckedPrincipal<Owner>,
    to: Principal,
    to_subaccount: Option<Subaccount>,
    amount: Tokens128,
) -> TxReceipt {
    mint(
        caller.inner(),
        AccountInternal::new(to, to_subaccount),
        amount,
    )
}

pub fn burn(caller: Principal, from: AccountInternal, amount: Tokens128) -> TxReceipt {
    let balance = StableBalances.balance_of(&from);

    if !amount.is_zero() && balance.is_zero() {
        return Err(TxError::InsufficientFunds { balance });
    }

    let new_balance = (balance - amount).ok_or(TxError::InsufficientFunds { balance })?;

    if new_balance == Tokens128::ZERO {
        StableBalances.remove(&from);
    } else {
        StableBalances.insert(from, new_balance)
    }

    let id = LedgerData::burn(caller.into(), from, amount);
    Ok(id.into())
}

pub fn burn_own_tokens(from_subaccount: Option<Subaccount>, amount: Tokens128) -> TxReceipt {
    let caller = ic::caller();
    burn(
        caller,
        AccountInternal::new(caller, from_subaccount),
        amount,
    )
}

pub fn burn_as_owner(
    caller: CheckedPrincipal<Owner>,
    from: Principal,
    from_subaccount: Option<Subaccount>,
    amount: Tokens128,
) -> TxReceipt {
    burn(
        caller.inner(),
        AccountInternal::new(from, from_subaccount),
        amount,
    )
}

#[cfg(feature = "claim")]
pub fn get_claim_subaccount(
    claimer: Principal,
    claimer_subaccount: Option<Subaccount>,
) -> Subaccount {
    let account_id = AccountIdentifier::new(
        claimer.into(),
        Some(SubaccountIdentifier(claimer_subaccount.unwrap_or_default())),
    );

    account_id.to_address()
}

#[cfg(feature = "claim")]
pub fn claim(holder: Principal, subaccount: Option<Subaccount>) -> TxReceipt {
    let caller = canister_sdk::ic_kit::ic::caller();
    let claim_subaccount = get_claim_subaccount(caller, subaccount);
    let claim_account = AccountInternal::new(holder, Some(claim_subaccount));
    let amount = StableBalances.balance_of(&claim_account);
    if amount.is_zero() {
        return Err(TxError::NothingToClaim);
    }

    let stats = TokenConfig::get_stable();
    transfer_internal(
        &mut StableBalances,
        claim_account,
        caller.into(),
        amount,
        0.into(),
        stats.owner.into(),
        FeeRatio::default(),
    )?;
    let id = LedgerData::claim(claim_account, AccountInternal::new(caller, None), amount);
    Ok(id.into())
}

pub fn batch_transfer(
    from_subaccount: Option<Subaccount>,
    transfers: Vec<BatchTransferArgs>,
    auction_fee_ratio: f64,
) -> Result<Vec<TxId>, TxError> {
    let caller = canister_sdk::ic_kit::ic::caller();
    let from = AccountInternal::new(caller, from_subaccount);

    let stats = TokenConfig::get_stable();
    let (fee, fee_to) = stats.fee_info();

    batch_transfer_internal(
        from,
        &transfers,
        &mut StableBalances,
        fee,
        fee_to,
        auction_fee_ratio,
    )?;
    let id = LedgerData::batch_transfer(from, transfers, fee);
    Ok(id)
}

pub(crate) fn batch_transfer_internal(
    from: AccountInternal,
    transfers: &Vec<BatchTransferArgs>,
    balances: &mut impl Balances,
    fee: Tokens128,
    fee_to: Principal,
    auction_fee_ratio: f64,
) -> Result<(), TxError> {
    let fee_to = AccountInternal::new(fee_to, None);
    let auction_acc = auction_account();

    let mut updates = LocalBalances::from_iter([
        (from, balances.balance_of(&from)),
        (fee_to, balances.balance_of(&fee_to)),
        (auction_acc, balances.balance_of(&auction_acc)),
    ]);

    for transfer in transfers {
        let receiver = transfer.receiver.into();
        updates.insert(receiver, balances.balance_of(&receiver));
    }

    for transfer in transfers {
        let receiver = transfer.receiver.into();
        transfer_internal(
            &mut updates,
            from,
            receiver,
            transfer.amount,
            fee,
            fee_to,
            FeeRatio::new(auction_fee_ratio),
        )
        .map_err(|err| match err {
            TxError::InsufficientFunds { .. } => TxError::InsufficientFunds {
                balance: balances.balance_of(&from),
            },
            other => other,
        })?;
    }

    balances.apply_updates(updates.list_balances(0, usize::MAX));
    Ok(())
}

#[cfg(test)]
mod tests {
    use canister_sdk::ic_auction::api::Auction;
    use canister_sdk::ic_canister::Canister;
    use canister_sdk::ic_kit::inject::get_context;
    use canister_sdk::ic_kit::mock_principals::{alice, bob, john, xtc};
    use canister_sdk::ic_kit::MockContext;
    use coverage_helper::test;

    use super::*;
    use crate::account::{Account, DEFAULT_SUBACCOUNT};
    use crate::canister::TokenCanisterAPI;
    use crate::mock::TokenCanisterMock;
    use crate::state::config::Metadata;

    fn test_canister() -> TokenCanisterMock {
        let context = MockContext::new().with_caller(alice()).inject();

        let principal = Principal::from_text("mfufu-x6j4c-gomzb-geilq").unwrap();
        let canister = TokenCanisterMock::from_principal(principal);
        context.update_id(canister.principal());

        // Refresh canister's state.
        TokenConfig::set_stable(TokenConfig::default());
        StableBalances.clear();
        LedgerData::clear();

        canister.init(
            Metadata {
                name: "".to_string(),
                symbol: "".to_string(),
                decimals: 8,
                owner: alice(),
                fee: Tokens128::from(0),
                fee_to: alice(),
                is_test_token: None,
            },
            Tokens128::from(1000),
        );

        // This is to make tests that don't rely on auction state
        // pass, because since we are running auction state on each
        // endpoint call, it affects `BiddingInfo.fee_ratio` that is
        // used for charging fees in `approve` endpoint.
        let mut stats = TokenConfig::get_stable();
        stats.min_cycles = 0;
        TokenConfig::set_stable(stats);

        canister
    }

    #[test]
    fn batch_transfer_without_fee() {
        let canister = test_canister();
        assert_eq!(
            Tokens128::from(1000),
            canister.icrc1_balance_of(Account::new(alice(), None))
        );
        let transfer1 = BatchTransferArgs {
            receiver: Account::new(bob(), None),
            amount: Tokens128::from(100),
        };
        let transfer2 = BatchTransferArgs {
            receiver: Account::new(john(), None),
            amount: Tokens128::from(200),
        };
        let receipt = canister
            .batch_transfer(None, vec![transfer1, transfer2])
            .unwrap();
        assert_eq!(receipt.len(), 2);
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(700)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), None)),
            Tokens128::from(100)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(john(), None)),
            Tokens128::from(200)
        );
    }

    #[test]
    fn batch_transfer_with_fee() {
        let canister = test_canister();

        let mut stats = TokenConfig::get_stable();
        stats.fee = Tokens128::from(50);
        stats.fee_to = john();
        TokenConfig::set_stable(stats);

        assert_eq!(
            Tokens128::from(1000),
            canister.icrc1_balance_of(Account::new(alice(), None))
        );
        let transfer1 = BatchTransferArgs {
            receiver: Account::new(bob(), None),
            amount: Tokens128::from(100),
        };
        let transfer2 = BatchTransferArgs {
            receiver: Account::new(xtc(), None),
            amount: Tokens128::from(200),
        };
        let receipt = canister
            .batch_transfer(None, vec![transfer1, transfer2])
            .unwrap();
        assert_eq!(receipt.len(), 2);
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(600)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), None)),
            Tokens128::from(100)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(xtc(), None)),
            Tokens128::from(200)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(john(), None)),
            Tokens128::from(100)
        );
    }

    #[test]
    fn batch_transfer_insufficient_balance() {
        let canister = test_canister();

        let transfer1 = BatchTransferArgs {
            receiver: Account::new(bob(), None),
            amount: Tokens128::from(500),
        };
        let transfer2 = BatchTransferArgs {
            receiver: Account::new(john(), None),
            amount: Tokens128::from(600),
        };
        let receipt = canister.batch_transfer(None, vec![transfer1, transfer2]);
        assert!(receipt.is_err());
        let balance = canister.icrc1_balance_of(Account::new(alice(), None));
        assert_eq!(receipt.unwrap_err(), TxError::InsufficientFunds { balance });
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(1000)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), None)),
            Tokens128::from(0)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(john(), None)),
            Tokens128::from(0)
        );
    }

    #[test]
    fn batch_transfer_overflow() {
        let canister = test_canister();

        let transfer1 = BatchTransferArgs {
            receiver: Account::new(bob(), None),
            amount: Tokens128::from(u128::MAX - 10),
        };
        let transfer2 = BatchTransferArgs {
            receiver: Account::new(john(), None),
            amount: Tokens128::from(20),
        };
        let res = canister.batch_transfer(None, vec![transfer1, transfer2]);
        assert_eq!(
            res,
            Err(TxError::InsufficientFunds {
                balance: 1000.into()
            })
        );
    }

    #[test]
    fn batch_transfer_zero_amount() {
        let canister = test_canister();

        let transfer1 = BatchTransferArgs {
            receiver: Account::new(bob(), None),
            amount: Tokens128::from(100),
        };
        let transfer2 = BatchTransferArgs {
            receiver: Account::new(john(), None),
            amount: Tokens128::from(0),
        };
        let res = canister.batch_transfer(None, vec![transfer1, transfer2]);
        assert_eq!(res, Err(TxError::AmountTooSmall));
    }

    #[test]
    fn deduplication_error() {
        let canister = test_canister();
        let curr_time = ic::time();

        let transfer = TransferArgs {
            from_subaccount: None,
            to: Account::new(bob(), None),
            amount: 10_000.into(),
            fee: None,
            memo: None,
            created_at_time: Some(curr_time),
        };

        assert!(validate_and_get_tx_ts(alice(), &transfer).is_ok());

        let tx_id = canister.icrc1_transfer(transfer.clone()).unwrap();

        assert_eq!(
            validate_and_get_tx_ts(alice(), &transfer),
            Err(TxError::Duplicate {
                duplicate_of: tx_id as u64
            })
        )
    }

    #[test]
    fn deduplicate_check_pass() {
        let canister = test_canister();
        let curr_time = ic::time();

        let transfer = TransferArgs {
            from_subaccount: None,
            to: Account::new(bob(), None),
            amount: 10_000.into(),
            fee: None,
            memo: None,
            created_at_time: Some(curr_time),
        };

        let _ = canister.icrc1_transfer(transfer.clone()).unwrap();
        assert!(validate_and_get_tx_ts(john(), &transfer).is_ok());

        let mut tx = transfer.clone();
        tx.from_subaccount = Some([0; 32]);
        assert!(validate_and_get_tx_ts(john(), &tx).is_ok());

        let mut tx = transfer.clone();
        tx.amount = 10_001.into();
        assert!(validate_and_get_tx_ts(john(), &tx).is_ok());

        let mut tx = transfer.clone();
        tx.fee = Some(0.into());
        assert!(validate_and_get_tx_ts(john(), &tx).is_ok());

        let mut tx = transfer.clone();
        tx.memo = Some([0; 32]);
        assert!(validate_and_get_tx_ts(john(), &tx).is_ok());

        let mut tx = transfer.clone();
        tx.created_at_time = None;
        assert!(validate_and_get_tx_ts(john(), &tx).is_ok());

        let mut tx = transfer;
        tx.created_at_time = Some(curr_time + 1);
        assert!(validate_and_get_tx_ts(john(), &tx).is_ok());

        let transfer = TransferArgs {
            from_subaccount: None,
            to: Account::new(bob(), None),
            amount: 10_000.into(),
            fee: None,
            memo: Some([1; 32]),
            created_at_time: Some(curr_time),
        };

        let _ = canister.icrc1_transfer(transfer.clone()).unwrap();
        assert!(validate_and_get_tx_ts(john(), &transfer).is_ok());

        let mut tx = transfer.clone();
        tx.memo = None;
        assert!(validate_and_get_tx_ts(john(), &tx).is_ok());

        let mut tx = transfer;
        tx.memo = Some([2; 32]);
        assert!(validate_and_get_tx_ts(john(), &tx).is_ok());
    }

    #[test]
    fn deduplicate_check_no_created_at_time() {
        let canister = test_canister();

        let transfer = TransferArgs {
            from_subaccount: None,
            to: Account::new(bob(), None),
            amount: 10_000.into(),
            fee: None,
            memo: None,
            created_at_time: None,
        };

        let _ = canister.icrc1_transfer(transfer.clone()).unwrap();
        assert!(validate_and_get_tx_ts(alice(), &transfer).is_ok());
    }

    #[test]
    fn zero_transfer() {
        let canister = test_canister();
        let transfer = TransferArgs {
            from_subaccount: None,
            to: bob().into(),
            amount: 0.into(),
            fee: None,
            memo: None,
            created_at_time: None,
        };

        let caller = CheckedAccount::with_recipient(transfer.to.into(), None).unwrap();

        let res = is20_transfer(caller, &transfer, canister.bidding_info().fee_ratio);
        assert_eq!(res, Err(TxError::AmountTooSmall));
    }

    #[test]
    fn transfer_with_overflow() {
        let canister = test_canister();

        let mut stats = TokenConfig::get_stable();
        stats.fee = 100500.into();
        TokenConfig::set_stable(stats);

        let transfer = TransferArgs {
            from_subaccount: None,
            to: bob().into(),
            amount: (u128::MAX - 100000).into(),
            fee: None,
            memo: None,
            created_at_time: None,
        };

        let caller = CheckedAccount::with_recipient(transfer.to.into(), None).unwrap();

        let res = is20_transfer(caller, &transfer, canister.bidding_info().fee_ratio);
        assert_eq!(
            res,
            Err(TxError::InsufficientFunds {
                balance: 1000.into()
            })
        );
    }

    #[test]
    fn mint_too_much() {
        let _ = test_canister(); // initialize context

        mint(alice(), bob().into(), Tokens128::from(u128::MAX - 2000)).unwrap();
        let res = mint(alice(), john().into(), Tokens128::from(2000));
        assert_eq!(res, Err(TxError::AmountOverflow));
    }

    #[test]
    fn transfer_to_own_subaccount() {
        let canister = test_canister();
        let transfer = TransferArgs {
            from_subaccount: None,
            to: Account::new(alice(), Some([1; 32])),
            amount: (200).into(),
            fee: None,
            memo: None,
            created_at_time: None,
        };
        let caller = CheckedAccount::with_recipient(transfer.to.into(), None).unwrap();

        is20_transfer(caller, &transfer, canister.bidding_info().fee_ratio).unwrap();
        assert_eq!(canister.icrc1_balance_of(alice().into()), 800.into());
        assert_eq!(canister.icrc1_balance_of(transfer.to), 200.into());
    }

    #[test]
    fn transfer_using_default_subaccount() {
        let canister = test_canister();
        let transfer = TransferArgs {
            from_subaccount: None,
            to: Account::new(bob(), Some(DEFAULT_SUBACCOUNT)),
            amount: 200.into(),
            fee: None,
            memo: None,
            created_at_time: None,
        };
        let caller = CheckedAccount::with_recipient(transfer.to.into(), None).unwrap();

        is20_transfer(caller, &transfer, canister.bidding_info().fee_ratio).unwrap();
        assert_eq!(canister.icrc1_balance_of(bob().into()), 200.into());
    }

    // The transactions in the ledger can be saved not in the order of their `created_at_time`
    // value. In this test we check if the deduplication logic works properly in such cases.
    #[test]
    fn validate_time_transactions_with_strange_ts() {
        let canister = test_canister();
        let now = ic::time();

        let delayed_transfer = TransferArgs {
            from_subaccount: None,
            to: bob().into(),
            amount: 200.into(),
            fee: None,
            memo: None,
            created_at_time: Some(now + 121_000_000_000),
        };
        let caller = CheckedAccount::with_recipient(bob().into(), None).unwrap();
        let result = is20_transfer(caller, &delayed_transfer, canister.bidding_info().fee_ratio);
        assert_eq!(result, Err(TxError::CreatedInFuture { ledger_time: now }));

        let transfer = TransferArgs {
            from_subaccount: None,
            to: bob().into(),
            amount: 200.into(),
            fee: None,
            memo: None,
            created_at_time: Some(now),
        };

        let caller = CheckedAccount::with_recipient(bob().into(), None).unwrap();
        is20_transfer(caller, &transfer, canister.bidding_info().fee_ratio).unwrap();

        let context = get_context();
        context.update_caller(alice());
        context.add_time(61_000_000_000);

        let caller = CheckedAccount::with_recipient(bob().into(), None).unwrap();
        let tx_id =
            is20_transfer(caller, &delayed_transfer, canister.bidding_info().fee_ratio).unwrap();

        let caller = CheckedAccount::with_recipient(bob().into(), None).unwrap();
        let result = is20_transfer(caller, &delayed_transfer, canister.bidding_info().fee_ratio);
        assert_eq!(
            result,
            Err(TxError::Duplicate {
                duplicate_of: tx_id as u64
            })
        );

        context.add_time(60_000_000_000);

        let caller = CheckedAccount::with_recipient(bob().into(), None).unwrap();
        let result = is20_transfer(caller, &delayed_transfer, canister.bidding_info().fee_ratio);
        assert_eq!(
            result,
            Err(TxError::Duplicate {
                duplicate_of: tx_id as u64
            })
        );

        context.add_time(180_000_000_000);

        let caller = CheckedAccount::with_recipient(bob().into(), None).unwrap();
        let result = is20_transfer(caller, &delayed_transfer, canister.bidding_info().fee_ratio);
        assert_eq!(
            result,
            Err(TxError::TooOld {
                allowed_window_nanos: 60_000_000_000
            })
        );

        // This last transfer is needed to check if the deduplication logic stops at the right
        // moment when iterating over old transactions. It is visible in the test coverage report
        // only though.
        let transfer = TransferArgs {
            from_subaccount: None,
            to: bob().into(),
            amount: 200.into(),
            fee: None,
            memo: None,
            created_at_time: Some(ic::time()),
        };

        let caller = CheckedAccount::with_recipient(bob().into(), None).unwrap();
        is20_transfer(caller, &transfer, canister.bidding_info().fee_ratio).unwrap();
    }

    #[cfg(feature = "claim")]
    #[test]
    fn zero_claim_returns_error() {
        MockContext::new().with_caller(john()).inject();

        let res = claim(alice(), None);
        assert_eq!(res, Err(TxError::NothingToClaim));
    }

    #[test]
    fn burn_removes_empty_entry() {
        let _ = test_canister();
        mint(alice(), bob().into(), Tokens128::from(1_000_000)).unwrap();
        assert_ne!(StableBalances.get(&bob().into()), None);

        burn(alice(), bob().into(), Tokens128::from(1_000_000)).unwrap();
        assert_eq!(StableBalances.get(&bob().into()), None);
    }
}
