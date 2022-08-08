use candid::{CandidType, Deserialize};
use ic_helpers::tokens::Tokens128;
use thiserror::Error;

// TODO: a wrapper over `ic_helpers::TxError`, this is a most likely
// place to make tests fail in amm.
#[derive(CandidType, Debug, PartialEq, Deserialize, Error)]
pub enum TxError {
    #[error("Insufficient balance")]
    InsufficientBalance,
    #[error("Insufficient Allowance")]
    InsufficientAllowance,
    #[error("No Allowance")]
    NoAllowance,
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Amount too small")]
    AmountTooSmall,
    #[error("Fee exceeded limit {fee_limit}")]
    FeeExceededLimit { fee_limit: Tokens128 },
    #[error("Approve succeeded but notify failed : {tx_error}")]
    ApproveSucceededButNotifyFailed { tx_error: Box<TxError> },
    #[error("Notification failed for transaction : {transaction_id}")]
    NotificationFailed { transaction_id: u64 },
    #[error("Already actioned")]
    AlreadyActioned,
    #[error("Notification does not exist")]
    NotificationDoesNotExist,
    #[error("Transaction does not exist")]
    TransactionDoesNotExist,
    #[error("Bad fee {expected_fee}")]
    BadFee { expected_fee: Tokens128 },
    #[error("Insufficient funds : {balance}")]
    InsufficientFunds { balance: Tokens128 },
    #[error("Transaction is too old : {allowed_window_nanos}")]
    TxTooOld { allowed_window_nanos: u64 },
    #[error("Transaction is created in the future")]
    TxCreatedInFuture,
    #[error("Transaction is duplicate of {duplicate_of}")]
    TxDuplicate { duplicate_of: u64 },
    #[error("Self transfer")]
    SelfTransfer,
    #[error("Amount overflow")]
    AmountOverflow,
    #[error("Account is not found")]
    AccountNotFound,
    #[error("{message}")]
    GenericError { message: String },
    #[error("Claim not Allowed")]
    ClaimNotAllowed,
}
