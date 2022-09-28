use candid::CandidType;
use canister_sdk::ic_factory::error::FactoryError;
use thiserror::Error;

#[derive(Debug, Error, CandidType)]
pub enum TokenFactoryError {
    #[error("the property {0} has invalid value: {0}")]
    InvalidConfiguration(&'static str, &'static str),

    #[error("a token with the same name is already registered")]
    AlreadyExists,

    #[error(transparent)]
    FactoryError(#[from] FactoryError),
}
