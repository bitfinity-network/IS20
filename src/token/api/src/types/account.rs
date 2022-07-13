use std::fmt::{Display, Formatter};

use candid::{CandidType, Principal};
use ic_helpers::ledger::Subaccount;
use serde::{Deserialize, Serialize};

pub static SUB_ACCOUNT_ZERO: Subaccount = Subaccount([0; 32]);

#[derive(Debug, Clone, CandidType, Deserialize, Copy, PartialEq, Eq, Serialize)]
pub struct Account {
    pub account: Principal,
    pub subaccount: Subaccount,
}

impl Account {
    pub fn new(account: Principal, subaccount: Option<Subaccount>) -> Self {
        Self {
            account,
            subaccount: subaccount.unwrap_or(SUB_ACCOUNT_ZERO),
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
        write!(f, "{}", self.account)
    }
}
