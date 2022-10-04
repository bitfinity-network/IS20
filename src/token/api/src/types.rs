use candid::{CandidType, Deserialize};
use canister_sdk::{ic_auction::state::Timestamp, ic_helpers::tokens::Tokens128};

use crate::{
    account::{Account, Subaccount},
    error::TxError,
    tx_record::{TxId, TxRecord},
};

pub type Memo = [u8; 32];

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
