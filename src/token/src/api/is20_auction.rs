//! This module contains APIs from IS20 standard providing cycle auction related functionality.

use crate::api::dip20_transactions::_transfer;
use crate::state::{AuctionHistory, Balances, BiddingState, State};
use crate::types::{AuctionInfo, Timestamp, TxError};
use crate::utils::check_caller_is_owner;
use candid::{candid_method, CandidType, Deserialize, Nat, Principal};
use ic_cdk_macros::*;
use ic_kit::ic;
use ic_storage::IcStorage;
use std::collections::HashMap;

// Minimum bidding amount is required, for every update call costs cycles, and we want bidding
// to add cycles rather then to decrease them. 1M is chosen as one ingress call costs 590K cycles.
const MIN_BIDDING_AMOUNT: u64 = 1_000_000;

/// Current information about upcoming auction and current cycle bids.
#[derive(CandidType, Debug, Clone, Deserialize)]
pub struct BiddingInfo {
    /// Proportion of the transaction fees that will be distributed to the auction participants.
    ///
    /// The value of 1.0 means that all fees go to the auction, 0.0 means that all the fees go to
    /// the canister owner.
    fee_ratio: f64,

    /// Timestamp of the last auction.
    last_auction: Timestamp,

    /// Period of performing auctions. Auction cannot be started before `last_auction + auction_period`
    /// IC time.
    auction_period: Timestamp,

    /// Total cycles accumulated since the last auction.
    total_cycles: u64,

    /// The amount of cycles the caller bid for the upcoming auction.
    caller_cycles: u64,

    /// The amount of fees accumulated since the last auction and that will be distributed on the
    /// next auction.
    accumulated_fees: Nat,
}

#[derive(CandidType, Debug, Clone, Deserialize, PartialEq)]
pub enum AuctionError {
    /// Provided cycles in the `bid_cycles` call is less then the minimum allowed amount.
    BiddingTooSmall,

    /// There are no cycle bids pending, so the auction cannot be held.
    NoBids,

    /// Auction with the given ID is not found.
    AuctionNotFound,

    /// The specified period between the auctions is not passed yet.
    TooEarlyToBeginAuction,
}

/// Bid cycles for the next cycle auction.
///
/// This method must be called with the cycles provided in the call. The amount of cycles cannot be
/// less than 1_000_000. The provided cycles are accepted by the canister, and the user bid is
/// saved for the next auction.
#[update(name = "bidCycles")]
#[candid_method(update, rename = "bidCycles")]
fn bid_cycles(bidder: Principal) -> Result<u64, AuctionError> {
    let amount = ic::msg_cycles_available();
    if amount < MIN_BIDDING_AMOUNT {
        return Err(AuctionError::BiddingTooSmall);
    }

    let state = BiddingState::get();
    let mut state = state.borrow_mut();

    let amount_accepted = ic::msg_cycles_accept(amount);
    state.cycles_since_auction += amount_accepted;
    *state.bids.entry(bidder).or_insert(0) += amount_accepted;

    Ok(amount_accepted)
}

/// Current information about bids and auction.
#[query(name = "biddingInfo")]
#[candid_method(query, rename = "biddingInfo")]
fn bidding_info() -> BiddingInfo {
    let state = BiddingState::get();
    let state = state.borrow();
    BiddingInfo {
        fee_ratio: state.fee_ratio,
        last_auction: state.last_auction,
        auction_period: state.auction_period,
        total_cycles: state.cycles_since_auction,
        caller_cycles: state.bids.get(&ic::caller()).cloned().unwrap_or(0),
        accumulated_fees: accumulated_fees(),
    }
}

/// Starts the cycle auction.
///
/// This method can be called only once in a [BiddingState.auction_period]. If the time elapsed
/// since the last auction is less than the set period, [AuctionError::TooEarly] will be returned.
///
/// The auction will distribute the accumulated fees in proportion to the user cycle bids, and
/// then will update the fee ratio until the next auction.
#[update(name = "runAuction")]
#[candid_method(update, rename = "runAuction")]
fn run_auction() -> Result<AuctionInfo, AuctionError> {
    let state = BiddingState::get();
    let mut state = state.borrow_mut();

    let curr_time = ic::time();
    let next_auction = state.last_auction + state.auction_period;
    if curr_time < next_auction {
        return Err(AuctionError::TooEarlyToBeginAuction);
    }

    let result = perform_auction(&mut *state);
    reset_bidding_state(&mut *state);

    result
}

/// Returns the information about a previously held auction.
#[query(name = "auctionInfo")]
#[candid_method(query, rename = "auctionInfo")]
fn auction_info(id: usize) -> Result<AuctionInfo, AuctionError> {
    let auction_history = AuctionHistory::get();
    let auction_history = auction_history.borrow();
    auction_history
        .0
        .get(id)
        .cloned()
        .ok_or(AuctionError::AuctionNotFound)
}

/// Returns the minimum cycles set for the canister.
///
/// This value affects the fee ratio set by the auctions. The more cycles available in the canister
/// the less proportion of the fees will be transferred to the auction participants. If the amount
/// of cycles in the canister drops below this value, all the fees will be used for cycle auction.
#[query(name = getMinCycles)]
#[candid_method(query, rename = "getMinCycles")]
fn get_min_cycles() -> u64 {
    let state = State::get();
    let state = state.borrow();
    state.stats().min_cycles
}

/// Sets the minimum cycles for the canister. For more information about this value, read [get_min_cycles].
///
/// Only the owner is allowed to call this method.
#[update(name = "setMinCycles")]
#[candid_method(update, rename = "setMinCycles")]
fn set_min_cycles(min_cycles: u64) -> Result<(), TxError> {
    check_caller_is_owner()?;

    let state = State::get();
    let mut state = state.borrow_mut();
    state.stats_mut().min_cycles = min_cycles;
    Ok(())
}

/// Sets the minimum time between two consecutive auctions, in seconds.
///
/// Only the owner is allowed to call this method.
#[update(name = "setAuctionPeriod")]
#[candid_method(update, rename = "setAuctionPeriod")]
fn set_auction_period(period_sec: u64) -> Result<(), TxError> {
    check_caller_is_owner()?;

    let bidding_state = BiddingState::get();
    // IC timestamp is in nanoseconds, thus multiplying
    bidding_state.borrow_mut().auction_period = period_sec * 1_000_000;
    Ok(())
}

fn perform_auction(bidding_state: &mut BiddingState) -> Result<AuctionInfo, AuctionError> {
    if bidding_state.bids.is_empty() {
        return Err(AuctionError::NoBids);
    }

    let total_amount = accumulated_fees();
    let mut transferred_amount = Nat::from(0);
    let total_cycles = bidding_state.cycles_since_auction;

    let state = State::get();
    let mut state = state.borrow_mut();
    let ledger = state.ledger_mut();

    let first_id = ledger.len();

    for (bidder, cycles) in &bidding_state.bids {
        let amount = total_amount.clone() * *cycles / total_cycles;
        _transfer(auction_principal(), *bidder, amount.clone());
        ledger.auction(*bidder, amount.clone());
        transferred_amount += amount;
    }

    let last_id = ledger.len() - 1;
    let auction_history = AuctionHistory::get();
    let mut auction_history = auction_history.borrow_mut();

    let result = AuctionInfo {
        auction_id: auction_history.0.len(),
        auction_time: ic::time(),
        tokens_distributed: transferred_amount,
        cycles_collected: total_cycles,
        fee_ratio: bidding_state.fee_ratio,
        first_transaction_id: first_id,
        last_transaction_id: last_id,
    };

    auction_history.0.push(result.clone());

    Ok(result)
}

fn reset_bidding_state(bidding_state: &mut BiddingState) {
    let state = State::get();
    bidding_state.fee_ratio = get_fee_ratio(state.borrow().stats().min_cycles, ic::balance());
    bidding_state.cycles_since_auction = 0;
    bidding_state.last_auction = ic::time();
    bidding_state.bids = HashMap::new();
}

fn get_fee_ratio(min_cycles: u64, current_cycles: u64) -> f64 {
    let min_cycles = min_cycles as f64;
    let current_cycles = current_cycles as f64;
    if min_cycles == 0.0 {
        // Setting min_cycles to zero effectively turns off the auction functionality, as all the
        // fees will go to the owner.
        0.0
    } else if current_cycles <= min_cycles {
        1.0
    } else {
        // If current cycles are 10 times larger, then min_cycles, half of the fees go to the auction.
        // If current cycles are 1000 times larger, 17% of the fees go to the auction.
        2f64.powf((min_cycles / current_cycles).log10())
    }
}

pub fn auction_principal() -> Principal {
    // The management canister is not a real canister in IC, so it's usually used as a black hole
    // principal. In our case, we can use this principal as a balance holder for the auction tokens,
    // as not requests can ever be made from this principal.
    Principal::management_canister()
}

pub fn accumulated_fees() -> Nat {
    let balances = Balances::get();
    let balances = balances.borrow_mut();
    balances
        .0
        .get(&auction_principal())
        .cloned()
        .unwrap_or_else(|| Nat::from(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::init_context;
    use ic_kit::mock_principals::{alice, bob};
    use test_case::test_case;

    #[test_case(0, 0, 0.0)]
    #[test_case(0, 1000, 0.0)]
    #[test_case(1000, 0, 1.0)]
    #[test_case(1000, 1000, 1.0)]
    #[test_case(1000, 10_000, 0.5)]
    #[test_case(1000, 1_000_000, 0.125)]
    fn fee_ratio_tests(min_cycles: u64, current_cycles: u64, ratio: f64) {
        assert_eq!(get_fee_ratio(min_cycles, current_cycles), ratio);
    }

    #[test]
    fn bidding_cycles() {
        let context = init_context();
        context.update_caller(bob());
        context.update_msg_cycles(2_000_000);

        bid_cycles(bob()).unwrap();
        let info = bidding_info();
        assert_eq!(info.total_cycles, 2_000_000);
        assert_eq!(info.caller_cycles, 2_000_000);

        context.update_caller(alice());
        let info = bidding_info();
        assert_eq!(info.total_cycles, 2_000_000);
        assert_eq!(info.caller_cycles, 0);
    }

    #[test]
    fn bidding_cycles_under_limit() {
        let context = init_context();
        context.update_msg_cycles(MIN_BIDDING_AMOUNT - 1);
        assert_eq!(bid_cycles(alice()), Err(AuctionError::BiddingTooSmall));
    }

    #[test]
    fn bidding_multiple_times() {
        let context = init_context();
        context.update_msg_cycles(2_000_000);
        bid_cycles(alice()).unwrap();

        context.update_msg_cycles(2_000_000);
        bid_cycles(alice()).unwrap();

        assert_eq!(bidding_info().caller_cycles, 4_000_000);
    }

    #[test]
    fn auction_test() {
        let context = init_context();
        context.update_msg_cycles(2_000_000);
        bid_cycles(alice()).unwrap();

        context.update_msg_cycles(4_000_000);
        bid_cycles(bob()).unwrap();

        let balances = Balances::get();
        balances
            .borrow_mut()
            .0
            .insert(auction_principal(), Nat::from(6_000));

        let result = run_auction().unwrap();
        assert_eq!(result.cycles_collected, 6_000_000);
        assert_eq!(result.first_transaction_id, Nat::from(1));
        assert_eq!(result.last_transaction_id, Nat::from(2));
        assert_eq!(result.tokens_distributed, Nat::from(6_000));

        let balances = Balances::get();
        assert_eq!(balances.borrow().0[&bob()], 4_000);

        let retrieved_result = auction_info(result.auction_id).unwrap();
        assert_eq!(retrieved_result, result);
    }

    #[test]
    fn auction_without_bids() {
        init_context();
        assert_eq!(run_auction(), Err(AuctionError::NoBids));
    }

    #[test]
    fn auction_not_in_time() {
        let context = init_context();
        context.update_msg_cycles(2_000_000);
        bid_cycles(alice()).unwrap();

        let state = BiddingState::get();
        state.borrow_mut().last_auction = ic::time() - 100_000;
        state.borrow_mut().auction_period = 1_000_000_000;

        assert_eq!(run_auction(), Err(AuctionError::TooEarlyToBeginAuction));
    }

    #[test]
    fn fee_ratio_update() {
        let context = init_context();
        context.update_balance(1_000_000_000);

        let state = State::get();
        state.borrow_mut().stats_mut().min_cycles = 1_000_000;
        run_auction().unwrap_err();

        let bidding_state = BiddingState::get();
        assert_eq!(bidding_state.borrow().fee_ratio, 0.125);
    }

    #[test]
    fn setting_min_cycles() {
        init_context();
        set_min_cycles(100500).unwrap();
        assert_eq!(get_min_cycles(), 100500);
    }

    #[test]
    fn setting_min_cycles_not_authorized() {
        let context = init_context();
        context.update_caller(bob());
        assert_eq!(set_min_cycles(100500), Err(TxError::Unauthorized));
    }

    #[test]
    fn setting_auction_period() {
        init_context();
        set_auction_period(100500).unwrap();
        assert_eq!(bidding_info().auction_period, 100500 * 1000000);
    }

    #[test]
    fn setting_auction_period_not_authorized() {
        let context = init_context();
        context.update_caller(bob());
        assert_eq!(set_auction_period(100500), Err(TxError::Unauthorized));
    }
}
