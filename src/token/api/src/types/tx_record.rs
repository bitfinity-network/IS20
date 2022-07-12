use candid::{CandidType, Deserialize, Principal};
use ic_canister::ic_kit::ic;
use ic_helpers::tokens::Tokens128;

use crate::types::{Account, Operation, TransactionStatus, TxId};

#[derive(Deserialize, CandidType, Debug, Clone)]
pub struct TxRecord {
    pub caller: Principal,
    pub index: TxId,
    pub from: Account,
    pub to: Account,
    pub amount: Tokens128,
    pub fee: Tokens128,
    pub timestamp: u64,
    pub status: TransactionStatus,
    pub operation: Operation,
}

impl TxRecord {
    pub fn transfer(
        index: TxId,
        from: Account,
        to: Account,
        amount: Tokens128,
        fee: Tokens128,
    ) -> Self {
        Self {
            caller: from.account,
            index,
            from,
            to,
            amount,
            fee,
            timestamp: ic::time(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Transfer,
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
        }
    }
}
