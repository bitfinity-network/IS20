use std::collections::HashMap;

use candid::{CandidType, Deserialize, Int, Principal};
use ic_helpers::{ledger::AccountIdentifier, tokens::Tokens128};

pub use tx_record::*;

use crate::{
    account::{Account, Subaccount},
    error::TxError,
};

mod tx_record;

pub type Timestamp = u64;

pub const MAX_MEMO_LENGTH: usize = 32;

#[derive(Debug, Default, CandidType, Deserialize, Clone)]
pub struct Memo(Vec<u8>);

impl Memo {
    pub fn new(data: Vec<u8>) -> Result<Self, TxError> {
        if data.len() > MAX_MEMO_LENGTH {
            return Err(TxError::MemoTooLarge);
        }
        Ok(Self(data))
    }
}

impl From<[u8; MAX_MEMO_LENGTH]> for Memo {
    fn from(memo: [u8; MAX_MEMO_LENGTH]) -> Self {
        Self(memo.to_vec())
    }
}

impl From<u64> for Memo {
    fn from(memo: u64) -> Self {
        Self(memo.to_be_bytes().to_vec())
    }
}

#[allow(non_snake_case)]
#[derive(Deserialize, CandidType, Clone, Debug)]
pub struct Metadata {
    pub logo: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub owner: Principal,
    pub fee: Tokens128,
    pub feeTo: Principal,
    pub isTestToken: Option<bool>,
}

/// Variant type for the metadata endpoint
#[derive(Deserialize, CandidType, Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Nat(Tokens128),
    Int(Int),
    Text(String),
}

#[derive(Deserialize, CandidType, Clone, Debug)]
pub struct StatsData {
    pub logo: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub owner: Principal,
    pub fee: Tokens128,
    pub fee_to: Principal,
    pub deploy_time: u64,
    pub min_cycles: u64,
    pub is_test_token: bool,
}

#[derive(Debug, CandidType, Deserialize, Clone)]
pub struct StandardRecord {
    pub name: String,
    pub url: String,
}

impl StandardRecord {
    pub fn new(name: String, url: String) -> Self {
        Self { name, url }
    }
}

impl StatsData {
    pub fn fee_info(&self) -> (Tokens128, Principal) {
        (self.fee, self.fee_to)
    }

    pub fn supported_standards(&self) -> Vec<StandardRecord> {
        vec![
            StandardRecord::new(
                "ICRC-1".to_string(),
                "https://github.com/dfinity/ICRC-1".to_string(),
            ),
            StandardRecord::new(
                "IS-20".to_string(),
                "https://github.com/infinity-swap/is20".to_string(),
            ),
        ]
    }
}

// 10T cycles is an equivalent of approximately $10. This should be enough to last the canister
// for the default auction cycle, which is 1 day.
pub const DEFAULT_MIN_CYCLES: u64 = 10_000_000_000_000;

impl From<Metadata> for StatsData {
    fn from(md: Metadata) -> Self {
        Self {
            logo: md.logo,
            name: md.name,
            symbol: md.symbol,
            decimals: md.decimals,
            owner: md.owner,
            fee: md.fee,
            fee_to: md.feeTo,
            deploy_time: ic_canister::ic_kit::ic::time(),
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
    pub historySize: u64,
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
            owner: Principal::anonymous(),
            fee: Tokens128::from(0u128),
            fee_to: Principal::anonymous(),
            deploy_time: 0,
            min_cycles: 0,
            is_test_token: false,
        }
    }
}

/// This data structure is used for supporting minting to `AccountIdentifier`, after a claim is saved, We use the `claim` functions to claim the amount and is minted to `Account`.
pub type Claims = HashMap<AccountIdentifier, Tokens128>;

pub type TxReceipt = Result<u128, TxError>;

#[derive(CandidType, Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum TransactionStatus {
    Succeeded,
    Failed,
}

#[derive(CandidType, Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum Operation {
    Approve,
    Mint,
    Transfer,
    Burn,
    Auction,
}

#[derive(CandidType, Debug, Clone, Deserialize, PartialEq)]
pub struct AuctionInfo {
    pub auction_id: usize,
    pub auction_time: Timestamp,
    pub tokens_distributed: Tokens128,
    pub cycles_collected: Cycles,
    pub fee_ratio: f64,
    pub first_transaction_id: TxId,
    pub last_transaction_id: TxId,
}

/// `PaginatedResult` is returned by paginated queries i.e `getTransactions`.
#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct PaginatedResult {
    /// The result is the transactions which is the `count` transactions starting from `next` if it exists.
    pub result: Vec<TxRecord>,

    /// This is  the next `id` of the transaction. The `next` is used as offset for the next query if it exits.
    pub next: Option<TxId>,
}

pub type TxId = u64;
pub type Cycles = u64;

// Batch transfer arguments.
#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct BatchTransferArgs {
    pub receiver: Account,
    pub amount: Tokens128,
}

/// These are the arguments which are taken in the `icrc1_transfer`
#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct TransferArgs {
    pub from_subaccount: Option<Subaccount>,
    pub to: Account,
    pub amount: Tokens128,
    pub fee: Option<Tokens128>,
    pub memo: Option<Memo>,
    pub created_at_time: Option<Timestamp>,
}

/// `BalanceArgs` struct are the arguments which are taken in the `icrc1_balance_of`
#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct BalanceArgs {
    pub of: Principal,
    pub subaccount: Option<Subaccount>,
}

impl From<(Principal, Option<Subaccount>)> for BalanceArgs {
    fn from(from: (Principal, Option<Subaccount>)) -> Self {
        BalanceArgs {
            of: from.0,
            subaccount: from.1,
        }
    }
}
