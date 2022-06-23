use std::cell::RefCell;
use std::rc::Rc;

use candid::Principal;
use ic_canister::{query, update, Canister};
use ic_helpers::tokens::Tokens128;

use crate::canister::erc20_transactions::{
    approve, burn_as_owner, burn_own_tokens, mint_as_owner, mint_test_token, transfer,
    transfer_from,
};
use crate::canister::is20_auction::{
    auction_info, bid_cycles, bidding_info, run_auction, AuctionError, BiddingInfo,
};
use crate::canister::is20_transactions::{batch_transfer, transfer_include_fee};
use crate::canister::TokenCanister;
use crate::principal::{CheckedPrincipal, Owner};
use crate::state::CanisterState;
use crate::types::{
    AuctionInfo, Metadata, StatsData, TokenInfo, TxError, TxId, TxReceipt, TxRecord,
};

pub enum CanisterUpdate {
    Name(String),
    Logo(String),
    Fee(Tokens128),
    FeeTo(Principal),
    Owner(Principal),
    MinCycles(u64),
    AuctionPeriod(u64),
}

#[allow(non_snake_case)]
pub trait ISTokenCanister: Canister {
    fn state(&self) -> Rc<RefCell<CanisterState>>;

    fn canister(&self) -> &TokenCanister;

    #[query]
    fn isTestToken(&self) -> bool {
        self.state().borrow().stats.is_test_token
    }

    #[query]
    fn name(&self) -> String {
        self.state().borrow().stats.name.clone()
    }

    #[query]
    fn symbol(&self) -> String {
        self.state().borrow().stats.symbol.clone()
    }

    #[query]
    fn logo(&self) -> String {
        self.state().borrow().stats.logo.clone()
    }

    #[query]
    fn decimals(&self) -> u8 {
        self.state().borrow().stats.decimals
    }

    #[query]
    fn totalSupply(&self) -> Tokens128 {
        self.state().borrow().stats.total_supply
    }

    #[query]
    fn owner(&self) -> Principal {
        self.state().borrow().stats.owner
    }

    #[query]
    fn getMetadata(&self) -> Metadata {
        self.state().borrow().get_metadata()
    }

    #[query]
    fn getTokenInfo(&self) -> TokenInfo {
        let StatsData {
            fee_to,
            deploy_time,
            ..
        } = self.state().borrow().stats;
        TokenInfo {
            metadata: self.state().borrow().get_metadata(),
            feeTo: fee_to,
            historySize: self.state().borrow().ledger.len(),
            deployTime: deploy_time,
            holderNumber: self.state().borrow().balances.0.len(),
            cycles: ic_canister::ic_kit::ic::balance(),
        }
    }

    #[query]
    fn getHolders(&self, start: usize, limit: usize) -> Vec<(Principal, Tokens128)> {
        self.state().borrow().balances.get_holders(start, limit)
    }

    #[query]
    fn getAllowanceSize(&self) -> usize {
        self.state().borrow().allowance_size()
    }

    #[query]
    fn getUserApprovals(&self, who: Principal) -> Vec<(Principal, Tokens128)> {
        self.state().borrow().user_approvals(who)
    }

    #[query]
    fn balanceOf(&self, holder: Principal) -> Tokens128 {
        self.state().borrow().balances.balance_of(&holder)
    }

    #[query]
    fn allowance(&self, owner: Principal, spender: Principal) -> Tokens128 {
        self.state().borrow().allowance(owner, spender)
    }

    #[query]
    fn historySize(&self) -> u64 {
        self.state().borrow().ledger.len()
    }

    fn update_stats(&self, _caller: CheckedPrincipal<Owner>, update: CanisterUpdate) {
        use CanisterUpdate::*;
        match update {
            Name(name) => self.state().borrow_mut().stats.name = name,
            Logo(logo) => self.state().borrow_mut().stats.logo = logo,
            Fee(fee) => self.state().borrow_mut().stats.fee = fee,
            FeeTo(fee_to) => self.state().borrow_mut().stats.fee_to = fee_to,
            Owner(owner) => self.state().borrow_mut().stats.owner = owner,
            MinCycles(min_cycles) => self.state().borrow_mut().stats.min_cycles = min_cycles,
            AuctionPeriod(period_sec) => {
                self.state().borrow_mut().bidding_state.auction_period = period_sec * 1_000_000
            }
        }
    }

    #[update]
    fn setName(&self, name: String) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&self.state().borrow_mut().stats)?;
        self.update_stats(caller, CanisterUpdate::Name(name));
        Ok(())
    }

    #[update]
    fn setLogo(&self, logo: String) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&self.state().borrow_mut().stats)?;
        self.update_stats(caller, CanisterUpdate::Logo(logo));
        Ok(())
    }

    #[update]
    fn setFee(&self, fee: Tokens128) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&self.state().borrow_mut().stats)?;
        self.update_stats(caller, CanisterUpdate::Fee(fee));
        Ok(())
    }

    #[update]
    fn setFeeTo(&self, fee_to: Principal) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&self.state().borrow_mut().stats)?;
        self.update_stats(caller, CanisterUpdate::FeeTo(fee_to));
        Ok(())
    }

    #[update]
    fn setOwner(&self, owner: Principal) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&self.state().borrow_mut().stats)?;
        self.update_stats(caller, CanisterUpdate::Owner(owner));
        Ok(())
    }

    #[update]
    fn approve(&self, spender: Principal, amount: Tokens128) -> TxReceipt {
        let caller = CheckedPrincipal::with_recipient(spender)?;
        approve(self.canister(), caller, amount)
    }

    /********************** TRANSFERS ***********************/
    #[update]
    fn transfer(
        &self,
        to: Principal,
        amount: Tokens128,
        fee_limit: Option<Tokens128>,
    ) -> TxReceipt {
        let caller = CheckedPrincipal::with_recipient(to)?;
        transfer(self.canister(), caller, amount, fee_limit)
    }

    #[update]
    fn transferFrom(&self, from: Principal, to: Principal, amount: Tokens128) -> TxReceipt {
        let caller = CheckedPrincipal::from_to(from, to)?;
        transfer_from(self.canister(), caller, amount)
    }

    /// Transfers `value` amount to the `to` principal, applying American style fee. This means, that
    /// the recipient will receive `value - fee`, and the sender account will be reduced exactly by `value`.
    ///
    /// Note, that the `value` cannot be less than the `fee` amount. If the value given is too small,
    /// transaction will fail with `TxError::AmountTooSmall` error.
    #[update]
    fn transferIncludeFee(&self, to: Principal, amount: Tokens128) -> TxReceipt {
        let caller = CheckedPrincipal::with_recipient(to)?;
        transfer_include_fee(self.canister(), caller, amount)
    }

    /// Takes a list of transfers, each of which is a pair of `to` and `value` fields, it returns a `TxReceipt` which contains
    /// a vec of transaction index or an error message. The list of transfers is processed in the order they are given. if the `fee`
    /// is set, the `fee` amount is applied to each transfer.
    /// The balance of the caller is reduced by sum of `value + fee` amount for each transfer. If the total sum of `value + fee` for all transfers,
    /// is less than the `balance` of the caller, the transaction will fail with `TxError::InsufficientBalance` error.
    #[update]
    fn batchTransfer(&self, transfers: Vec<(Principal, Tokens128)>) -> Result<Vec<TxId>, TxError> {
        for (to, _) in transfers.clone() {
            let _ = CheckedPrincipal::with_recipient(to)?;
        }
        batch_transfer(self.canister(), transfers)
    }

    #[update]
    fn mint(&self, to: Principal, amount: Tokens128) -> TxReceipt {
        if self.isTestToken() {
            let test_user = CheckedPrincipal::test_user(&self.state().borrow().stats)?;
            mint_test_token(&mut *self.state().borrow_mut(), test_user, to, amount)
        } else {
            let owner = CheckedPrincipal::owner(&self.state().borrow().stats)?;
            mint_as_owner(&mut *self.state().borrow_mut(), owner, to, amount)
        }
    }

    /// Burn `amount` of tokens from `from` principal.
    /// If `from` is None, then caller's tokens will be burned.
    /// If `from` is Some(_) but method called not by owner, `TxError::Unauthorized` will be returned.
    /// If owner calls this method and `from` is Some(who), then who's tokens will be burned.
    #[update]
    fn burn(&self, from: Option<Principal>, amount: Tokens128) -> TxReceipt {
        match from {
            None => burn_own_tokens(&mut *self.state().borrow_mut(), amount),
            Some(from) if from == ic_canister::ic_kit::ic::caller() => {
                burn_own_tokens(&mut *self.state().borrow_mut(), amount)
            }
            Some(from) => {
                let caller = CheckedPrincipal::owner(&self.state().borrow().stats)?;
                burn_as_owner(&mut *self.state().borrow_mut(), caller, from, amount)
            }
        }
    }

    /********************** AUCTION ***********************/

    /// Bid cycles for the next cycle auction.
    ///
    /// This method must be called with the cycles provided in the call. The amount of cycles cannot be
    /// less than 1_000_000. The provided cycles are accepted by the canister, and the user bid is
    /// saved for the next auction.
    #[update]
    fn bidCycles(&self, bidder: Principal) -> Result<u64, AuctionError> {
        bid_cycles(self.canister(), bidder)
    }

    /// Current information about bids and auction.
    #[query]
    fn biddingInfo(&self) -> BiddingInfo {
        bidding_info(self.canister())
    }

    /// Starts the cycle auction.
    ///
    /// This method can be called only once in a [BiddingState.auction_period]. If the time elapsed
    /// since the last auction is less than the set period, [AuctionError::TooEarly] will be returned.
    ///
    /// The auction will distribute the accumulated fees in proportion to the user cycle bids, and
    /// then will update the fee ratio until the next auction.
    #[update]
    fn runAuction(&self) -> Result<AuctionInfo, AuctionError> {
        run_auction(self.canister())
    }

    /// Returns the information about a previously held auction.
    #[query]
    fn auctionInfo(&self, id: usize) -> Result<AuctionInfo, AuctionError> {
        auction_info(self.canister(), id)
    }

    /// Returns the minimum cycles set for the canister.
    ///
    /// This value affects the fee ratio set by the auctions. The more cycles available in the canister
    /// the less proportion of the fees will be transferred to the auction participants. If the amount
    /// of cycles in the canister drops below this value, all the fees will be used for cycle auction.
    #[query]
    fn getMinCycles(&self) -> u64 {
        self.state().borrow().stats.min_cycles
    }

    /// Sets the minimum cycles for the canister. For more information about this value, read [get_min_cycles].
    ///
    /// Only the owner is allowed to call this method.
    #[update]
    fn setMinCycles(&self, min_cycles: u64) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&self.state().borrow_mut().stats)?;
        self.update_stats(caller, CanisterUpdate::MinCycles(min_cycles));
        Ok(())
    }

    /// Sets the minimum time between two consecutive auctions, in seconds.
    ///
    /// Only the owner is allowed to call this method.
    #[update]
    fn setAuctionPeriod(&self, period_sec: u64) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&self.state().borrow_mut().stats)?;
        // IC timestamp is in nanoseconds, thus multiplying
        self.update_stats(caller, CanisterUpdate::AuctionPeriod(period_sec));
        Ok(())
    }
}
