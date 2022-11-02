use candid::{CandidType, Deserialize, Principal};
use canister_sdk::ic_helpers::tokens::Tokens128;
use canister_sdk::ic_kit::ic;

use crate::{
    account::{Account, AccountInternal},
    state::config::Timestamp,
    state::ledger::{Memo, Operation, TransactionStatus},
};

pub type TxId = u64;

// We use `Account` instead of `AccountInternal` in this structure for two reasons:
// 1. It was there before `AccountInternal` was introduced, so if we want to change this type, we
//    would need to introduce a new version of the state.
// 2. This structre is returned to the client by APIs, and it's prefered to use `Account` in APIs.
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
    pub memo: Option<Memo>,
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
            caller: from.owner,
            index,
            from: from.into(),
            to: to.into(),
            amount,
            fee,
            timestamp: created_at_time,
            status: TransactionStatus::Succeeded,
            operation: Operation::Transfer,
            memo,
        }
    }

    pub fn mint(
        index: TxId,
        from: AccountInternal,
        to: AccountInternal,
        amount: Tokens128,
    ) -> Self {
        Self {
            caller: from.owner,
            index,
            from: from.into(),
            to: to.into(),
            amount,
            fee: Tokens128::from(0u128),
            timestamp: ic::time(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Mint,
            memo: None,
        }
    }

    pub fn burn(
        index: TxId,
        caller: AccountInternal,
        from: AccountInternal,
        amount: Tokens128,
    ) -> Self {
        Self {
            caller: caller.owner,
            index,
            from: from.into(),
            to: from.into(),
            amount,
            fee: Tokens128::from(0u128),
            timestamp: ic::time(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Burn,
            memo: None,
        }
    }

    pub fn auction(index: TxId, to: AccountInternal, amount: Tokens128) -> Self {
        Self {
            caller: to.owner,
            index,
            from: to.into(),
            to: to.into(),
            amount,
            fee: Tokens128::from(0u128),
            timestamp: ic::time(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Auction,
            memo: None,
        }
    }

    // This is a helper funntion to compare the principal of a transaction record.
    pub fn contains(&self, pid: Principal) -> bool {
        self.caller == pid || self.from.owner == pid || self.to.owner == pid
    }

    pub fn claim(id: u64, from: AccountInternal, to: AccountInternal, amount: Tokens128) -> Self {
        Self {
            caller: to.owner,
            index: id,
            from: from.into(),
            to: to.into(),
            amount,
            fee: 0.into(),
            timestamp: ic::time(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Claim,
            memo: None,
        }
    }
}
