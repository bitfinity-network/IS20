//! This module contains APIs from IS20 standard providing cycle auction related functionality.

use candid::Principal;
use ic_auction::error::AuctionError;
use ic_auction::state::{AuctionInfo, AuctionState};
use ic_helpers::tokens::Tokens128;

use crate::account::Account;
use crate::canister::erc20_transactions::transfer_balance;
use crate::state::{Balances, CanisterState};

use super::TokenCanisterAPI;

pub fn disburse_rewards(canister: &impl TokenCanisterAPI) -> Result<AuctionInfo, AuctionError> {
    let canister_state = canister.state();
    let auction_state = canister.auction_state();

    let CanisterState {
        ref mut balances,
        ref mut ledger,
        ..
    } = *canister_state.borrow_mut();

    let AuctionState {
        ref bidding_state,
        ref history,
        ..
    } = *auction_state.borrow();

    let total_amount = accumulated_fees(balances);
    let mut transferred_amount = Tokens128::from(0u128);
    let total_cycles = bidding_state.cycles_since_auction;

    let first_transaction_id = ledger.len();

    for (bidder, cycles) in &bidding_state.bids {
        let amount = (total_amount * cycles / total_cycles)
            .expect("total cycles is not 0 checked by bids existing")
            .to_tokens128()
            .expect("total cycles is smaller then single user bid cycles");
        transfer_balance(balances, auction_account(), (*bidder).into(), amount)
            .expect("auction principal always have enough balance");
        ledger.record_auction(*bidder, amount);
        transferred_amount =
            (transferred_amount + amount).expect("can never be larger than total_supply");
    }

    let last_transaction_id = ledger.len() - 1;
    let result = AuctionInfo {
        auction_id: history.len(),
        auction_time: ic_canister::ic_kit::ic::time(),
        tokens_distributed: transferred_amount,
        cycles_collected: total_cycles,
        fee_ratio: bidding_state.fee_ratio,
        first_transaction_id,
        last_transaction_id,
    };

    Ok(result)
}

pub fn auction_principal() -> Principal {
    // The management canister is not a real canister in IC, so it's usually used as a black hole
    // principal. In our case, we can use this principal as a balance holder for the auction tokens,
    // as no requests can ever be made from this principal.
    Principal::management_canister()
}

pub fn auction_account() -> Account {
    // There are no sub accounts for the auction principal
    Account::new(Principal::management_canister(), None)
}

pub fn accumulated_fees(balances: &Balances) -> Tokens128 {
    balances.balance_of(auction_account())
}

#[cfg(test)]
mod tests {
    use ic_auction::api::Auction;
    use ic_auction::state::MIN_BIDDING_AMOUNT;
    use ic_canister::ic_kit::mock_principals::{alice, bob};
    use ic_canister::ic_kit::MockContext;
    use ic_canister::Canister;
    use ic_helpers::metrics::Interval;

    use crate::error::TxError;
    use crate::mock::*;
    use crate::types::Metadata;

    use super::*;

    #[cfg_attr(coverage_nightly, no_coverage)]
    fn test_context() -> (&'static mut MockContext, TokenCanisterMock) {
        let context = MockContext::new().with_caller(alice()).inject();

        let canister = TokenCanisterMock::init_instance();
        canister.init(
            Metadata {
                logo: "".to_string(),
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

        (context, canister)
    }

    #[test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    fn bidding_cycles() {
        let (context, canister) = test_context();
        context.update_caller(bob());
        context.update_msg_cycles(2_000_000);

        canister.bid_cycles(bob()).unwrap();
        let info = canister.bidding_info();
        assert_eq!(info.total_cycles, 2_000_000);
        assert_eq!(info.caller_cycles, 2_000_000);

        context.update_caller(alice());
        let info = canister.bidding_info();
        assert_eq!(info.total_cycles, 2_000_000);
        assert_eq!(info.caller_cycles, 0);
    }

    #[test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    fn bidding_cycles_under_limit() {
        let (context, canister) = test_context();
        context.update_msg_cycles(MIN_BIDDING_AMOUNT - 1);
        assert_eq!(
            canister.bid_cycles(alice()),
            Err(AuctionError::BiddingTooSmall)
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    fn bidding_multiple_times() {
        let (context, canister) = test_context();
        context.update_msg_cycles(2_000_000);
        canister.bid_cycles(alice()).unwrap();

        context.update_msg_cycles(2_000_000);
        canister.bid_cycles(alice()).unwrap();

        assert_eq!(canister.bidding_info().caller_cycles, 4_000_000);
    }

    #[test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    fn auction_test() {
        let (context, canister) = test_context();
        context.update_msg_cycles(2_000_000);
        canister.bid_cycles(alice()).unwrap();

        context.update_msg_cycles(4_000_000);
        canister.bid_cycles(bob()).unwrap();

        canister.state().borrow_mut().balances.insert(
            auction_principal(),
            None,
            Tokens128::from(6000),
        );

        canister
            .state()
            .borrow()
            .balances
            .balance_of(auction_account());

        context.add_time(10u64.pow(9) * 60 * 60 * 300);

        let result = canister.run_auction().unwrap();
        assert_eq!(result.cycles_collected, 6_000_000);
        assert_eq!(result.first_transaction_id, 1);
        assert_eq!(result.last_transaction_id, 2);
        assert_eq!(result.tokens_distributed, Tokens128::from(6_000));

        assert_eq!(
            canister.state().borrow().balances.balance_of(bob().into()),
            Tokens128::from(4_000)
        );

        let retrieved_result = canister.auction_info(result.auction_id).unwrap();
        assert_eq!(retrieved_result, result);
    }

    #[test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    fn auction_without_bids() {
        let (_, canister) = test_context();
        assert_eq!(canister.run_auction(), Err(AuctionError::NoBids));
    }

    #[test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    fn auction_not_in_time() {
        let (context, canister) = test_context();
        context.update_msg_cycles(2_000_000);
        canister.bid_cycles(alice()).unwrap();

        {
            let state = canister.auction_state();
            let state = &mut state.borrow_mut().bidding_state;
            state.last_auction = ic_canister::ic_kit::ic::time() - 100_000;
            state.auction_period = 1_000_000_000;
        }

        let secs_remaining = canister
            .auction_state()
            .borrow()
            .bidding_state
            .cooldown_secs_remaining();

        assert_eq!(
            canister.run_auction(),
            Err(AuctionError::TooEarlyToBeginAuction(secs_remaining))
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    fn setting_min_cycles() {
        let (_, canister) = test_context();
        canister.set_min_cycles(100500).unwrap();
        assert_eq!(canister.get_min_cycles(), 100500);
    }

    #[test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    fn setting_min_cycles_not_authorized() {
        let (context, canister) = test_context();
        context.update_caller(bob());
        assert_eq!(
            canister.set_min_cycles(100500),
            Err(AuctionError::Unauthorized(bob().to_string()))
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    fn setting_auction_period() {
        let (context, canister) = test_context();
        context.update_caller(alice());
        canister
            .set_auction_period(Interval::Period { seconds: 100500 })
            .unwrap();
        assert_eq!(
            canister.bidding_info().auction_period,
            100500 * 10u64.pow(9)
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    fn setting_auction_period_not_authorized() {
        let (context, canister) = test_context();
        context.update_caller(bob());
        assert_eq!(
            canister.set_auction_period(Interval::Period { seconds: 100500 }),
            Err(AuctionError::Unauthorized(bob().to_string()))
        );
    }
}
