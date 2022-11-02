use std::fmt::{Display, Formatter};

use canister_sdk::candid::{CandidType, Principal};
use serde::Deserialize;

use crate::error::TxError;

pub static DEFAULT_SUBACCOUNT: Subaccount = [0u8; 32];

#[derive(Debug, Clone, CandidType, Deserialize, Copy, PartialEq, Eq)]
pub struct Account {
    pub owner: Principal,
    pub subaccount: Option<Subaccount>,
}

impl Account {
    pub fn new(owner: Principal, subaccount: Option<Subaccount>) -> Self {
        Self { owner, subaccount }
    }
}

// We use internal type separately from `Account` to make it semantically more correct. This
// simplifies, for example comparison of accounts with default subaccount.
#[derive(Debug, Clone, CandidType, Deserialize, Copy, PartialEq, Eq, Hash)]
pub struct AccountInternal {
    pub owner: Principal,
    pub subaccount: Subaccount,
}

impl AccountInternal {
    pub fn new(owner: Principal, subaccount: Option<Subaccount>) -> Self {
        Self {
            owner,
            subaccount: subaccount.unwrap_or(DEFAULT_SUBACCOUNT),
        }
    }
}

impl From<Principal> for AccountInternal {
    fn from(owner: Principal) -> Self {
        Self::new(owner, None)
    }
}

impl From<Principal> for Account {
    fn from(owner: Principal) -> Self {
        Self {
            owner,
            subaccount: None,
        }
    }
}

impl From<Account> for AccountInternal {
    fn from(acc: Account) -> Self {
        Self::new(acc.owner, acc.subaccount)
    }
}

impl From<AccountInternal> for Account {
    fn from(acc: AccountInternal) -> Self {
        let subaccount = if acc.subaccount == DEFAULT_SUBACCOUNT {
            None
        } else {
            Some(acc.subaccount)
        };

        Account {
            owner: acc.owner,
            subaccount,
        }
    }
}

impl Display for AccountInternal {
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

pub struct CheckedAccount<T>(AccountInternal, T);

impl<T> CheckedAccount<T> {
    pub fn inner(&self) -> AccountInternal {
        self.0
    }
}

pub struct WithRecipient {
    pub recipient: AccountInternal,
}

impl CheckedAccount<WithRecipient> {
    pub fn with_recipient(
        recipient: AccountInternal,
        from_subaccount: Option<Subaccount>,
    ) -> Result<Self, TxError> {
        let caller = canister_sdk::ic_kit::ic::caller();
        let from = AccountInternal::new(caller, from_subaccount);
        if recipient == from {
            Err(TxError::SelfTransfer)
        } else {
            Ok(Self(from, WithRecipient { recipient }))
        }
    }
    pub fn recipient(&self) -> AccountInternal {
        self.1.recipient
    }
}

#[cfg(test)]
mod tests {
    use candid::{Decode, Encode};
    use canister_sdk::ic_kit::mock_principals::alice;
    use coverage_helper::test;

    use super::*;

    #[test]
    fn compare_default_subaccount_and_none() {
        let acc1 = AccountInternal::new(alice(), None);
        let acc2 = AccountInternal::new(alice(), Some(DEFAULT_SUBACCOUNT));

        assert_eq!(acc1, acc2);
    }

    #[test]
    fn account_display() {
        assert_eq!(
            format!("{}", AccountInternal::new(alice(), None)),
            "Account(sgymv-uiaaa-aaaaa-aaaia-cai)".to_string()
        );
        assert_eq!(
            format!("{:?}", AccountInternal::new(alice(), None)),
            "AccountInternal { owner: Principal { len: 10, bytes: [0, 0, 0, 0, 0, 0, 0, 16, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] }, subaccount: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] }".to_string()
        );
        assert_eq!(
            format!(
                "{}",
                AccountInternal::new(alice(), Some(DEFAULT_SUBACCOUNT))
            ),
            "Account(sgymv-uiaaa-aaaaa-aaaia-cai)".to_string()
        );
        assert_eq!(
            format!("{}", AccountInternal::new(alice(), Some([1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,255]))),
            "Account(sgymv-uiaaa-aaaaa-aaaia-cai, 01000000000000000000000000000000000000000000000000000000000000FF)".to_string()
        );
    }

    #[test]
    fn serialization() {
        let acc = AccountInternal::new(alice(), Some([1; 32]));
        let serialized = Encode!(&acc).unwrap();
        let deserialized = Decode!(&serialized, AccountInternal).unwrap();

        assert_eq!(deserialized, acc);
    }
}
