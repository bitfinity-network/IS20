use crate::account::{AccountInternal, CheckedAccount, WithRecipient};
use crate::error::TxError;
use crate::state::config::TokenConfig;
use crate::state::ledger::{TransferArgs, TxReceipt};

use super::is20_transactions::burn;
use super::is20_transactions::is20_transfer;
use super::is20_transactions::mint;

pub const TX_WINDOW: u64 = 60_000_000_000;
pub const PERMITTED_DRIFT: u64 = 2 * 60_000_000_000;

pub fn icrc1_transfer(
    caller: CheckedAccount<WithRecipient>,
    transfer: &TransferArgs,
    auction_fee_ratio: f64,
) -> TxReceipt {
    let amount = transfer.amount;
    let minter = AccountInternal::new(TokenConfig::get_stable().owner, None);

    // Checks and returns error if the fee is not zero
    let check_zero_fee = || {
        if let Some(t) = transfer.fee {
            if !t.is_zero() {
                return Err(TxError::BadFee {
                    expected_fee: 0.into(),
                });
            }
        }
        Ok(())
    };

    if caller.inner() == minter {
        // Minting transfers must have zero fees.
        check_zero_fee()?;
        return mint(caller.inner().owner, transfer.to.into(), amount);
    }

    if caller.recipient() == minter {
        // Burning transfers must have zero fees.
        check_zero_fee()?;
        return burn(caller.recipient().owner, caller.inner(), amount);
    }

    is20_transfer(caller, transfer, auction_fee_ratio)
}

#[cfg(test)]
mod tests {
    use std::time::UNIX_EPOCH;

    use candid::Principal;
    use canister_sdk::ic_auction::api::Auction;
    use canister_sdk::ic_canister::Canister;
    use canister_sdk::ic_helpers::tokens::Tokens128;
    use canister_sdk::ic_kit::inject::get_context;
    use canister_sdk::ic_kit::mock_principals::{alice, bob, john, xtc};
    use canister_sdk::ic_kit::MockContext;
    use rand::prelude::*;

    use crate::account::{Account, Subaccount};
    use crate::canister::{auction_account, TokenCanisterAPI};
    use crate::error::{TransferError, TxError};
    use crate::mock::*;
    use crate::state::balances::{Balances, StableBalances};
    use crate::state::config::{Metadata, DEFAULT_MIN_CYCLES};
    use crate::state::ledger::{LedgerData, Operation, TransactionStatus};

    use super::*;

    use coverage_helper::test;

    // Method for generating random Subaccount.
    #[cfg_attr(coverage_nightly, no_coverage)]
    fn gen_subaccount() -> Subaccount {
        let mut subaccount = [0u8; 32];
        thread_rng().fill(&mut subaccount);
        subaccount
    }

    #[cfg_attr(coverage_nightly, no_coverage)]
    fn test_context() -> (&'static MockContext, TokenCanisterMock) {
        let context = MockContext::new().with_caller(john()).inject();

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
                owner: john(),
                fee: Tokens128::from(0),
                fee_to: john(),
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

        canister.mint(alice(), None, 1000.into()).unwrap();
        context.update_caller(alice());

        (context, canister)
    }

    #[cfg_attr(coverage_nightly, no_coverage)]
    fn test_canister() -> TokenCanisterMock {
        let (_, canister) = test_context();
        canister
    }

    #[test]
    fn minting_with_nonzero_fee() {
        let (_ctx, canister) = test_context();

        let minter = AccountInternal::new(TokenConfig::get_stable().owner, None);
        let to = Account::from(bob());

        let transfer = TransferArgs {
            from_subaccount: Some(minter.subaccount),
            to,
            amount: Tokens128::from(100),
            fee: Some(1.into()),
            memo: None,
            created_at_time: None,
        };

        assert!(
            canister.icrc1_transfer(transfer).is_err(),
            "minting with non zero fee must fail!"
        );
    }

    #[test]
    fn burning_with_nonzero_fee() {
        let (_ctx, canister) = test_context();

        let to = Account::from(TokenConfig::get_stable().owner);
        let from_subaccount = Account::from(bob()).subaccount;

        let transfer = TransferArgs {
            from_subaccount,
            to,
            amount: Tokens128::from(100),
            fee: Some(1.into()),
            memo: None,
            created_at_time: None,
        };

        assert!(
            canister.icrc1_transfer(transfer).is_err(),
            "burning with non zero fee must fail!"
        );
    }

    #[test]
    fn transfer_without_fee() {
        let (ctx, canister) = test_context();
        let alice_sub = gen_subaccount();
        let bob_sub = gen_subaccount();

        assert_eq!(
            Tokens128::from(1000),
            canister.icrc1_balance_of(Account::new(alice(), None))
        );

        let transfer1 = TransferArgs {
            from_subaccount: None,
            to: Account::from(bob()),
            amount: Tokens128::from(100),
            fee: None,
            memo: None,
            created_at_time: None,
        };

        assert!(canister.icrc1_transfer(transfer1).is_ok());
        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), None)),
            Tokens128::from(100)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(900)
        );

        ctx.update_caller(john());
        assert!(canister
            .mint(alice(), Some(alice_sub), Tokens128::from(100))
            .is_ok());

        ctx.update_caller(alice());
        let transfer2 = TransferArgs {
            from_subaccount: Some(alice_sub),
            to: Account::new(bob(), Some(bob_sub)),
            amount: Tokens128::from(50),
            fee: None,
            memo: None,
            created_at_time: None,
        };
        assert!(canister.icrc1_transfer(transfer2).is_ok());
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), Some(alice_sub))),
            Tokens128::from(50)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), Some(bob_sub))),
            Tokens128::from(50)
        );
        assert_eq!(canister.icrc1_total_supply(), Tokens128::from(2100));
    }

    #[test]
    fn transfer_with_fee() {
        let (ctx, canister) = test_context();
        let alice_sub = gen_subaccount();
        let bob_sub = gen_subaccount();

        let mut stats = TokenConfig::get_stable();
        stats.fee = Tokens128::from(100);
        stats.fee_to = john();
        TokenConfig::set_stable(stats);

        let transfer1 = TransferArgs {
            from_subaccount: None,
            to: Account::from(bob()),
            amount: Tokens128::from(200),
            fee: None,
            memo: None,
            created_at_time: None,
        };

        assert!(canister.icrc1_transfer(transfer1).is_ok());
        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), None)),
            Tokens128::from(200)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(700)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(john(), None)),
            Tokens128::from(1100)
        );

        ctx.update_caller(john());
        assert!(canister
            .mint(alice(), Some(alice_sub), Tokens128::from(1000))
            .is_ok());

        ctx.update_caller(alice());
        let transfer2 = TransferArgs {
            from_subaccount: Some(alice_sub),
            to: Account::new(bob(), Some(bob_sub)),

            amount: Tokens128::from(500),
            fee: None,
            memo: None,
            created_at_time: None,
        };
        assert!(canister.icrc1_transfer(transfer2).is_ok());

        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), Some(bob_sub))),
            Tokens128::from(500)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), Some(alice_sub))),
            Tokens128::from(400)
        );
    }

    #[test]
    fn transfer_fee_exceeded() {
        let canister = test_canister();

        let mut stats = TokenConfig::get_stable();
        stats.fee = Tokens128::from(100);
        stats.fee_to = john();
        TokenConfig::set_stable(stats);

        let transfer1 = TransferArgs {
            from_subaccount: None,
            to: Account::from(bob()),
            amount: Tokens128::from(200),
            fee: Some(Tokens128::from(100)),
            memo: None,
            created_at_time: None,
        };

        assert!(canister.icrc1_transfer(transfer1).is_ok());

        let transfer2 = TransferArgs {
            from_subaccount: None,
            to: Account::from(bob()),
            amount: Tokens128::from(200),
            fee: Some(Tokens128::from(50)),
            memo: None,
            created_at_time: None,
        };
        assert_eq!(
            canister.icrc1_transfer(transfer2),
            Err(TransferError::BadFee {
                expected_fee: Tokens128::from(100)
            })
        );

        let transfer3 = TransferArgs {
            from_subaccount: None,
            to: Account::new(bob(), Some(gen_subaccount())),
            amount: Tokens128::from(200),
            fee: Some(Tokens128::from(50)),
            memo: None,
            created_at_time: None,
        };
        assert_eq!(
            canister.icrc1_transfer(transfer3),
            Err(TransferError::BadFee {
                expected_fee: Tokens128::from(100)
            })
        );
    }

    #[test]
    fn fees_with_auction_enabled() {
        let canister = test_canister();

        let mut stats = TokenConfig::get_stable();
        stats.fee = Tokens128::from(50);
        stats.fee_to = john();
        stats.min_cycles = DEFAULT_MIN_CYCLES;
        TokenConfig::set_stable(stats);

        canister
            .auction_state()
            .borrow_mut()
            .bidding_state
            .fee_ratio = 0.5;

        let transfer1 = TransferArgs {
            from_subaccount: None,
            to: Account::from(bob()),
            amount: Tokens128::from(100),
            fee: None,
            memo: None,
            created_at_time: None,
        };

        canister.icrc1_transfer(transfer1).unwrap();
        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), None)),
            Tokens128::from(100)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(850)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(john(), None)),
            Tokens128::from(1025)
        );
        assert_eq!(
            canister.icrc1_balance_of(auction_account().into()),
            Tokens128::from(25)
        );
    }

    #[test]
    fn transfer_insufficient_balance() {
        let canister = test_canister();

        let transfer1 = TransferArgs {
            from_subaccount: None,
            to: Account::from(bob()),
            amount: Tokens128::from(1001),
            fee: None,
            memo: None,
            created_at_time: None,
        };
        let balance = canister.icrc1_balance_of(Account::new(alice(), None));
        assert_eq!(
            canister.icrc1_transfer(transfer1),
            Err(TransferError::InsufficientFunds { balance })
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(1000)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), None)),
            Tokens128::from(0)
        );
    }

    #[test]
    fn transfer_with_fee_insufficient_balance() {
        let canister = test_canister();

        let mut stats = TokenConfig::get_stable();
        stats.fee = Tokens128::from(100);
        stats.fee_to = john();
        TokenConfig::set_stable(stats);

        let transfer1 = TransferArgs {
            from_subaccount: None,
            to: Account::from(bob()),
            amount: Tokens128::from(950),
            fee: None,
            memo: None,
            created_at_time: None,
        };

        let balance = canister.icrc1_balance_of(Account::new(alice(), None));

        assert_eq!(
            canister.icrc1_transfer(transfer1),
            Err(TransferError::InsufficientFunds { balance })
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(1000)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), None)),
            Tokens128::from(0)
        );
    }

    #[test]
    fn transfer_wrong_caller() {
        let canister = test_canister();
        get_context().update_caller(bob());

        let transfer1 = TransferArgs {
            from_subaccount: None,
            to: Account::from(bob()),
            amount: Tokens128::from(100),
            fee: None,
            memo: None,
            created_at_time: None,
        };
        assert!(matches!(
            canister.icrc1_transfer(transfer1),
            Err(TransferError::GenericError { .. })
        ));
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(1000)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), None)),
            Tokens128::from(0)
        );

        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(1000)
        );
    }

    #[test]
    fn transfer_saved_into_history() {
        let (ctx, canister) = test_context();

        let mut stats = TokenConfig::get_stable();
        stats.fee = Tokens128::from(10);
        TokenConfig::set_stable(stats);

        let before_history_size = canister.history_size();

        let transfer1 = TransferArgs {
            from_subaccount: None,
            to: Account::from(bob()),
            amount: Tokens128::from(1001),
            fee: None,
            memo: None,
            created_at_time: None,
        };

        canister.icrc1_transfer(transfer1).unwrap_err();
        assert_eq!(canister.history_size(), before_history_size);

        const COUNT: u64 = 5;
        let mut ts = canister_sdk::ic_kit::ic::time();
        for i in 0..COUNT {
            let transfer1 = TransferArgs {
                from_subaccount: None,
                to: Account::from(bob()),
                amount: Tokens128::from(100 + i as u128),
                fee: None,
                memo: None,
                created_at_time: None,
            };
            ctx.add_time(10);
            let id = canister.icrc1_transfer(transfer1).unwrap();
            assert_eq!(canister.history_size() - before_history_size, 1 + i);
            let tx = canister.get_transaction(id as u64);
            assert_eq!(tx.amount, Tokens128::from(100 + i as u128));
            assert_eq!(tx.fee, Tokens128::from(10));
            assert_eq!(tx.operation, Operation::Transfer);
            assert_eq!(tx.status, TransactionStatus::Succeeded);
            assert_eq!(tx.index, i + before_history_size);
            assert_eq!(tx.from, alice().into());
            assert_eq!(tx.to, bob().into());
            assert!(ts < tx.timestamp);
            ts = tx.timestamp;
        }
    }

    #[test]
    fn mint_test_token() {
        let alice_sub = gen_subaccount();

        let canister = test_canister();
        get_context().update_caller(bob());
        assert_eq!(
            canister.mint(alice(), None, Tokens128::from(100)),
            Err(TxError::Unauthorized)
        );

        let mut stats = TokenConfig::get_stable();
        stats.is_test_token = true;
        TokenConfig::set_stable(stats);

        assert!(canister.mint(alice(), None, Tokens128::from(2000)).is_ok());
        assert!(canister.mint(bob(), None, Tokens128::from(5000)).is_ok());

        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(3000)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), None)),
            Tokens128::from(5000)
        );
        assert!(canister
            .mint(alice(), Some(alice_sub), Tokens128::from(1000))
            .is_ok());
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), Some(alice_sub))),
            Tokens128::from(1000)
        );
    }

    #[test]
    fn mint_by_owner() {
        let (ctx, canister) = test_context();
        let alice_sub = gen_subaccount();
        let bob_sub = gen_subaccount();
        ctx.update_caller(john());
        assert!(canister.mint(alice(), None, Tokens128::from(2000)).is_ok());
        assert!(canister.mint(bob(), None, Tokens128::from(5000)).is_ok());
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(3000)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), None)),
            Tokens128::from(5000)
        );
        assert_eq!(canister.icrc1_total_supply(), Tokens128::from(9000));

        //     mint to subaccounts
        assert!(canister
            .mint(alice(), Some(alice_sub), Tokens128::from(2000))
            .is_ok());
        assert!(canister
            .mint(bob(), Some(bob_sub), Tokens128::from(5000))
            .is_ok());

        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), Some(alice_sub))),
            Tokens128::from(2000)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), Some(bob_sub))),
            Tokens128::from(5000)
        );
        assert_eq!(canister.icrc1_total_supply(), Tokens128::from(16000));
    }

    #[test]
    fn mint_saved_into_history() {
        let (ctx, canister) = test_context();

        let mut stats = TokenConfig::get_stable();
        stats.fee = Tokens128::from(10);
        TokenConfig::set_stable(stats);

        ctx.update_caller(john());

        assert_eq!(canister.history_size(), 2);

        const COUNT: u64 = 5;
        let mut ts = canister_sdk::ic_kit::ic::time();
        for i in 0..COUNT {
            ctx.add_time(10);
            let id = canister
                .mint(bob(), None, Tokens128::from(100 + i as u128))
                .unwrap();
            assert_eq!(canister.history_size(), 3 + i);
            let tx = canister.get_transaction(id as u64);
            assert_eq!(tx.amount, Tokens128::from(100 + i as u128));
            assert_eq!(tx.fee, Tokens128::from(0));
            assert_eq!(tx.operation, Operation::Mint);
            assert_eq!(tx.status, TransactionStatus::Succeeded);
            assert_eq!(tx.index, i + 2);
            assert_eq!(tx.from, john().into());
            assert_eq!(tx.to, bob().into());

            assert!(ts < tx.timestamp);
            ts = tx.timestamp;
        }
    }

    #[test]
    fn burn_by_owner() {
        let canister = test_canister();
        assert!(canister.burn(None, None, Tokens128::from(100)).is_ok());
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(900)
        );
        assert_eq!(canister.icrc1_total_supply(), Tokens128::from(1900));
    }

    #[test]
    fn burn_too_much() {
        let canister = test_canister();
        let balance = canister.icrc1_balance_of(Account::new(alice(), None));
        assert_eq!(
            canister.burn(None, None, Tokens128::from(1001)),
            Err(TxError::InsufficientFunds { balance })
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(1000)
        );
        assert_eq!(canister.icrc1_total_supply(), Tokens128::from(2000));
    }

    #[test]
    fn burn_by_wrong_user() {
        let canister = test_canister();

        get_context().update_caller(bob());
        let balance = canister.icrc1_balance_of(Account::new(bob(), None));
        assert_eq!(
            canister.burn(None, None, Tokens128::from(100)),
            Err(TxError::InsufficientFunds { balance })
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(1000)
        );
        assert_eq!(canister.icrc1_total_supply(), Tokens128::from(2000));
    }

    #[test]
    fn burn_from() {
        let bob_sub = gen_subaccount();
        let (ctx, canister) = test_context();
        let bob_balance = Tokens128::from(1000);
        ctx.update_caller(john());
        canister.mint(bob(), None, bob_balance).unwrap();
        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), None)),
            bob_balance
        );
        canister
            .burn(Some(bob()), None, Tokens128::from(100))
            .unwrap();
        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), None)),
            Tokens128::from(900)
        );
        assert_eq!(canister.icrc1_total_supply(), Tokens128::from(2900));
        //     Burn from subaccount
        canister.mint(bob(), Some(bob_sub), bob_balance).unwrap();
        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), Some(bob_sub))),
            bob_balance
        );
        canister
            .burn(Some(bob()), Some(bob_sub), Tokens128::from(100))
            .unwrap();
        assert_eq!(
            canister.icrc1_balance_of(Account::new(bob(), Some(bob_sub))),
            Tokens128::from(900)
        );
    }

    #[test]
    fn burn_from_unauthorized() {
        let canister = test_canister();

        get_context().update_caller(bob());
        assert_eq!(
            canister.burn(Some(alice()), None, Tokens128::from(100)),
            Err(TxError::Unauthorized)
        );

        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(1000)
        );
        assert_eq!(canister.icrc1_total_supply(), Tokens128::from(2000));
    }

    #[test]
    fn burn_saved_into_history() {
        let (ctx, canister) = test_context();

        let mut stats = TokenConfig::get_stable();
        stats.fee = Tokens128::from(10);
        TokenConfig::set_stable(stats);

        let history_size_before = canister.history_size();

        ctx.update_caller(john());
        assert_eq!(canister.history_size(), history_size_before);

        const COUNT: u64 = 5;
        let mut ts = canister_sdk::ic_kit::ic::time();
        for i in 0..COUNT {
            ctx.add_time(10);
            let id = canister
                .burn(None, None, Tokens128::from(100 + i as u128))
                .unwrap();
            assert_eq!(canister.history_size(), history_size_before + 1 + i);
            let tx = canister.get_transaction(id as u64);
            assert_eq!(tx.amount, Tokens128::from(100 + i as u128));
            assert_eq!(tx.fee, Tokens128::from(0));
            assert_eq!(tx.operation, Operation::Burn);
            assert_eq!(tx.status, TransactionStatus::Succeeded);
            assert_eq!(tx.index, history_size_before + i);
            assert_eq!(tx.to, john().into());
            assert_eq!(tx.from, john().into());
            assert!(ts < tx.timestamp);
            ts = tx.timestamp;
        }
    }

    #[test]
    fn get_transactions_test() {
        let canister = test_canister();
        let transfer1 = TransferArgs {
            from_subaccount: None,
            to: Account::from(bob()),
            amount: Tokens128::from(10),
            fee: None,
            memo: None,
            created_at_time: None,
        };

        for _ in 1..=5 {
            canister.icrc1_transfer(transfer1.clone()).unwrap();
        }
        let transfer2 = TransferArgs {
            from_subaccount: None,
            to: Account::from(bob()),

            amount: Tokens128::from(10),
            fee: None,
            memo: None,
            created_at_time: None,
        };
        canister.icrc1_transfer(transfer2).unwrap();
        let transfer3 = TransferArgs {
            from_subaccount: None,
            to: Account::from(xtc()),
            amount: Tokens128::from(10),
            fee: None,
            memo: None,
            created_at_time: None,
        };
        canister.icrc1_transfer(transfer3).unwrap();
        let transfer4 = TransferArgs {
            from_subaccount: None,
            to: Account::from(john()),
            amount: Tokens128::from(10),
            fee: None,
            memo: None,
            created_at_time: None,
        };
        canister.icrc1_transfer(transfer4).unwrap();

        assert_eq!(canister.get_transactions(None, 11, None).result.len(), 10);
        assert_eq!(canister.get_transactions(None, 10, Some(3)).result.len(), 4);
        assert_eq!(
            canister
                .get_transactions(Some(bob()), 10, None)
                .result
                .len(),
            6
        );
        assert_eq!(
            canister.get_transactions(Some(xtc()), 5, None).result.len(),
            1
        );
        assert_eq!(
            canister
                .get_transactions(Some(alice()), 10, Some(5))
                .result
                .len(),
            5
        );
        assert_eq!(canister.get_transactions(None, 5, None).next, Some(4));
        assert_eq!(
            canister.get_transactions(Some(alice()), 3, Some(5)).next,
            Some(2)
        );
        assert_eq!(
            canister.get_transactions(Some(bob()), 3, Some(2)).next,
            None
        );

        let transfer5 = TransferArgs {
            from_subaccount: None,
            to: Account::from(bob()),
            amount: Tokens128::from(10),
            fee: None,
            memo: None,
            created_at_time: None,
        };

        for _ in 1..=10 {
            canister.icrc1_transfer(transfer5.clone()).unwrap();
        }

        let txn = canister.get_transactions(None, 5, None);
        assert_eq!(txn.result[0].index, 19);
        assert_eq!(txn.result[1].index, 18);
        assert_eq!(txn.result[2].index, 17);
        assert_eq!(txn.result[3].index, 16);
        assert_eq!(txn.result[4].index, 15);
        let txn2 = canister.get_transactions(None, 5, txn.next);
        assert_eq!(txn2.result[0].index, 14);
        assert_eq!(txn2.result[1].index, 13);
        assert_eq!(txn2.result[2].index, 12);
        assert_eq!(txn2.result[3].index, 11);
        assert_eq!(txn2.result[4].index, 10);
        assert_eq!(canister.get_transactions(None, 5, txn.next).next, Some(9));
    }

    #[test]
    #[should_panic]
    fn get_transaction_not_existing() {
        let canister = test_canister();
        canister.get_transaction(2);
    }

    #[test]
    fn get_transaction_count() {
        let canister = test_canister();
        const COUNT: usize = 10;
        let transfer1 = TransferArgs {
            from_subaccount: None,
            to: Account::from(bob()),

            amount: Tokens128::from(10),
            fee: None,
            memo: None,
            created_at_time: None,
        };
        for _ in 1..COUNT {
            canister.icrc1_transfer(transfer1.clone()).unwrap();
        }
        assert_eq!(canister.get_user_transaction_count(alice()), COUNT);
    }

    #[test]
    fn valid_transaction_time_window() {
        let canister = test_canister();

        let system_time = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let transfer = TransferArgs {
            from_subaccount: None,

            to: Account::from(bob()),
            amount: Tokens128::from(10),
            fee: None,
            memo: None,
            created_at_time: Some(system_time as u64 + 30_000_000_000),
        };
        assert!(canister.icrc1_transfer(transfer).is_ok());
    }

    #[test]
    fn invalid_transaction_time_window() {
        let canister = test_canister();

        let system_time = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let transfer = TransferArgs {
            from_subaccount: None,
            to: Account::from(bob()),
            amount: Tokens128::from(10),
            fee: None,
            memo: None,
            created_at_time: Some(system_time as u64 - TX_WINDOW * 2),
        };
        assert!(canister.icrc1_transfer(transfer).is_err());

        let transfer = TransferArgs {
            from_subaccount: None,
            to: Account::from(bob()),
            amount: Tokens128::from(10),
            fee: None,
            memo: None,
            created_at_time: Some(system_time as u64 + TX_WINDOW * 2),
        };
        assert!(canister.icrc1_transfer(transfer).is_err());
    }

    #[test]
    fn test_invalid_self_account_transfer() {
        let canister = test_canister();
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(1000)
        );
        let transfer = TransferArgs {
            from_subaccount: None,
            to: Account::from(alice()),
            amount: Tokens128::from(100),
            fee: None,
            memo: None,
            created_at_time: None,
        };
        assert!(canister.icrc1_transfer(transfer).is_err());

        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(1000)
        );

        let alice_sub = gen_subaccount();

        let transfer = TransferArgs {
            from_subaccount: Some(alice_sub),
            to: Account::new(alice(), Some(alice_sub)),
            amount: Tokens128::from(100),
            fee: None,
            memo: None,
            created_at_time: None,
        };

        assert!(canister.icrc1_transfer(transfer.clone()).is_err());

        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), Some(alice_sub))),
            Tokens128::from(0)
        );

        assert!(matches!(
            canister.icrc1_transfer(transfer),
            Err(TransferError::GenericError { .. })
        ));
    }

    #[test]
    fn test_valid_self_subaccount_transfer() {
        let canister = test_canister();
        let alice_sub1 = gen_subaccount();
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(1000)
        );
        let transfer = TransferArgs {
            from_subaccount: None,
            to: Account::new(alice(), Some(alice_sub1)),

            amount: Tokens128::from(100),
            fee: None,
            memo: None,
            created_at_time: None,
        };
        assert!(canister.icrc1_transfer(transfer).is_ok());

        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), None)),
            Tokens128::from(900)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), Some(alice_sub1))),
            Tokens128::from(100)
        );

        let alice_sub2 = gen_subaccount();

        let transfer = TransferArgs {
            from_subaccount: Some(alice_sub1),
            to: Account::new(alice(), Some(alice_sub2)),
            amount: Tokens128::from(10),
            fee: None,
            memo: None,
            created_at_time: None,
        };
        assert!(canister.icrc1_transfer(transfer).is_ok());
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), Some(alice_sub2))),
            Tokens128::from(10)
        );
        assert_eq!(
            canister.icrc1_balance_of(Account::new(alice(), Some(alice_sub1))),
            Tokens128::from(90)
        );
    }
}

#[cfg(test)]
mod proptests {
    use canister_sdk::ic_canister::Canister;
    use canister_sdk::ic_helpers::tokens::Tokens128;
    use canister_sdk::ic_kit::inject::get_context;
    use canister_sdk::ic_kit::MockContext;
    use ic_exports::Principal;
    use proptest::collection::vec;
    use proptest::prelude::*;
    use proptest::sample::Index;

    use crate::account::Account;
    use crate::canister::TokenCanisterAPI;
    use crate::error::{TransferError, TxError};
    use crate::mock::*;
    use crate::state::balances::{Balances, StableBalances};
    use crate::state::config::Metadata;
    use crate::state::ledger::LedgerData;

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Action {
        Mint {
            minter: Principal,
            recipient: Principal,
            amount: Tokens128,
        },
        Burn(Tokens128, Principal),
        TransferWithoutFee {
            from: Principal,
            to: Principal,
            amount: Tokens128,
            fee_limit: Option<Tokens128>,
        },
    }

    prop_compose! {
        fn select_principal(p: Vec<Principal>) (index in any::<Index>()) -> Principal {
            let i = index.index(p.len());
            p[i]
        }

    }

    #[cfg_attr(coverage_nightly, no_coverage)]
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
            // Without fee
            (
                select_principal(principals.clone()),
                select_principal(principals),
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
        ]
    }

    #[cfg_attr(coverage_nightly, no_coverage)]
    fn make_option() -> impl Strategy<Value = Option<Tokens128>> {
        prop_oneof![Just(None), (make_tokens128()).prop_map(Some)]
    }

    #[cfg_attr(coverage_nightly, no_coverage)]
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
            Tokens128::from(num.parse::<u128>().unwrap())
        }
    }
    prop_compose! {
        fn make_canister() (
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
                name,
                symbol,
                decimals,
                owner,
                fee,
                fee_to,
                is_test_token: None,
            };

            let principal = Principal::from_text("mfufu-x6j4c-gomzb-geilq").unwrap();
            let canister = TokenCanisterMock::from_principal(principal);
            get_context().update_id(canister.principal());

            // Refresh canister's state.
            TokenConfig::set_stable(TokenConfig::default());
            StableBalances.clear();
            LedgerData::clear();

            canister.init(meta,total_supply);
            // This is to make tests that don't rely on auction state
            // pass, because since we are running auction state on each
            // endpoint call, it affects `BiddingInfo.fee_ratio` that is
            // used for charging fees in `approve` endpoint.

            let mut stats = TokenConfig::get_stable();
            stats.min_cycles = 0;

            TokenConfig::set_stable(stats);
            (canister, principals)
        }
    }
    #[cfg_attr(coverage_nightly, no_coverage)]
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
            let starting_supply = canister.icrc1_total_supply();
            for action in actions {
                use Action::*;
                match action {
                    Mint { minter, recipient, amount } => {
                        get_context().update_caller(minter);
                        let original = canister.icrc1_total_supply();
                        let res = canister.mint(recipient, None,amount);
                        let expected = if minter == canister.owner() {
                            total_minted = (total_minted + amount).unwrap();
                            assert!(matches!(res, Ok(_)));
                            (original + amount).unwrap()
                        } else {
                            assert_eq!(res, Err(TxError::Unauthorized));
                            original
                        };
                        assert_eq!(expected, canister.icrc1_total_supply());
                    },
                    Burn(amount, burner) => {
                        get_context().update_caller(burner);
                        let original = canister.icrc1_total_supply();
                        let balance = canister.icrc1_balance_of(Account::new(burner, None));
                        let res = canister.burn(Some(burner), None, amount);
                        if balance < amount {
                            prop_assert_eq!(res, Err(TxError::InsufficientFunds { balance }));
                            prop_assert_eq!(original, canister.icrc1_total_supply());
                        } else {
                            prop_assert!(matches!(res, Ok(_)), "Burn error: {:?}. Balance: {}, amount: {}", res, balance, amount);
                            prop_assert_eq!((original - amount).unwrap(), canister.icrc1_total_supply());
                            total_burned = (total_burned + amount).unwrap();
                        }
                    },

                    TransferWithoutFee{from,to,amount,fee_limit} => {
                        if to == canister.owner() || from == canister.owner() {
                            // Skip these operation, becase they behave transfer to/from minting
                            // account behaves like mint/burn, and we test them in different cases.
                            return Ok(());
                        }

                        get_context().update_caller(from);
                        let from_balance = canister.icrc1_balance_of(Account::new(from, None));
                        let to_balance = canister.icrc1_balance_of(Account::new(to, None));
                        let (fee , fee_to) = TokenConfig::get_stable().fee_info();
                        let amount_with_fee = (amount + fee).unwrap();
                        let transfer1 = TransferArgs {
                            from_subaccount: None,
                            to:Account::new(to, None),
                            amount,
                            fee: fee_limit,
                            memo: None,
                            created_at_time: None,
                        };
                        let res = canister.icrc1_transfer(transfer1);

                        if to == from {
                            prop_assert!(matches!(res, Err(TransferError::GenericError {..})), "Invalid self transfer error");
                            return Ok(())
                        }

                        if let Some(fee_limit) = fee_limit {
                            if fee_limit != fee && from != canister.owner() && to != canister.owner() {
                                prop_assert_eq!(res, Err(TransferError::BadFee { expected_fee: fee }));
                                return Ok(())
                            }
                        }

                        if amount.is_zero() {
                            prop_assert_eq!(res, Err(TransferError::GenericError { error_code: 500, message: "amount too small".into() }));
                            return Ok(());
                        }
                        if from_balance < amount_with_fee {
                            prop_assert_eq!(res, Err(TransferError::InsufficientFunds { balance:from_balance }));
                            return Ok(());
                        }

                        if fee_to == from {
                            prop_assert!(matches!(res, Ok(_)));
                            prop_assert_eq!((from_balance - amount).unwrap(), canister.icrc1_balance_of(Account::new(from, None)));
                            return Ok(());
                        }

                        if fee_to == to {
                            prop_assert!(matches!(res, Ok(_)));
                            prop_assert_eq!(((to_balance + amount).unwrap() + fee).unwrap(), canister.icrc1_balance_of(Account::new(to, None)));
                            return Ok(());
                        }

                        prop_assert!(matches!(res, Ok(_)));

                        prop_assert_eq!((from_balance - amount_with_fee).unwrap(), canister.icrc1_balance_of(Account::new(from, None)));
                        prop_assert_eq!((to_balance + amount).unwrap(), canister.icrc1_balance_of(Account::new(to, None)));
                    }
                }
            }
            prop_assert_eq!(((total_minted + starting_supply).unwrap() - total_burned).unwrap(), canister.icrc1_total_supply());
        }
    }
}
