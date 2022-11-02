use crate::state::config::Timestamp;
use candid::{CandidType, Deserialize};
use canister_sdk::ic_helpers::tokens::Tokens128;
use thiserror::Error;

#[derive(CandidType, Debug, PartialEq, Deserialize, Error, Eq)]
pub enum TxError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("amount too small")]
    AmountTooSmall,
    #[error("bad fee {expected_fee}")]
    BadFee { expected_fee: Tokens128 },
    #[error("insufficient funds : {balance}")]
    InsufficientFunds { balance: Tokens128 },
    #[error("transaction is too old : {allowed_window_nanos}")]
    TooOld { allowed_window_nanos: u64 },
    #[error("transaction is created in the future {ledger_time}")]
    CreatedInFuture { ledger_time: u64 },
    #[error("transaction is duplicate of {duplicate_of}")]
    Duplicate { duplicate_of: u64 },
    #[error("self transfer")]
    SelfTransfer,
    #[error("amount overflow")]
    AmountOverflow,
    #[error("account is not found")]
    AccountNotFound,
    #[error("no claimable tokens are on the requested subaccount")]
    NothingToClaim,
}

// This type is the exact error type from ICRC-1 standard. We use it as the return type for
// icrc1_transfer method to fully comply with the standard. As such, it doesn't need to implement
// `Error` trait, as internally everywhere the `TxError` is used.
#[derive(CandidType, Debug, PartialEq, Deserialize, Eq)]
pub enum TransferError {
    BadFee { expected_fee: Tokens128 },
    BadBurn { min_burn_amount: Tokens128 },
    InsufficientFunds { balance: Tokens128 },
    TooOld,
    CreatedInFuture { ledger_time: Timestamp },
    Duplicate { duplicate_of: u128 },
    TemporarilyUnavailable,
    GenericError { error_code: u128, message: String },
}

impl From<TxError> for TransferError {
    fn from(err: TxError) -> Self {
        match err {
            TxError::BadFee { expected_fee } => Self::BadFee { expected_fee },
            TxError::InsufficientFunds { balance } => Self::InsufficientFunds { balance },
            TxError::TooOld { .. } => Self::TooOld,
            TxError::CreatedInFuture { ledger_time } => Self::CreatedInFuture { ledger_time },
            TxError::Duplicate { duplicate_of } => Self::Duplicate {
                duplicate_of: duplicate_of as u128,
            },
            _ => TransferError::GenericError {
                error_code: 500,
                message: format!("{err}"),
            },
        }
    }
}
