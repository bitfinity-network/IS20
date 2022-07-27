use std::fmt::{Display, Formatter};

use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

pub static DEFAULT_SUBACCOUNT: Subaccount = [0u8; 32];

#[derive(Debug, Clone, CandidType, Deserialize, Copy, PartialEq, Eq, Serialize)]
pub struct Account {
    pub of: Principal,
    pub subaccount: Subaccount,
}

impl Account {
    pub fn new(of: Principal, subaccount: Option<Subaccount>) -> Self {
        Self {
            of,
            subaccount: subaccount.unwrap_or(DEFAULT_SUBACCOUNT),
        }
    }
}

impl From<Principal> for Account {
    fn from(principal: Principal) -> Self {
        Self::new(principal, None)
    }
}

impl Display for Account {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self.of)
    }
}

pub type Subaccount = [u8; 32];
