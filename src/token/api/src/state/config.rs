use std::{borrow::Cow, cell::RefCell};

use canister_sdk::ic_helpers::tokens::Tokens128;
use ic_exports::candid::{CandidType, Decode, Deserialize, Encode, Int, Nat};
use ic_exports::Principal;
use ic_stable_structures::{MemoryId, StableCell, Storable};

#[derive(Deserialize, CandidType, Clone, Debug)]
pub struct TokenConfig {
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

impl TokenConfig {
    /// Get config data stored in stable memory.
    pub fn get_stable() -> TokenConfig {
        CELL.with(|c| c.borrow().get().clone())
    }

    /// Store config data in stable memory.
    pub fn set_stable(config: TokenConfig) {
        CELL.with(|c| c.borrow_mut().set(config))
            .expect("unable to set token config to stable memory")
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

    pub fn icrc1_metadata(&self) -> Vec<(String, Value)> {
        vec![
            ("icrc1:symbol".to_string(), Value::Text(self.symbol.clone())),
            ("icrc1:name".to_string(), Value::Text(self.name.clone())),
            (
                "icrc1:decimals".to_string(),
                Value::Nat(Nat::from(self.decimals)),
            ),
            ("icrc1:fee".to_string(), Value::Nat(self.fee.amount.into())),
        ]
    }

    pub fn get_metadata(&self) -> Metadata {
        Metadata {
            name: self.name.clone(),
            symbol: self.symbol.clone(),
            decimals: self.decimals,
            owner: self.owner,
            fee: self.fee,
            fee_to: self.fee_to,
            is_test_token: Some(self.is_test_token),
        }
    }
}

impl Default for TokenConfig {
    fn default() -> Self {
        TokenConfig {
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

impl Storable for TokenConfig {
    // Stable storage expects non-failing serialization/deserialization.

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Owned(Encode!(self).expect("failed to encode token config"))
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        Decode!(&bytes, Self).expect("failed to decode token config")
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

impl From<Metadata> for TokenConfig {
    fn from(md: Metadata) -> Self {
        Self {
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

#[derive(CandidType, Default, Debug, Copy, Clone, Deserialize, PartialEq)]
pub struct FeeRatio(f64);

impl FeeRatio {
    pub fn new(value: f64) -> Self {
        let adj_value = value.clamp(0.0, 1.0);
        Self(adj_value)
    }

    /// Returns the tupple (raw_fee, auction_fee). Raw fee is the fee amount to be transferred to
    /// the canister owner, and auction_fee is the portion of the fee for the cycle auction.
    pub(crate) fn get_value(&self, fee: Tokens128) -> (Tokens128, Tokens128) {
        // Both auction fee and owner fee have the same purpose of providing the tokens to pay for
        // the canister operations. As such we do not care much about rounding errors in this case.
        // The only important thing to make sure that the sum of auction fee and the owner fee is
        // equal to the total fee amount.
        let auction_fee_amount = Tokens128::from((f64::from(fee) * self.0) as u128);
        let owner_fee_amount = fee.saturating_sub(auction_fee_amount);

        (owner_fee_amount, auction_fee_amount)
    }
}

impl From<FeeRatio> for f64 {
    fn from(v: FeeRatio) -> Self {
        v.0
    }
}

const CONFIG_MEMORY_ID: MemoryId = MemoryId::new(0);

thread_local! {
    static CELL: RefCell<StableCell<TokenConfig>> = {
            RefCell::new(StableCell::new(CONFIG_MEMORY_ID, TokenConfig::default())
                .expect("stable memory token config initialization failed"))
    }
}
