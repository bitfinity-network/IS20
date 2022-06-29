use crate::state::STABLE_MAP;
use crate::types::{Operation, StableMap, TransactionStatus, TxId};
use candid::{CandidType, Deserialize, Principal};
use ic_canister::ic_kit::ic;
use ic_helpers::tokens::Tokens128;

const CALLER_MAGIC: &[u8; 3] = b"CAR";
const CALLER_LAYOUT_VERSION: u8 = 1;
const INDEX_MAGIC: &[u8; 3] = b"INX";
const INDEX_LAYOUT_VERSION: u8 = 1;
const FROM_MAGIC: &[u8; 3] = b"FRM";
const FROM_LAYOUT_VERSION: u8 = 1;
const TO_MAGIC: &[u8; 3] = b"TOM";
const TO_LAYOUT_VERSION: u8 = 1;
const AMOUNT_MAGIC: &[u8; 3] = b"AMT";
const AMOUNT_LAYOUT_VERSION: u8 = 1;
const FEE_MAGIC: &[u8; 3] = b"FEE";
const FEE_LAYOUT_VERSION: u8 = 1;
const TIME_MAGIC: &[u8; 3] = b"TIE";
const TIME_LAYOUT_VERSION: u8 = 1;
const STATUS_MAGIC: &[u8; 3] = b"STU";
const STATUS_LAYOUT_VERSION: u8 = 1;
const OPERATION_MAGIC: &[u8; 3] = b"OPN";
const OPERATION_LAYOUT_VERSION: u8 = 1;

#[derive(Deserialize, CandidType, Debug, Clone)]
pub struct TxRecord {
    pub caller: Option<Principal>,
    pub index: TxId,
    pub from: Principal,
    pub to: Principal,
    pub amount: Tokens128,
    pub fee: Tokens128,
    pub timestamp: u64,
    pub status: TransactionStatus,
    pub operation: Operation,
}

impl TxRecord {
    pub fn transfer(
        index: TxId,
        from: Principal,
        to: Principal,
        amount: Tokens128,
        fee: Tokens128,
    ) -> Self {
        Self {
            caller: Some(from),
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

    pub fn transfer_from(
        index: TxId,
        caller: Principal,
        from: Principal,
        to: Principal,
        amount: Tokens128,
        fee: Tokens128,
    ) -> Self {
        Self {
            caller: Some(caller),
            index,
            from,
            to,
            amount,
            fee,
            timestamp: ic::time(),
            status: TransactionStatus::Succeeded,
            operation: Operation::TransferFrom,
        }
    }

    pub fn approve(
        index: TxId,
        from: Principal,
        to: Principal,
        amount: Tokens128,
        fee: Tokens128,
    ) -> Self {
        Self {
            caller: Some(from),
            index,
            from,
            to,
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
            from,
            to,
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
            from,
            to: from,
            amount,
            fee: Tokens128::from(0u128),
            timestamp: ic::time(),
            status: TransactionStatus::Succeeded,
            operation: Operation::Burn,
        }
    }

    pub fn auction(index: TxId, to: Principal, amount: Tokens128) -> Self {
        Self {
            caller: Some(to),
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

#[derive(Debug, CandidType, Deserialize)]
pub struct TxRecordStable {
    pub caller: StableMap,
    pub index: StableMap,
    pub from: StableMap,
    pub to: StableMap,
    pub amount: StableMap,
    pub fee: StableMap,
    pub timestamp: StableMap,
    pub status: StableMap,
    pub operation: StableMap,
}

impl Default for TxRecordStable {
    fn default() -> Self {
        Self {
            caller: StableMap::new(*CALLER_MAGIC, CALLER_LAYOUT_VERSION),
            index: StableMap::new(*INDEX_MAGIC, INDEX_LAYOUT_VERSION),
            from: StableMap::new(*FROM_MAGIC, FROM_LAYOUT_VERSION),
            to: StableMap::new(*TO_MAGIC, TO_LAYOUT_VERSION),
            amount: StableMap::new(*AMOUNT_MAGIC, AMOUNT_LAYOUT_VERSION),
            fee: StableMap::new(*FEE_MAGIC, FEE_LAYOUT_VERSION),
            timestamp: StableMap::new(*TIME_MAGIC, TIME_LAYOUT_VERSION),
            status: StableMap::new(*STATUS_MAGIC, STATUS_LAYOUT_VERSION),
            operation: StableMap::new(*OPERATION_MAGIC, OPERATION_LAYOUT_VERSION),
        }
    }
}

impl TxRecordStable {
    pub fn get(&self, id: usize) -> Option<TxRecord> {
        STABLE_MAP.with(|s| {
            let map = s.borrow();
            let id = id as u64;
            let caller = self.caller.get::<u64, Option<Principal>>(&id, &map);
            let index = self.index.get::<u64, TxId>(&id, &map);
            let from = self.from.get::<u64, Principal>(&id, &map);
            let to = self.to.get::<u64, Principal>(&id, &map);
            let amount = self.amount.get::<u64, Tokens128>(&id, &map);
            let fee = self.fee.get::<u64, Tokens128>(&id, &map);
            let timestamp = self.timestamp.get::<u64, u64>(&id, &map);
            let status = self.status.get::<u64, TransactionStatus>(&id, &map);
            let operation = self.operation.get::<u64, Operation>(&id, &map);
            index.map(|index| TxRecord {
                caller: caller.unwrap(),
                index,
                from: from.unwrap(),
                to: to.unwrap(),
                amount: amount.unwrap(),
                fee: fee.unwrap(),
                timestamp: timestamp.unwrap(),
                status: status.unwrap(),
                operation: operation.unwrap(),
            })
        })
    }

    pub fn len(&self) -> usize {
        STABLE_MAP.with(|s| {
            let map = s.borrow();
            self.index.size(&map)
        })
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn push(&self, item: TxRecord, id: u64) {
        STABLE_MAP.with(|s| {
            let mut map = s.borrow_mut();
            self.caller
                .insert::<u64, Option<Principal>>(&id, &item.caller, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("TxRecordStable push error: {}", e))
                });
            self.index
                .insert::<u64, TxId>(&id, &item.index, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("TxRecordStable push error: {}", e))
                });
            self.from
                .insert::<u64, Principal>(&id, &item.from, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("TxRecordStable push error: {}", e))
                });
            self.to
                .insert::<u64, Principal>(&id, &item.to, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("TxRecordStable push error: {}", e))
                });
            self.amount
                .insert::<u64, Tokens128>(&id, &item.amount, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("TxRecordStable push error: {}", e))
                });
            self.fee
                .insert::<u64, Tokens128>(&id, &item.fee, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("TxRecordStable push error: {}", e))
                });
            self.timestamp
                .insert::<u64, u64>(&id, &item.timestamp, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("TxRecordStable push error: {}", e))
                });
            self.status
                .insert::<u64, TransactionStatus>(&id, &item.status, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("TxRecordStable push error: {}", e))
                });
            self.operation
                .insert::<u64, Operation>(&id, &item.operation, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("TxRecordStable push error: {}", e))
                });
        });
    }

    pub fn remove(&self, id: usize) {
        let id = id as u64;
        STABLE_MAP.with(|s| {
            let mut map = s.borrow_mut();
            self.caller.remove::<u64, Option<Principal>>(&id, &mut map);
            self.index.remove::<u64, Tokens128>(&id, &mut map);
            self.from.remove::<u64, Principal>(&id, &mut map);
            self.to.remove::<u64, Principal>(&id, &mut map);
            self.amount.remove::<u64, Tokens128>(&id, &mut map);
            self.fee.remove::<u64, Tokens128>(&id, &mut map);
            self.timestamp.remove::<u64, u64>(&id, &mut map);
            self.status.remove::<u64, TransactionStatus>(&id, &mut map);
            self.operation.remove::<u64, Operation>(&id, &mut map);
        });
    }
}
