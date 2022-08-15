use std::fmt::{Display, Formatter};

use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

use crate::error::TxError;

pub static DEFAULT_SUBACCOUNT: Subaccount = [0u8; 32];

#[derive(Debug, Clone, CandidType, Deserialize, Copy, PartialEq, Eq, Serialize)]
pub struct Account {
    pub owner: Principal,
    pub subaccount: Option<Subaccount>,
}

impl Account {
    pub fn new(owner: Principal, subaccount: Option<Subaccount>) -> Self {
        Self { owner, subaccount }
    }
}

impl From<Principal> for Account {
    fn from(owner: Principal) -> Self {
        Self::new(owner, None)
    }
}

impl From<(Principal, Option<Subaccount>)> for Account {
    fn from(from: (Principal, Option<Subaccount>)) -> Self {
        Self::new(from.0, from.1)
    }
}

impl Display for Account {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self.owner)
    }
}

pub type Subaccount = [u8; 32];

pub struct CheckedAccount<T>(Account, T);

impl<T> CheckedAccount<T> {
    pub fn inner(&self) -> Account {
        self.0
    }

    pub fn owner(&self) -> Principal {
        self.0.owner
    }

    pub fn subaccount(&self) -> Subaccount {
        self.0.subaccount.unwrap_or(DEFAULT_SUBACCOUNT)
    }
}

pub struct WithRecipient {
    pub recipient: Account,
}

impl CheckedAccount<WithRecipient> {
    pub fn with_recipient(
        recipient: Account,
        from_subaccount: Option<Subaccount>,
    ) -> Result<Self, TxError> {
        let caller = ic_canister::ic_kit::ic::caller();
        let from = Account::new(caller, from_subaccount);
        if recipient == from {
            Err(TxError::SelfTransfer)
        } else {
            Ok(Self(from, WithRecipient { recipient }))
        }
    }
    pub fn recipient(&self) -> Account {
        self.1.recipient
    }
}
