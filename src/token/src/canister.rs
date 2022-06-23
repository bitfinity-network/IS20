use std::cell::RefCell;
use std::rc::Rc;

use ic_canister::{init, query, update, Canister};
use ic_cdk::export::candid::Principal;

use crate::core::ISTokenCanister;
use crate::state::CanisterState;
use crate::types::{Metadata, PaginatedResult, Timestamp, TxId, TxRecord};

pub mod erc20_transactions;

#[cfg(not(feature = "no_api"))]
mod inspect;

pub mod is20_auction;
pub mod is20_notify;
pub mod is20_transactions;

// 1 day in nanoseconds.
const DEFAULT_AUCTION_PERIOD: Timestamp = 24 * 60 * 60 * 1_000_000;

const MAX_TRANSACTION_QUERY_LEN: usize = 1000;

#[derive(Debug, Clone, Canister)]
pub struct TokenCanister {
    #[id]
    principal: Principal,

    #[state]
    pub(crate) state: Rc<RefCell<CanisterState>>,
}

#[allow(non_snake_case)]
impl TokenCanister {
    #[init]
    pub fn init(&self, metadata: Metadata) {
        self.state
            .borrow_mut()
            .balances
            .0
            .insert(metadata.owner, metadata.totalSupply);

        self.state
            .borrow_mut()
            .ledger
            .mint(metadata.owner, metadata.owner, metadata.totalSupply);

        self.state.borrow_mut().stats = metadata.into();
        self.state.borrow_mut().bidding_state.auction_period = DEFAULT_AUCTION_PERIOD;
    }

    #[query]
    fn getTransaction(&self, id: TxId) -> TxRecord {
        self.state().borrow().ledger.get(id).unwrap_or_else(|| {
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
        count: usize,
        transaction_id: Option<TxId>,
    ) -> PaginatedResult {
        if count > MAX_TRANSACTION_QUERY_LEN {
            ic_canister::ic_kit::ic::trap("Too many transactions requested");
        }

        self.state
            .borrow()
            .ledger
            .get_transactions(who, count, transaction_id)
    }

    /// Returns the total number of transactions related to the user `who`.
    #[query]
    pub fn getUserTransactionCount(&self, who: Principal) -> usize {
        self.state.borrow().ledger.get_len_user_history(who)
    }
}

impl ISTokenCanister for TokenCanister {
    fn state(&self) -> Rc<RefCell<CanisterState>> {
        self.state.clone()
    }

    fn canister(&self) -> &TokenCanister {
        self
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
