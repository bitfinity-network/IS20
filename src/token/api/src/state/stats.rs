use std::borrow::Cow;

use candid::{CandidType, Decode, Deserialize, Encode, Int, Nat, Principal};
use canister_sdk::ic_helpers::tokens::Tokens128;
use ic_stable_structures::{memory_manager::MemoryId, Storable};

use crate::storage::{self, StableCell};

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

impl StatsData {
    /// Get stats data stored in stable memory.
    pub fn get_stable() -> StatsData {
        Self::read_stable_cell().get().clone()
    }

    /// Store stats data in stable memory.
    pub fn set_stable(stats: StatsData) {
        Self::read_stable_cell()
            .set(stats)
            .expect("failed to set stats data to stable memory");
    }

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
                "IS20".to_string(),
                "https://github.com/infinity-swap/is20".to_string(),
            ),
        ]
    }

    fn read_stable_cell() -> StableCell<StatsData> {
        let memory = storage::get_memory_by_id(STATS_MEMORY_ID);
        let default_data = StatsData::default();
        StableCell::init(memory, default_data).expect("stats cell initialization failed")
    }
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

impl Storable for StatsData {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        Decode!(&bytes, Self).unwrap()
    }
}

#[derive(Debug, CandidType, Deserialize, Clone, PartialEq, Eq)]
pub struct StandardRecord {
    pub name: String,
    pub url: String,
}

impl StandardRecord {
    pub fn new(name: String, url: String) -> Self {
        Self { name, url }
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
    pub fee_to: Principal,
    pub is_test_token: Option<bool>,
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
            fee_to: md.fee_to,
            deploy_time: canister_sdk::ic_kit::ic::time(),
            min_cycles: DEFAULT_MIN_CYCLES,
            is_test_token: md.is_test_token.unwrap_or(false),
        }
    }
}

#[allow(non_snake_case)]
#[derive(Deserialize, CandidType, Clone, Debug)]
pub struct TokenInfo {
    pub metadata: Metadata,
    pub fee_to: Principal,
    pub history_size: u64,
    pub deployTime: Timestamp,
    pub holderNumber: usize,
    pub cycles: u64,
}

/// Variant type for the metadata endpoint
#[derive(Deserialize, CandidType, Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Nat(Nat),
    Int(Int),
    Text(String),
    Blob(Vec<u8>),
}

pub type Timestamp = u64;

const STATS_MEMORY_ID: MemoryId = MemoryId::new(0);
