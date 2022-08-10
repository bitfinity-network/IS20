use candid::{CandidType, Deserialize};
use ic_helpers::tokens::Tokens128;
use thiserror::Error;

#[derive(CandidType, Debug, PartialEq, Deserialize, Error)]
pub enum TxError {
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Amount too small")]
    AmountTooSmall,
    #[error("Fee exceeded limit {fee_limit}")]
    FeeExceededLimit { fee_limit: Tokens128 },
    #[error("Already actioned")]
    AlreadyActioned,
    #[error("Transaction does not exist")]
    TransactionDoesNotExist,
    #[error("Bad fee {expected_fee}")]
    BadFee { expected_fee: Tokens128 },
    #[error("Insufficient funds : {balance}")]
    InsufficientFunds { balance: Tokens128 },
    #[error("Transaction is too old : {allowed_window_nanos}")]
    TooOld { allowed_window_nanos: u64 },
    #[error("Transaction is created in the future {ledger_time}")]
    CreatedInFuture { ledger_time: u64 },
    #[error("Transaction is duplicate of {duplicate_of}")]
    Duplicate { duplicate_of: u64 },
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
    #[error("Temporary unavailable")]
    TemporaryUnavailable,
}
