use candid::{CandidType, Deserialize, Principal};
use ic_canister::ic_kit::ic;
use ic_helpers::tokens::Tokens128;

use crate::account::Account;
use crate::types::{Operation, Timestamp, TransactionStatus, TxId};

#[derive(Deserialize, CandidType, Debug, Clone)]
pub struct TxRecord {
    pub caller: Principal,
    pub index: TxId,
    pub from: Account,
    pub to: Account,
    pub amount: Tokens128,
    pub fee: Tokens128,
    pub timestamp: Timestamp,
    pub status: TransactionStatus,
    pub operation: Operation,
    pub memo: Option<u64>,
}

impl TxRecord {
    pub fn transfer(
        index: TxId,
        from: Account,
        to: Account,
        amount: Tokens128,
        fee: Tokens128,
        memo: Option<u64>,
        created_at_time: Option<Timestamp>,
    ) -> Self {
        Self {
            caller: from.account,
            index,
            from,
            to,
            amount,
            fee,
            timestamp: created_at_time.unwrap_or(ic::time()),
            status: TransactionStatus::Succeeded,
            operation: Operation::Transfer,
            memo,
        }
    }

    pub fn mint(index: TxId, from: Account, to: Account, amount: Tokens128) -> Self {
        Self {
            caller: from.account,
            index,
            from,
            to,
            amount,
            fee: Tokens128::from(0u128),
            timestamp: ic::time(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Mint,
            memo: None,
        }
    }

    pub fn burn(index: TxId, caller: Account, from: Account, amount: Tokens128) -> Self {
        Self {
            caller: caller.account,
            index,
            from,
            to: from,
            amount,
            fee: Tokens128::from(0u128),
            timestamp: ic::time(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Burn,
            memo: None,
        }
    }

    pub fn auction(index: TxId, to: Account, amount: Tokens128) -> Self {
        Self {
            caller: to.account,
            index,
            from: to,
            to,
            amount,
            fee: Tokens128::from(0u128),
            timestamp: ic::time(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Auction,
            memo: None,
        }
    }

    pub fn contains(&self, pid: Principal) -> bool {
        self.caller == pid || self.from.account == pid || self.to.account == pid
    }
}
