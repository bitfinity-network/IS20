use crate::canister::erc20_transactions::{
    approve, burn_as_owner, burn_own_tokens, mint_as_owner, mint_test_token, transfer,
    transfer_from,
};
use crate::canister::is20_auction::{
    auction_info, bid_cycles, bidding_info, run_auction, AuctionError, BiddingInfo,
};
use crate::canister::is20_notify::{approve_and_notify, consume_notification, notify};
use crate::canister::is20_transactions::{batch_transfer, transfer_include_fee};
use crate::principal::{CheckedPrincipal, Owner};
use crate::state::{CanisterState, BIDDING_STATE_HEADER, STATS_DATA_HEADER};
use crate::types::{
    AuctionInfo, PaginatedResult, StatsData, Timestamp, TokenInfo, TxError, TxReceipt, TxRecord,
};
use candid::Nat;
use common::types::Metadata;
use ic_canister::{init, query, update, Canister};
use ic_cdk::export::candid::Principal;
use std::cell::RefCell;
use std::rc::Rc;

mod erc20_transactions;
mod inspect;
pub mod is20_auction;
pub mod is20_notify;
mod is20_transactions;

// 1 day in nanoseconds.
const DEFAULT_AUCTION_PERIOD: Timestamp = 24 * 60 * 60 * 1_000_000;

const MAX_TRANSACTION_QUERY_LEN: usize = 1000;

enum CanisterUpdate {
    Name(String),
    Logo(String),
    Fee(Nat),
    FeeTo(Principal),
    Owner(Principal),
    MinCycles(u64),
    AuctionPeriod(u64),
}

#[derive(Debug, Clone, Canister)]
pub struct TokenCanister {
    #[id]
    principal: Principal,

    #[state(stable = false)]
    pub(crate) state: Rc<RefCell<CanisterState>>,
}

#[allow(non_snake_case)]
impl TokenCanister {
    #[init]
    pub fn init(&self, metadata: Metadata) {
        self.state
            .borrow_mut()
            .balances
            .insert(metadata.owner, metadata.totalSupply.clone());

        self.state.borrow_mut().ledger.mint(
            metadata.owner,
            metadata.owner,
            metadata.totalSupply.clone(),
        );

        self.state.borrow_mut().stats = metadata.into();
        STATS_DATA_HEADER.with(|s| self.state.borrow().stats.save_header(&s.borrow()));

        self.state.borrow_mut().bidding_state.auction_period = DEFAULT_AUCTION_PERIOD;
        BIDDING_STATE_HEADER.with(|b| {
            self.state.borrow().bidding_state.save_header(&b.borrow());
        });
    }

    // #[post_upgrade]
    // pub fn post_upgrade(&self) {
    //     BIDDING_STATE_HEADER.with(|b| {
    //         self.state
    //             .borrow_mut()
    //             .bidding_state
    //             .load_header(&b.borrow());
    //     });
    //     LEDGER_HEADER.with(|l| {
    //         self.state.borrow_mut().ledger.load_header(&l.borrow());
    //     });
    //     STATS_DATA_HEADER.with(|s| self.state.borrow_mut().stats.load_header(&s.borrow()));
    // }

    #[query]
    pub fn getTokenInfo(&self) -> TokenInfo {
        let StatsData {
            fee_to,
            deploy_time,
            ..
        } = self.state.borrow().stats;
        TokenInfo {
            metadata: self.state.borrow().get_metadata(),
            feeTo: fee_to,
            historySize: self.state.borrow().ledger.len(),
            deployTime: deploy_time,
            holderNumber: self.state.borrow().balances.len(),
            cycles: ic_canister::ic_kit::ic::balance(),
        }
    }

    #[query]
    pub fn getHolders(&self, start: usize, limit: usize) -> Vec<(Principal, Nat)> {
        self.state.borrow().balances.get_holders(start, limit)
    }

    #[query]
    pub fn getAllowanceSize(&self) -> usize {
        self.state.borrow().allowance_size()
    }

    #[query]
    pub fn getUserApprovals(&self, who: Principal) -> Vec<(Principal, Nat)> {
        self.state.borrow().user_approvals(who)
    }

    #[query]
    pub fn isTestToken(&self) -> bool {
        self.state.borrow().stats.is_test_token
    }

    #[query]
    pub fn name(&self) -> String {
        self.state.borrow().stats.name.clone()
    }

    #[query]
    pub fn symbol(&self) -> String {
        self.state.borrow().stats.symbol.clone()
    }

    #[query]
    pub fn logo(&self) -> String {
        self.state.borrow().stats.logo.clone()
    }

    #[query]
    pub fn decimals(&self) -> u8 {
        self.state.borrow().stats.decimals
    }

    #[query]
    pub fn totalSupply(&self) -> Nat {
        self.state.borrow().stats.total_supply.clone()
    }

    #[query]
    pub fn balanceOf(&self, holder: Principal) -> Nat {
        self.state.borrow().balances.balance_of(&holder)
    }

    #[query]
    pub fn allowance(&self, owner: Principal, spender: Principal) -> Nat {
        self.state.borrow().allowance(owner, spender)
    }

    #[query]
    pub fn getMetadata(&self) -> Metadata {
        self.state.borrow().get_metadata()
    }

    #[query]
    pub fn historySize(&self) -> Nat {
        self.state.borrow().ledger.len()
    }

    #[query]
    pub fn getTransaction(&self, id: Nat) -> TxRecord {
        self.state.borrow().ledger.get(&id).unwrap_or_else(|| {
            ic_canister::ic_kit::ic::trap(&format!("Transaction {} does not exist", id))
        })
    }

    /// Returns a list of transactions in paginated form. The `who` is optional, if given, only transactions of the `who` are
    /// returned. `count` is the number of transactions to return, `transaction_id` is the transaction index which is used as
    /// the offset of the first transaction to return, any
    ///
    /// It returns `PaginatedResult` a struct, which contains `result` which is a list of transactions `Vec<TxRecord>` that meet the requirements of the query,
    /// and `next_id` which is the index of the next transaction to return.

    #[query]
    pub fn getTransactions(
        &self,
        who: Option<Principal>,
        count: u32,
        transaction_id: Option<u128>,
    ) -> PaginatedResult {
        if count as usize > MAX_TRANSACTION_QUERY_LEN {
            ic_canister::ic_kit::ic::trap("Too many transactions requested");
        }

        self.state
            .borrow()
            .ledger
            .get_transactions(who, count, transaction_id)
    }

    // This function can only be called as the owner
    fn update_stats(&self, _caller: CheckedPrincipal<Owner>, update: CanisterUpdate) {
        use CanisterUpdate::*;
        match update {
            Name(name) => {
                self.state.borrow_mut().stats.name = name;
                STATS_DATA_HEADER.with(|s| self.state.borrow().stats.save_header(&s.borrow()));
            }
            Logo(logo) => {
                self.state.borrow_mut().stats.logo = logo;
                STATS_DATA_HEADER.with(|s| self.state.borrow().stats.save_header(&s.borrow()));
            }
            Fee(fee) => {
                self.state.borrow_mut().stats.fee = fee;
                STATS_DATA_HEADER.with(|s| self.state.borrow().stats.save_header(&s.borrow()));
            }
            FeeTo(fee_to) => {
                self.state.borrow_mut().stats.fee_to = fee_to;
                STATS_DATA_HEADER.with(|s| self.state.borrow().stats.save_header(&s.borrow()));
            }
            Owner(owner) => {
                self.state.borrow_mut().stats.owner = owner;
                STATS_DATA_HEADER.with(|s| self.state.borrow().stats.save_header(&s.borrow()));
            }
            MinCycles(min_cycles) => {
                self.state.borrow_mut().stats.min_cycles = min_cycles;
                STATS_DATA_HEADER.with(|s| self.state.borrow().stats.save_header(&s.borrow()));
            }
            AuctionPeriod(period_sec) => {
                self.state.borrow_mut().bidding_state.auction_period = period_sec * 1_000_000;
                BIDDING_STATE_HEADER.with(|b| {
                    self.state.borrow().bidding_state.save_header(&b.borrow());
                });
            }
        }
    }

    #[update]
    pub fn setName(&self, name: String) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&self.state.borrow_mut().stats)?;
        self.update_stats(caller, CanisterUpdate::Name(name));
        Ok(())
    }

    #[update]
    pub fn setLogo(&self, logo: String) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&self.state.borrow_mut().stats)?;
        self.update_stats(caller, CanisterUpdate::Logo(logo));
        Ok(())
    }

    #[update]
    pub fn setFee(&self, fee: Nat) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&self.state.borrow_mut().stats)?;
        self.update_stats(caller, CanisterUpdate::Fee(fee));
        Ok(())
    }

    #[update]
    pub fn setFeeTo(&self, fee_to: Principal) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&self.state.borrow_mut().stats)?;
        self.update_stats(caller, CanisterUpdate::FeeTo(fee_to));
        Ok(())
    }

    #[update]
    pub fn setOwner(&self, owner: Principal) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&self.state.borrow_mut().stats)?;
        self.update_stats(caller, CanisterUpdate::Owner(owner));
        Ok(())
    }

    #[query]
    pub fn owner(&self) -> Principal {
        self.state.borrow().stats.owner
    }

    /// Returns the total number of transactions related to the user `who`.
    #[query]
    pub fn getUserTransactionCount(&self, who: Principal) -> Nat {
        self.state.borrow().ledger.get_len_user_history(who)
    }

    #[update]
    pub fn transfer(&self, to: Principal, value: Nat, fee_limit: Option<Nat>) -> TxReceipt {
        let caller = CheckedPrincipal::with_recipient(to)?;
        transfer(self, caller, value, fee_limit)
    }

    #[update]
    pub fn transferFrom(&self, from: Principal, to: Principal, value: Nat) -> TxReceipt {
        let caller = CheckedPrincipal::from_to(from, to)?;
        transfer_from(self, caller, value)
    }

    /// Transfers `value` amount to the `to` principal, applying American style fee. This means, that
    /// the recipient will receive `value - fee`, and the sender account will be reduced exactly by `value`.
    ///
    /// Note, that the `value` cannot be less than the `fee` amount. If the value given is too small,
    /// transaction will fail with `TxError::AmountTooSmall` error.
    #[update]
    pub fn transferIncludeFee(&self, to: Principal, value: Nat) -> TxReceipt {
        let caller = CheckedPrincipal::with_recipient(to)?;
        transfer_include_fee(self, caller, value)
    }

    /// Takes a list of transfers, each of which is a pair of `to` and `value` fields, it returns a `TxReceipt` which contains
    /// a vec of transaction index or an error message. The list of transfers is processed in the order they are given. if the `fee`
    /// is set, the `fee` amount is applied to each transfer.
    /// The balance of the caller is reduced by sum of `value + fee` amount for each transfer. If the total sum of `value + fee` for all transfers,
    /// is less than the `balance` of the caller, the transaction will fail with `TxError::InsufficientBalance` error.
    #[update]
    pub fn batchTransfer(&self, transfers: Vec<(Principal, Nat)>) -> Result<Vec<Nat>, TxError> {
        for (to, _) in transfers.clone() {
            let _ = CheckedPrincipal::with_recipient(to)?;
        }
        batch_transfer(self, transfers)
    }

    #[update]
    pub fn approve(&self, spender: Principal, value: Nat) -> TxReceipt {
        let caller = CheckedPrincipal::with_recipient(spender)?;
        approve(self, caller, value)
    }

    #[update]
    pub async fn approveAndNotify(&self, spender: Principal, value: Nat) -> TxReceipt {
        let caller = CheckedPrincipal::with_recipient(spender)?;
        approve_and_notify(self, caller, value).await
    }

    #[update]
    pub async fn notify(&self, transaction_id: Nat, to: Principal) -> TxReceipt {
        notify(self, transaction_id, to).await
    }

    #[update]
    pub async fn consume_notification(&self, transaction_id: Nat) -> TxReceipt {
        consume_notification(self, transaction_id).await
    }

    #[update]
    pub fn mint(&self, to: Principal, amount: Nat) -> TxReceipt {
        if self.isTestToken() {
            let test_user = CheckedPrincipal::test_user(&self.state.borrow().stats)?;
            mint_test_token(self, test_user, to, amount)
        } else {
            let owner = CheckedPrincipal::owner(&self.state.borrow().stats)?;
            mint_as_owner(self, owner, to, amount)
        }
    }

    /// Burn `amount` of tokens from `from` principal.
    /// If `from` is None, then caller's tokens will be burned.
    /// If `from` is Some(_) but method called not by owner, `TxError::Unauthorized` will be returned.
    /// If owner calls this method and `from` is Some(who), then who's tokens will be burned.
    #[update]
    pub fn burn(&self, from: Option<Principal>, amount: Nat) -> TxReceipt {
        match from {
            None => burn_own_tokens(self, amount),
            Some(from) if from == ic_canister::ic_kit::ic::caller() => {
                burn_own_tokens(self, amount)
            }
            Some(from) => {
                let caller = CheckedPrincipal::owner(&self.state.borrow().stats)?;
                burn_as_owner(self, caller, from, amount)
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
    pub fn bidCycles(&self, bidder: Principal) -> Result<u64, AuctionError> {
        bid_cycles(self, bidder)
    }

    /// Current information about bids and auction.
    #[query]
    pub fn biddingInfo(&self) -> BiddingInfo {
        bidding_info(self)
    }

    /// Starts the cycle auction.
    ///
    /// This method can be called only once in a [BiddingState.auction_period]. If the time elapsed
    /// since the last auction is less than the set period, [AuctionError::TooEarly] will be returned.
    ///
    /// The auction will distribute the accumulated fees in proportion to the user cycle bids, and
    /// then will update the fee ratio until the next auction.
    #[update]
    pub fn runAuction(&self) -> Result<AuctionInfo, AuctionError> {
        run_auction(self)
    }

    /// Returns the information about a previously held auction.
    #[query]
    pub fn auctionInfo(&self, id: usize) -> Result<AuctionInfo, AuctionError> {
        auction_info(self, id)
    }

    /// Returns the minimum cycles set for the canister.
    ///
    /// This value affects the fee ratio set by the auctions. The more cycles available in the canister
    /// the less proportion of the fees will be transferred to the auction participants. If the amount
    /// of cycles in the canister drops below this value, all the fees will be used for cycle auction.
    #[query]
    pub fn getMinCycles(&self) -> u64 {
        self.state.borrow().stats.min_cycles
    }

    /// Sets the minimum cycles for the canister. For more information about this value, read [get_min_cycles].
    ///
    /// Only the owner is allowed to call this method.
    #[update]
    pub fn setMinCycles(&self, min_cycles: u64) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&self.state.borrow_mut().stats)?;
        self.update_stats(caller, CanisterUpdate::MinCycles(min_cycles));
        Ok(())
    }

    /// Sets the minimum time between two consecutive auctions, in seconds.
    ///
    /// Only the owner is allowed to call this method.
    #[update]
    pub fn setAuctionPeriod(&self, period_sec: u64) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&self.state.borrow_mut().stats)?;
        // IC timestamp is in nanoseconds, thus multiplying
        self.update_stats(caller, CanisterUpdate::AuctionPeriod(period_sec));
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_upgrade_from_previous() {
        use ic_storage::stable::write;
        write(&()).unwrap();
        let canister = TokenCanister::init_instance();
        canister.__post_upgrade_inst();
    }

    #[test]
    fn test_upgrade_from_current() {
        // Set a value on the state...
        let canister = TokenCanister::init_instance();
        let mut state = canister.state.borrow_mut();
        state.bidding_state.fee_ratio = 12345.0;
        drop(state);
        // ... write the state to stable storage
        canister.__pre_upgrade_inst();

        // Update the value without writing it to stable storage
        let mut state = canister.state.borrow_mut();
        state.bidding_state.fee_ratio = 0.0;
        drop(state);

        // Upgrade the canister should have the state
        // written before pre_upgrade
        canister.__post_upgrade_inst();
        let state = canister.state.borrow();
        assert_eq!(state.bidding_state.fee_ratio, 12345.0);
    }
}
