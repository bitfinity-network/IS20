use crate::types::{Operation, TransactionStatus};
use candid::{CandidType, Deserialize, Int, Nat, Principal};
use ic_kit::ic;

#[derive(Deserialize, CandidType, Debug, Clone, Hash)]
pub struct TxRecord {
    pub caller: Option<Principal>,
    pub index: Nat,
    pub from: Principal,
    pub to: Principal,
    pub amount: Nat,
    pub fee: Nat,
    pub timestamp: Int,
    pub status: TransactionStatus,
    pub operation: Operation,
}

impl TxRecord {
    pub fn transfer(index: Nat, from: Principal, to: Principal, amount: Nat, fee: Nat) -> Self {
        Self {
            caller: Some(from),
            index,
            from,
            to,
            amount,
            fee,
            timestamp: ic::time().into(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Transfer,
        }
    }

    pub fn transfer_from(
        index: Nat,
        caller: Principal,
        from: Principal,
        to: Principal,
        amount: Nat,
        fee: Nat,
    ) -> Self {
        Self {
            caller: Some(caller),
            index,
            from,
            to,
            amount,
            fee,
            timestamp: ic::time().into(),
            status: TransactionStatus::Succeeded,
            operation: Operation::TransferFrom,
        }
    }

    pub fn approve(index: Nat, from: Principal, to: Principal, amount: Nat, fee: Nat) -> Self {
        Self {
            caller: Some(from),
            index,
            from,
            to,
            amount,
            fee,
            timestamp: ic::time().into(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Approve,
        }
    }

    pub fn mint(index: Nat, from: Principal, to: Principal, amount: Nat) -> Self {
        Self {
            caller: Some(from),
            index,
            from,
            to,
            amount,
            fee: Nat::from(0),
            timestamp: ic::time().into(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Mint,
        }
    }

    pub fn burn(index: Nat, caller: Principal, amount: Nat) -> Self {
        Self {
            caller: Some(caller),
            index,
            from: caller,
            to: caller,
            amount,
            fee: Nat::from(0),
            timestamp: ic::time().into(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Burn,
        }
    }

    pub fn auction(index: Nat, to: Principal, amount: Nat) -> Self {
        Self {
            caller: Some(to),
            index,
            from: to,
            to,
            amount,
            fee: Nat::from(0),
            timestamp: ic::time().into(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Auction,
        }
    }
}

pub fn get_key_bytes(key: &Nat) -> Vec<u8> {
    key.0.to_bytes_be()
}

pub fn get_tx_bytes(tx: &TxRecord) -> Vec<u8> {
    use ic_cdk::export::candid::Encode;
    Encode!(tx).unwrap()
}

pub fn tx_from_bytes(bytes: &[u8]) -> TxRecord {
    use ic_cdk::export::candid::Decode;
    Decode!(bytes, TxRecord).unwrap()
}
