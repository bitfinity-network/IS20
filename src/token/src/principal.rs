use candid::Principal;

use crate::types::{StatsData, TxError};
use ic_canister::ic_kit::ic;

/// Canister owner
pub struct Owner;

/// Any principal but the canister
/// has isTestToken set to true
pub struct TestNet;

/// The caller is not the recipient.
/// This is used when making transfers
pub struct WithRecipient {
    recipient: Principal,
}

pub struct CheckedPrincipal<T>(Principal, T);

impl<T> CheckedPrincipal<T> {
    pub fn inner(&self) -> Principal {
        self.0
    }
}

impl CheckedPrincipal<Owner> {
    pub fn owner(stats: &StatsData) -> Result<Self, TxError> {
        let caller = ic::caller();
        if caller == stats.owner {
            Ok(Self(caller, Owner))
        } else {
            Err(TxError::Unauthorized)
        }
    }
}

impl CheckedPrincipal<TestNet> {
    pub fn test_user(stats: &StatsData) -> Result<Self, TxError> {
        let caller = ic::caller();
        if stats.is_test_token {
            Ok(Self(caller, TestNet))
        } else {
            Err(TxError::Unauthorized)
        }
    }
}

impl CheckedPrincipal<WithRecipient> {
    pub fn with_recipient(recipient: Principal) -> Result<Self, TxError> {
        let caller = ic::caller();
        if caller == recipient {
            Err(TxError::SelfTransfer)
        } else {
            Ok(Self(caller, WithRecipient { recipient }))
        }
    }

    pub fn recipient(&self) -> Principal {
        self.1.recipient
    }
}
