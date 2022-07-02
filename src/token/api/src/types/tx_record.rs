use candid::{CandidType, Deserialize, Principal};
use ic_canister::ic_kit::ic;
use ic_helpers::tokens::Tokens128;

use crate::types::{Operation, TokenHolder, TokenReceiver, TransactionStatus, TxId};

// from can be either a TokenHolder or a TokenReceiver or a Principal.
#[derive(Deserialize, CandidType, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FromToOption {
    TokenHolder(TokenHolder),
    TokenReceiver(TokenReceiver),
    Principal(Principal),
}

#[derive(Deserialize, CandidType, Debug, Clone)]
pub struct TxRecord {
    pub caller: Option<Principal>,
    pub index: TxId,
    pub from: FromToOption,
    pub to: FromToOption,
    pub amount: Tokens128,
    pub fee: Tokens128,
    pub timestamp: u64,
    pub status: TransactionStatus,
    pub operation: Operation,
}

impl TxRecord {
    pub fn transfer(
        index: TxId,
        from: TokenHolder,
        to: TokenReceiver,
        amount: Tokens128,
        fee: Tokens128,
        caller: Principal,
    ) -> Self {
        Self {
            caller: Some(caller),
            index,
            from: FromToOption::TokenHolder(from),
            to: FromToOption::TokenReceiver(to),
            amount,
            fee,
            timestamp: ic::time(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Transfer,
        }
    }

    pub fn transfer_from(
        index: TxId,
        caller: Principal,
        from: TokenHolder,
        to: TokenReceiver,
        amount: Tokens128,
        fee: Tokens128,
    ) -> Self {
        Self {
            caller: Some(caller),
            index,
            from: FromToOption::TokenHolder(from),
            to: FromToOption::TokenReceiver(to),
            amount,
            fee,
            timestamp: ic::time(),
            status: TransactionStatus::Succeeded,
            operation: Operation::TransferFrom,
        }
    }

    pub fn approve(
        index: TxId,
        from: TokenHolder,
        to: TokenReceiver,
        amount: Tokens128,
        fee: Tokens128,
        caller: Principal,
    ) -> Self {
        Self {
            caller: Some(caller),
            index,
            from: FromToOption::TokenHolder(from),
            to: FromToOption::TokenReceiver(to),
            amount,
            fee,
            timestamp: ic::time(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Approve,
        }
    }

    pub fn mint(index: TxId, from: Principal, to: Principal, amount: Tokens128) -> Self {
        Self {
            caller: Some(from),
            index,
            from: FromToOption::Principal(from),
            to: FromToOption::Principal(to),
            amount,
            fee: Tokens128::from(0u128),
            timestamp: ic::time(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Mint,
        }
    }

    pub fn burn(index: TxId, caller: Principal, from: Principal, amount: Tokens128) -> Self {
        Self {
            caller: Some(caller),
            index,
            from: FromToOption::Principal(from),
            to: FromToOption::Principal(from),
            amount,
            fee: Tokens128::from(0u128),
            timestamp: ic::time(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Burn,
        }
    }

    pub fn auction(index: TxId, to: Principal, amount: Tokens128) -> Self {
        Self {
            caller: Some(ic_cdk::caller()),
            index,
            from: FromToOption::Principal(to),
            to: FromToOption::Principal(to),
            amount,
            fee: Tokens128::from(0u128),
            timestamp: ic::time(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Auction,
        }
    }
}
