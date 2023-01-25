use std::cell::RefCell;
use std::collections::HashMap;

use candid::{CandidType, Deserialize, Principal};
use canister_sdk::ic_helpers::tokens::Tokens128;
use canister_sdk::ic_kit::ic;
use ic_stable_structures::{MemoryId, StableCell};

use crate::account::{Account, AccountInternal, Subaccount};
use crate::error::TxError;
use crate::state::config::Timestamp;
use crate::tx_record::{TxId, TxRecord};

const MAX_HISTORY_LENGTH: usize = 1_000_000;
const HISTORY_REMOVAL_BATCH_SIZE: usize = 10_000;
const TOTAL_TX_COUNT_MEMORY_ID: MemoryId = MemoryId::new(2);

thread_local! {
    static LEDGER: RefCell<HashMap<Principal, Ledger>> = RefCell::default();
    static TOTAL_TX_COUNT: RefCell<StableCell<u64>> =
        RefCell::new(StableCell::new(TOTAL_TX_COUNT_MEMORY_ID, 0)
            .expect("unable to initialize index offset for ledger"));
}

pub struct LedgerData;

impl LedgerData {
    pub fn is_empty() -> bool {
        Self::with_ledger(|ledger| ledger.is_empty())
    }

    pub fn len() -> u64 {
        Self::with_ledger(|ledger| ledger.len())
    }

    pub fn get(id: TxId) -> Option<TxRecord> {
        Self::with_ledger(|ledger| ledger.get(id))
    }

    pub fn get_transactions(
        who: Option<Principal>,
        count: usize,
        transaction_id: Option<TxId>,
    ) -> PaginatedResult {
        Self::with_ledger(|ledger| ledger.get_transactions(who, count, transaction_id))
    }

    pub fn list_transactions() -> Vec<TxRecord> {
        Self::with_ledger(|ledger| ledger.iter().cloned().collect())
    }

    pub fn get_len_user_history(user: Principal) -> usize {
        Self::with_ledger(|ledger| ledger.get_len_user_history(user))
    }

    pub fn transfer(
        from: AccountInternal,
        to: AccountInternal,
        amount: Tokens128,
        fee: Tokens128,
        memo: Option<Memo>,
        created_at_time: Timestamp,
    ) -> TxId {
        Self::with_ledger(|ledger| ledger.transfer(from, to, amount, fee, memo, created_at_time))
    }

    pub fn batch_transfer(
        from: AccountInternal,
        transfers: Vec<BatchTransferArgs>,
        fee: Tokens128,
    ) -> Vec<TxId> {
        Self::with_ledger(|ledger| ledger.batch_transfer(from, transfers, fee))
    }

    pub fn mint(from: AccountInternal, to: AccountInternal, amount: Tokens128) -> TxId {
        Self::with_ledger(|ledger| ledger.mint(from, to, amount))
    }

    pub fn burn(caller: AccountInternal, from: AccountInternal, amount: Tokens128) -> TxId {
        Self::with_ledger(|ledger| ledger.burn(caller, from, amount))
    }

    pub fn record_auction(to: Principal, amount: Tokens128) {
        Self::with_ledger(|ledger| ledger.record_auction(to, amount))
    }

    pub fn claim(claim_account: AccountInternal, to: AccountInternal, amount: Tokens128) -> TxId {
        Self::with_ledger(|ledger| ledger.claim(claim_account, to, amount))
    }

    pub fn clear() {
        Self::with_ledger(|ledger| ledger.clear())
    }

    fn with_ledger<F, R>(f: F) -> R
    where
        F: FnOnce(&mut Ledger) -> R,
    {
        LEDGER.with(|ledgers| {
            let canister_id = ic::id();
            let mut borrowed = ledgers.borrow_mut();
            let ledger = borrowed.entry(canister_id).or_default();
            f(ledger)
        })
    }
}

#[derive(Debug, Default, CandidType, Deserialize)]
pub struct Ledger {
    history: Vec<TxRecord>,
}

impl Ledger {
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> u64 {
        Self::read_total_tx_count()
    }

    fn next_id(&self) -> TxId {
        Self::read_total_tx_count()
    }

    pub fn get(&self, id: TxId) -> Option<TxRecord> {
        self.history.get(self.get_index(id)?).cloned()
    }

    pub fn get_transactions(
        &self,
        who: Option<Principal>,
        count: usize,
        transaction_id: Option<TxId>,
    ) -> PaginatedResult {
        let mut transactions = self
            .history
            .iter()
            .rev()
            .filter(|&tx| who.map_or(true, |c| tx.contains(c)))
            .filter(|tx| transaction_id.map_or(true, |id| id >= tx.index))
            .take(count + 1)
            .cloned()
            .collect::<Vec<_>>();

        let next_id = if transactions.len() == count + 1 {
            Some(transactions.remove(count).index)
        } else {
            None
        };

        PaginatedResult {
            result: transactions,
            next: next_id,
        }
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &TxRecord> {
        self.history.iter()
    }

    fn get_index(&self, id: TxId) -> Option<usize> {
        let first_stored_tx_id = Self::read_total_tx_count() - self.history.len() as u64; // Always >= 0
        if id < first_stored_tx_id || id > usize::MAX as TxId {
            None
        } else {
            Some((id - first_stored_tx_id) as usize)
        }
    }

    pub fn get_len_user_history(&self, user: Principal) -> usize {
        self.history.iter().filter(|&tx| tx.contains(user)).count()
    }

    pub fn transfer(
        &mut self,
        from: AccountInternal,
        to: AccountInternal,
        amount: Tokens128,
        fee: Tokens128,
        memo: Option<Memo>,
        created_at_time: Timestamp,
    ) -> TxId {
        let id = self.next_id();
        self.push(TxRecord::transfer(
            id,
            from,
            to,
            amount,
            fee,
            memo,
            created_at_time,
        ));

        id
    }

    pub fn batch_transfer(
        &mut self,
        from: AccountInternal,
        transfers: Vec<BatchTransferArgs>,
        fee: Tokens128,
    ) -> Vec<TxId> {
        transfers
            .into_iter()
            .map(|x| self.transfer(from, x.receiver.into(), x.amount, fee, None, ic::time()))
            .collect()
    }

    pub fn mint(&mut self, from: AccountInternal, to: AccountInternal, amount: Tokens128) -> TxId {
        let id = self.len();
        self.push(TxRecord::mint(id, from, to, amount));

        id
    }

    pub fn burn(
        &mut self,
        caller: AccountInternal,
        from: AccountInternal,
        amount: Tokens128,
    ) -> TxId {
        let id = self.next_id();
        self.push(TxRecord::burn(id, caller, from, amount));

        id
    }

    pub fn record_auction(&mut self, to: Principal, amount: Tokens128) {
        let id = self.next_id();
        self.push(TxRecord::auction(id, to.into(), amount))
    }

    fn push(&mut self, record: TxRecord) {
        self.history.push(record);
        Self::increase_total_tx_count();
        if self.history.len() > MAX_HISTORY_LENGTH + HISTORY_REMOVAL_BATCH_SIZE {
            // We remove first `HISTORY_REMOVAL_BATCH_SIZE` from the history at one go, to prevent
            // often relocation of the history vec.
            // This removal code can later be changed to moving old history records into another
            // storage.

            self.history = self.history[HISTORY_REMOVAL_BATCH_SIZE..].into();
        }
    }

    pub fn claim(
        &mut self,
        claim_account: AccountInternal,
        to: AccountInternal,
        amount: Tokens128,
    ) -> TxId {
        let id = self.next_id();
        self.push(TxRecord::claim(id, claim_account, to, amount));

        id
    }

    pub fn clear(&mut self) {
        self.history.clear();
        TOTAL_TX_COUNT.with(|count| {
            count
                .borrow_mut()
                .set(0)
                .expect("fail to write total tx count")
        });
    }

    fn increase_total_tx_count() {
        TOTAL_TX_COUNT.with(|count| {
            let mut count_mut = count.borrow_mut();
            let prev_count = *count_mut.get();
            count_mut
                .set(prev_count + 1)
                .expect("fail to write total tx count")
        });
    }

    fn read_total_tx_count() -> u64 {
        TOTAL_TX_COUNT.with(|offset| *offset.borrow().get())
    }
}

pub type TxReceipt = Result<u128, TxError>;

#[derive(CandidType, Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum TransactionStatus {
    Succeeded,
    Failed,
}

#[derive(CandidType, Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum Operation {
    Approve,
    Mint,
    Transfer,
    TransferFrom,
    Burn,
    Auction,
    Claim,
}

/// `PaginatedResult` is returned by paginated queries i.e `get_transactions`.
#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct PaginatedResult {
    /// The result is the transactions which is the `count` transactions starting from `next` if it exists.
    pub result: Vec<TxRecord>,

    /// This is  the next `id` of the transaction. The `next` is used as offset for the next query if it exits.
    pub next: Option<TxId>,
}

// Batch transfer arguments.
#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct BatchTransferArgs {
    pub receiver: Account,
    pub amount: Tokens128,
}

/// These are the arguments which are taken in the `icrc1_transfer`
#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct TransferArgs {
    pub from_subaccount: Option<Subaccount>,
    pub to: Account,
    pub amount: Tokens128,
    pub fee: Option<Tokens128>,
    pub memo: Option<Memo>,
    pub created_at_time: Option<Timestamp>,
}

impl TransferArgs {
    pub fn with_amount(&self, amount: Tokens128) -> Self {
        Self {
            amount,
            ..self.clone()
        }
    }
}

pub type Memo = [u8; 32];
