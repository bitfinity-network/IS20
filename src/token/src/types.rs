use candid::{CandidType, Deserialize, Nat, Principal};
use common::types::Metadata;
use std::collections::{HashMap, HashSet};

mod tx_record;
pub use tx_record::*;

pub type Timestamp = u64;

#[derive(Deserialize, CandidType, Clone, Debug)]
pub struct StatsData {
    pub logo: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_supply: Nat,
    pub owner: Principal,
    pub fee: Nat,
    pub fee_to: Principal,
    pub deploy_time: u64,
    pub min_cycles: u64,
}

#[allow(non_snake_case)]
#[derive(Deserialize, CandidType, Clone, Debug)]
pub struct TokenInfo {
    pub metadata: Metadata,
    pub feeTo: Principal,
    pub historySize: Nat,
    pub deployTime: Timestamp,
    pub holderNumber: usize,
    pub cycles: u64,
}

impl Default for StatsData {
    fn default() -> Self {
        StatsData {
            logo: "".to_string(),
            name: "".to_string(),
            symbol: "".to_string(),
            decimals: 0u8,
            total_supply: Nat::from(0),
            owner: Principal::anonymous(),
            fee: Nat::from(0),
            fee_to: Principal::anonymous(),
            deploy_time: 0,
            min_cycles: 0,
        }
    }
}

pub type Allowances = HashMap<Principal, HashMap<Principal, Nat>>;

#[derive(CandidType, Debug, PartialEq)]
pub enum TxError {
    InsufficientBalance,
    InsufficientAllowance,
    Unauthorized,
    AmountTooSmall,
    FeeExceededLimit,
    NotificationFailed,
    AlreadyNotified,
    TransactionDoesNotExist,
}

pub type TxReceipt = Result<Nat, TxError>;
pub type PendingNotifications = HashSet<Nat>;

#[derive(CandidType, Debug, Clone, Copy, Deserialize, PartialEq)]
pub enum TransactionStatus {
    Succeeded,
    Failed,
}

#[derive(CandidType, Debug, Clone, Copy, Deserialize, PartialEq)]
pub enum Operation {
    Approve,
    Mint,
    Transfer,
    TransferFrom,
    Burn,
    Auction,
}

#[derive(CandidType, Debug, Clone, Deserialize, PartialEq)]
pub struct AuctionInfo {
    pub auction_id: usize,
    pub auction_time: Timestamp,
    pub tokens_distributed: Nat,
    pub cycles_collected: u64,
    pub fee_ratio: f64,
    pub first_transaction_id: Nat,
    pub last_transaction_id: Nat,
}
