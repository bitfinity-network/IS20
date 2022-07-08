use std::fmt::{Display, Formatter};

use candid::{CandidType, Principal};

use serde::{Deserialize, Serialize};

pub static SUB_ACCOUNT_ZERO: Subaccount = Subaccount([0; 32]);

#[derive(Debug, Clone, CandidType, Deserialize, Copy, PartialEq, Eq, Serialize)]
pub struct Account {
    pub account: Principal,
    pub subaccount: Option<Subaccount>,
}

impl Account {
    pub fn new(account: Principal, subaccount: Option<Subaccount>) -> Self {
        Self {
            account,
            subaccount,
        }
    }
}

impl From<Principal> for Account {
    fn from(principal: Principal) -> Self {
        Self::new(principal, None)
    }
}

/// Subaccounts are arbitrary 32-byte values
#[derive(Serialize, Deserialize, CandidType, Clone, Hash, Debug, PartialEq, Eq, Copy)]
pub struct Subaccount(pub [u8; 32]);

impl Subaccount {
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl Default for Subaccount {
    fn default() -> Self {
        SUB_ACCOUNT_ZERO
    }
}

impl Display for Subaccount {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        hex::encode(self.0).fmt(f)
    }
}
