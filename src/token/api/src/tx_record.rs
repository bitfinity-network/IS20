use candid::{CandidType, Deserialize, Principal};
use canister_sdk::ic_helpers::tokens::Tokens128;
use canister_sdk::ic_kit::ic;

use crate::{
    account::{Account, AccountInternal},
    types::{Memo, Timestamp},
};

pub type TxId = u64;

// We use `Account` instead of `AccountInternal` in this structure for two reasons:
// 1. It was there before `AccountInternal` was introduced, so if we want to change this type, we
//    would need to introduce a new version of the state.
// 2. This structure is returned to the client by APIs, and it's preferred to use `Account` in APIs.
#[derive(Deserialize, CandidType, Debug, Clone)]
pub struct TxRecord {
    pub index: TxId,
    pub operation: Operation,
    pub memo: Option<Memo>,
    pub created_at_time: Timestamp,
}

#[derive(CandidType, Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum Operation {
    Mint {
        from: Account,
        to: Account,
        amount: Tokens128,
    },
    Transfer {
        from: Account,
        to: Account,
        amount: Tokens128,
        fee: Tokens128,
    },
    Burn {
        from: Account,
        amount: Tokens128,
    },
    Auction {
        to: Account,
        amount: Tokens128,
    },
    Claim {
        from: Account,
        to: Account,
        amount: Tokens128,
    },
}

impl TxRecord {
    pub fn transfer(
        index: TxId,
        from: AccountInternal,
        to: AccountInternal,
        amount: Tokens128,
        fee: Tokens128,
        memo: Option<Memo>,
        created_at_time: Timestamp,
    ) -> Self {
        Self {
            index,
            operation: Operation::Transfer {
                from: from.into(),
                to: to.into(),
                amount,
                fee,
            },
            memo,
            created_at_time,
        }
    }

    pub fn mint(
        index: TxId,
        from: AccountInternal,
        to: AccountInternal,
        amount: Tokens128,
    ) -> Self {
        Self {
            index,
            operation: Operation::Mint {
                from: from.into(),
                to: to.into(),
                amount,
            },
            memo: None,
            created_at_time: ic::time(),
        }
    }

    pub fn burn(index: TxId, from: AccountInternal, amount: Tokens128) -> Self {
        Self {
            index,
            operation: Operation::Burn {
                from: from.into(),
                amount,
            },
            memo: None,
            created_at_time: ic::time(),
        }
    }

    pub fn auction(index: TxId, to: AccountInternal, amount: Tokens128) -> Self {
        Self {
            index,
            operation: Operation::Auction {
                to: to.into(),
                amount,
            },
            memo: None,
            created_at_time: ic::time(),
        }
    }

    pub fn claim(
        index: TxId,
        from: AccountInternal,
        to: AccountInternal,
        amount: Tokens128,
    ) -> Self {
        Self {
            index,
            operation: Operation::Claim {
                from: from.into(),
                to: to.into(),
                amount,
            },
            memo: None,
            created_at_time: ic::time(),
        }
    }

    // This is a helper function to compare the principal of a transaction record.
    pub fn contains(&self, pid: Principal) -> bool {
        match &self.operation {
            Operation::Mint { from, to, .. } => from.owner == pid || to.owner == pid,
            Operation::Transfer { from, to, .. } => from.owner == pid || to.owner == pid,
            Operation::Burn { from, .. } => from.owner == pid,
            Operation::Auction { to, .. } => to.owner == pid,
            Operation::Claim { from, to, .. } => from.owner == pid || to.owner == pid,
        }
    }

    // extract all the fields from the operation
    pub fn extract(&self) -> (Account, Account, Tokens128, Tokens128) {
        match &self.operation {
            Operation::Mint { from, to, amount } => (*from, *to, *amount, Tokens128::ZERO),
            Operation::Transfer {
                from,
                to,
                amount,
                fee,
            } => (*from, *to, *amount, *fee),
            Operation::Burn { from, amount } => (*from, *from, *amount, Tokens128::ZERO),
            Operation::Auction { to, amount } => (*to, *to, *amount, Tokens128::ZERO),
            Operation::Claim { from, to, amount } => (*from, *to, *amount, Tokens128::ZERO),
        }
    }
}
