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
    pub is_test_token: bool,
}

impl StatsData {
    pub fn fee_info(&self) -> (Nat, Principal) {
        (self.fee.clone(), self.fee_to)
    }
}

// 10T cycles is an equivalent of approximately $10. This should be enough to last the canister
// for the default auction cycle, which is 1 day.
const DEFAULT_MIN_CYCLES: u64 = 10_000_000_000_000;

impl From<Metadata> for StatsData {
    fn from(md: Metadata) -> Self {
        Self {
            logo: md.logo,
            name: md.name,
            symbol: md.symbol,
            decimals: md.decimals,
            total_supply: md.totalSupply,
            owner: md.owner,
            fee: md.fee,
            fee_to: md.feeTo,
            deploy_time: ic_kit::ic::time(),
            min_cycles: DEFAULT_MIN_CYCLES,
            is_test_token: md.isTestToken.unwrap_or(false),
        }
    }
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
            is_test_token: false,
        }
    }
}

pub type Allowances = HashMap<Principal, HashMap<Principal, Nat>>;

#[derive(CandidType, Debug, PartialEq, Deserialize)]
pub enum TxError {
    InsufficientBalance,
    InsufficientAllowance,
    // Storing owner and caller as strings for better readability
    Unauthorized { owner: String, caller: String },
    AmountTooSmall,
    FeeExceededLimit,
    NotificationFailed { cdk_msg: String },
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

/// Hash tree witness for a transaction with a certificate signed by the token canister and IC.
#[derive(CandidType, Debug, Clone, Deserialize)]
pub struct SignedTx {
    /// Id of the token canister.
    pub principal: Principal,

    /// Signed certificate. The certificate is in the format returned by `get_certificate` IC API
    /// call. The certified data in the certificate equals to the root hash of the witness.
    pub certificate: Vec<u8>,

    /// Hash tree serialized with `serde-cbor`. The hash tree is in the format defined in
    /// `ic-certified-map::HashTree` and contains one leaf node with the value of type [`TxRecord`].
    pub witness: Vec<u8>,
}
