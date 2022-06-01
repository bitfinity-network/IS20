use candid::Principal;
use std::marker::PhantomData;

use crate::types::{StatsData, TxError};

/// Canister owner
pub struct Owner;

/// Any principal but the canister
/// has isTestToken set to true
pub struct TestNet;

pub struct CheckedPrincipal<T>(Principal, PhantomData<T>);

impl<T> CheckedPrincipal<T> {
    pub fn inner(&self) -> Principal {
        self.0
    }
}

impl CheckedPrincipal<Owner> {
    pub fn owner(stats: &StatsData) -> Result<Self, TxError> {
        let caller = ic_kit::ic::caller();
        if caller == stats.owner {
            Ok(Self(caller, PhantomData))
        } else {
            Err(TxError::Unauthorized)
        }
    }
}

impl CheckedPrincipal<TestNet> {
    pub fn test_user(stats: &StatsData) -> Result<Self, TxError> {
        let caller = ic_kit::ic::caller();
        if stats.is_test_token {
            Ok(Self(caller, PhantomData))
        } else {
            Err(TxError::Unauthorized)
        }
    }
}