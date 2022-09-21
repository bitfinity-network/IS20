use candid::Principal;

use crate::error::TxError;
use crate::types::StatsData;
use canister_sdk::ic_kit::ic;

/// Canister owner
pub struct Owner;

/// Any principal but the canister
/// has is_test_token set to true
pub struct TestNet;

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
