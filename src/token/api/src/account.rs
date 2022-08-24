use std::fmt::{Display, Formatter};

use candid::{CandidType, Principal};
use serde::Deserialize;

use crate::error::TxError;

pub static DEFAULT_SUBACCOUNT: Subaccount = [0u8; 32];

#[derive(Debug, Clone, CandidType, Deserialize, Copy, PartialEq, Eq)]
pub struct Account {
    pub owner: Principal,
    pub subaccount: Subaccount,
}

impl Account {
    pub fn new(owner: Principal, subaccount: Option<Subaccount>) -> Self {
        Self {
            owner,
            subaccount: subaccount.unwrap_or(DEFAULT_SUBACCOUNT),
        }
    }
}

impl From<Principal> for Account {
    fn from(owner: Principal) -> Self {
        Self::new(owner, None)
    }
}

impl Display for Account {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        if self.subaccount == DEFAULT_SUBACCOUNT {
            write!(f, "Account({})", self.owner)
        } else {
            write!(f, "Account({}, ", self.owner)?;
            for b in self.subaccount {
                write!(f, "{b:02X}")?;
            }

            write!(f, ")")
        }
    }
}

pub type Subaccount = [u8; 32];

pub struct CheckedAccount<T>(Account, T);

impl<T> CheckedAccount<T> {
    pub fn inner(&self) -> Account {
        self.0
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

#[cfg(test)]
mod tests {
    use candid::{Decode, Encode};
    use coverage_helper::test;
    use ic_canister::ic_kit::mock_principals::alice;

    use super::*;

    #[test]
    fn compare_default_subaccount_and_none() {
        let acc1 = Account::new(alice(), None);
        let acc2 = Account::new(alice(), Some(DEFAULT_SUBACCOUNT));

        assert_eq!(acc1, acc2);
    }

    #[test]
    fn account_display() {
        assert_eq!(
            format!("{}", Account::new(alice(), None)),
            "Account(sgymv-uiaaa-aaaaa-aaaia-cai)".to_string()
        );
        assert_eq!(
            format!("{:?}", Account::new(alice(), None)),
            "Account { owner: Principal(PrincipalInner { len: 10, bytes: [0, 0, 0, 0, 0, 0, 0, 16, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] }), subaccount: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] }".to_string()
        );
        assert_eq!(
            format!("{}", Account::new(alice(), Some(DEFAULT_SUBACCOUNT))),
            "Account(sgymv-uiaaa-aaaaa-aaaia-cai)".to_string()
        );
        assert_eq!(
            format!("{}", Account::new(alice(), Some([1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,255]))),
            "Account(sgymv-uiaaa-aaaaa-aaaia-cai, 01000000000000000000000000000000000000000000000000000000000000FF)".to_string()
        );
    }

    #[test]
    fn serialization() {
        let acc = Account::new(alice(), Some([1; 32]));
        let serialized = Encode!(&acc).unwrap();
        let deserialized = Decode!(&serialized, Account).unwrap();

        assert_eq!(deserialized, acc);
    }
}
