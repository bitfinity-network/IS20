use candid::{CandidType, Deserialize, Nat, Principal};
use ic_helpers::is20::TxError;
use std::collections::HashMap;

mod tx_record;
pub use tx_record::*;

pub type Timestamp = u64;

#[allow(non_snake_case)]
#[derive(Deserialize, CandidType, Clone, Debug)]
pub struct Metadata {
    pub logo: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub totalSupply: Nat,
    pub owner: Principal,
    pub fee: Nat,
    pub feeTo: Principal,
    pub isTestToken: Option<bool>,
}

#[derive(CandidType, Debug, Clone, Deserialize)]
pub struct SignedTx {
    /// Principal of token that called `receive_is20`
    pub principal: Principal,
    /// Pubkey associated to a principal.
    pub publickey: Vec<u8>,
    /// Transaction signing signature on behalf of `publickey`.
    pub signature: Vec<u8>,
    /// Transaction serialized with `serde-cbor`.
    pub serialized_tx: Vec<u8>,
    /// Sha256 hash of serialized transaction.
    pub hash: Vec<u8>,
}

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

pub type TxReceipt = Result<Nat, TxError>;

#[derive(CandidType, Debug, Eq, PartialEq, Deserialize)]
pub enum TokenError {
    TransactionError(TxError),
    SignatureError(String),
    PubkeyError(String),
}

#[derive(CandidType, Debug, Clone, Copy, Deserialize, PartialEq, Hash)]
pub enum TransactionStatus {
    Succeeded,
    Failed,
}

#[derive(CandidType, Debug, Clone, Copy, Deserialize, PartialEq, Hash)]
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
