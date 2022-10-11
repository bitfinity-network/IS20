use std::cell::RefCell;

use candid::{CandidType, Deserialize, Principal};
use canister_sdk::ic_helpers::tokens::Tokens128;
use canister_sdk::ic_kit::ic;

use crate::account::{Account, AccountInternal, Subaccount};
use crate::error::TxError;
use crate::state::config::Timestamp;
use crate::tx_record::{TxId, TxRecord};

const MAX_HISTORY_LENGTH: usize = 1_000_000;
const HISTORY_REMOVAL_BATCH_SIZE: usize = 10_000;

thread_local! {
    static LEDGER: RefCell<Ledger> = RefCell::default();
}

pub struct LedgerData;

impl LedgerData {
    pub fn is_empty() -> bool {
        LEDGER.with(|ledger| ledger.borrow().is_empty())
    }

    pub fn len() -> u64 {
        LEDGER.with(|ledger| ledger.borrow().len())
    }

    pub fn get(id: TxId) -> Option<TxRecord> {
        LEDGER.with(|ledger| ledger.borrow().get(id))
    }

    pub fn get_transactions(
        who: Option<Principal>,
        count: usize,
        transaction_id: Option<TxId>,
    ) -> PaginatedResult {
        LEDGER.with(|ledger| ledger.borrow().get_transactions(who, count, transaction_id))
    }

    pub fn list_transactions() -> Vec<TxRecord> {
        LEDGER.with(|ledger| ledger.borrow().iter().cloned().collect())
    }

    pub fn get_len_user_history(user: Principal) -> usize {
        LEDGER.with(|ledger| ledger.borrow().get_len_user_history(user))
    }

    pub fn transfer(
        from: AccountInternal,
        to: AccountInternal,
        amount: Tokens128,
        fee: Tokens128,
        memo: Option<Memo>,
        created_at_time: Timestamp,
    ) -> TxId {
        LEDGER.with(|ledger| {
            ledger
                .borrow_mut()
                .transfer(from, to, amount, fee, memo, created_at_time)
        })
    }

    pub fn batch_transfer(
        from: AccountInternal,
        transfers: Vec<BatchTransferArgs>,
        fee: Tokens128,
    ) -> Vec<TxId> {
        LEDGER.with(|ledger| ledger.borrow_mut().batch_transfer(from, transfers, fee))
    }

    pub fn mint(from: AccountInternal, to: AccountInternal, amount: Tokens128) -> TxId {
        LEDGER.with(|ledger| ledger.borrow_mut().mint(from, to, amount))
    }

    pub fn burn(caller: AccountInternal, from: AccountInternal, amount: Tokens128) -> TxId {
        LEDGER.with(|ledger| ledger.borrow_mut().burn(caller, from, amount))
    }

    pub fn record_auction(to: Principal, amount: Tokens128) {
        LEDGER.with(|ledger| ledger.borrow_mut().record_auction(to, amount))
    }

    pub fn claim(claim_account: AccountInternal, to: AccountInternal, amount: Tokens128) -> TxId {
        LEDGER.with(|ledger| ledger.borrow_mut().claim(claim_account, to, amount))
    }

    pub fn clear() {
        LEDGER.with(|ledger| ledger.borrow_mut().clear())
    }
}

#[derive(Debug, Default, CandidType, Deserialize)]
pub struct Ledger {
    history: Vec<TxRecord>,
    vec_offset: u64,
}

impl Ledger {
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> u64 {
        self.vec_offset + self.history.len() as u64
    }

    fn next_id(&self) -> TxId {
        self.vec_offset + self.history.len() as u64
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
        let count = count as usize;
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
        if id < self.vec_offset || id > usize::MAX as TxId {
            None
        } else {
            Some((id - self.vec_offset) as usize)
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

        if self.history.len() > MAX_HISTORY_LENGTH + HISTORY_REMOVAL_BATCH_SIZE {
            // We remove first `HISTORY_REMOVAL_BATCH_SIZE` from the history at one go, to prevent
            // often relocation of the history vec.
            // This removal code can later be changed to moving old history records into another
            // storage.

            self.history = self.history[HISTORY_REMOVAL_BATCH_SIZE..].into();
            self.vec_offset += HISTORY_REMOVAL_BATCH_SIZE as u64;
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
        self.vec_offset = 0;
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
