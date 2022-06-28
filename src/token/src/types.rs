use crate::state::STABLE_MAP;
use candid::{CandidType, Deserialize, Nat, Principal};
use common::types::Metadata;
use stable_structures::{
    btreemap::{iter::Iter, InsertError},
    stable_storage::StableStorage,
    RestrictedMemory, StableBTreeMap,
};
use std::collections::VecDeque;

mod tx_record;
pub use tx_record::*;

pub type Timestamp = u64;

const STATS_MAGIC: &[u8; 3] = b"STS";
const STATS_LAYOUT_VERSION: u8 = 1;
const ALLOW_MAGIC: &[u8; 3] = b"ALW";
const ALLOW_LAYOUT_VERSION: u8 = 1;
const PEND_NOTICE_MAGIC: &[u8; 3] = b"PNE";
const PEND_NOTICE_LAYOUT_VERSION: u8 = 1;
const AUCTION_ID_MAGIC: &[u8; 3] = b"AID";
const AUCTION_ID_LAYOUT_VERSION: u8 = 1;
const AUCTION_TIME_MAGIC: &[u8; 3] = b"ATE";
const AUCTION_TIME_LAYOUT_VERSION: u8 = 1;
const TOKENS_DIST_MAGIC: &[u8; 3] = b"TDT";
const TOKENS_DIST_LAYOUT_VERSION: u8 = 1;
const CYCLES_COLLECT_MAGIC: &[u8; 3] = b"CCT";
const CYCLES_COLLECT_LAYOUT_VERSION: u8 = 1;
const FEE_RATIO_MAGIC: &[u8; 3] = b"FRO";
const FEE_RATIO_LAYOUT_VERSION: u8 = 1;
const FIRST_TX_MAGIC: &[u8; 3] = b"FTX";
const FIRST_TX_LAYOUT_VERSION: u8 = 1;
const LAST_TX_MAGIC: &[u8; 3] = b"LTX";
const LAST_TX_LAYOUT_VERSION: u8 = 1;

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

struct StatsDataHeader {
    magic: [u8; 3],
    version: u8,
    logo: String,
    name: String,
    symbol: String,
    decimals: u8,
    total_supply: Nat,
    owner: Principal,
    fee: Nat,
    fee_to: Principal,
    deploy_time: u64,
    min_cycles: u64,
    is_test_token: bool,
}

impl StatsData {
    pub fn fee_info(&self) -> (Nat, Principal) {
        (self.fee.clone(), self.fee_to)
    }

    pub fn save_header(&self, memory: &RestrictedMemory<StableStorage>) {
        memory.write_struct::<StatsDataHeader>(&StatsDataHeader::from(self), 0);
    }

    pub fn load_header(&mut self, memory: &RestrictedMemory<StableStorage>) {
        let header: StatsDataHeader = memory.read_struct(0);
        assert_eq!(&header.magic, STATS_MAGIC, "Bad magic.");
        assert_eq!(header.version, STATS_LAYOUT_VERSION, "Unsupported version.");
        self.logo = header.logo;
        self.name = header.name;
        self.symbol = header.symbol;
        self.decimals = header.decimals;
        self.total_supply = header.total_supply;
        self.owner = header.owner;
        self.fee = header.fee;
        self.fee_to = header.fee_to;
        self.deploy_time = header.deploy_time;
        self.min_cycles = header.min_cycles;
        self.is_test_token = header.is_test_token;
    }
}

impl From<&StatsData> for StatsDataHeader {
    fn from(value: &StatsData) -> Self {
        Self {
            magic: *STATS_MAGIC,
            version: STATS_LAYOUT_VERSION,
            logo: value.logo.clone(),
            name: value.name.clone(),
            symbol: value.symbol.clone(),
            decimals: value.decimals,
            total_supply: value.total_supply.clone(),
            owner: value.owner,
            fee: value.fee.clone(),
            fee_to: value.fee_to,
            deploy_time: value.deploy_time,
            min_cycles: value.min_cycles,
            is_test_token: value.is_test_token,
        }
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

#[derive(Debug, CandidType, Deserialize)]
pub struct Allowances(pub StableMap);

impl Default for Allowances {
    fn default() -> Self {
        Self(StableMap::new(*ALLOW_MAGIC, ALLOW_LAYOUT_VERSION))
    }
}

impl Allowances {
    pub fn encode_key(&self, owner: &Principal, spender: &Principal) -> Vec<u8> {
        let mut buf: Vec<u8> = vec![];
        let owner = owner.as_slice();
        let spender = spender.as_slice();
        buf.extend(self.0.magic);
        buf.push(owner.len() as u8);
        buf.extend(owner);
        buf.push(spender.len() as u8);
        buf.extend(spender);
        buf
    }
    pub fn decode_key(&self, key: Vec<u8>) -> (Principal, Principal) {
        let mut buf: VecDeque<u8> = VecDeque::from(key);

        let prefix: Vec<u8> = buf.drain(0..3).collect();
        assert_eq!(prefix, &self.0.magic);
        let owner_size = buf
            .pop_front()
            .unwrap_or_else(|| ic_canister::ic_kit::ic::trap("failed to decode allowance key"))
            as usize;
        let owner: Vec<u8> = buf.drain(0..owner_size).collect();
        let spender_size = buf
            .pop_front()
            .unwrap_or_else(|| ic_canister::ic_kit::ic::trap("failed to decode allowance key"))
            as usize;
        let spender: Vec<u8> = buf.drain(0..spender_size).collect();
        (
            Principal::from_slice(&owner),
            Principal::from_slice(&spender),
        )
    }

    pub fn get(&self, owner: &Principal, spender: &Principal) -> Option<Nat> {
        let key = self.encode_key(owner, spender);
        STABLE_MAP.with(|s| {
            let map = s.borrow();
            map.get(&key).map(|v| self.0.val_decode::<Nat>(&v))
        })
    }

    pub fn insert(
        &self,
        owner: &Principal,
        spender: &Principal,
        value: Nat,
    ) -> Result<Option<Nat>, InsertError> {
        STABLE_MAP.with(|s| {
            let mut map = s.borrow_mut();
            let key = self.encode_key(owner, spender);
            let val = self.0.val_encode::<Nat>(&value);
            let result = map.insert(key, val)?;
            match result {
                Some(v) => Ok(Some(self.0.val_decode(&v))),
                None => Ok(None),
            }
        })
    }

    pub fn remove(&self, owner: &Principal, spender: &Principal) -> Option<Nat> {
        STABLE_MAP.with(|s| {
            let mut map = s.borrow_mut();
            let key = self.encode_key(owner, spender);
            map.remove(&key).map(|v| self.0.val_decode(&v))
        })
    }

    pub fn len(&self) -> usize {
        STABLE_MAP.with(|s| {
            let map = s.borrow();
            self.0.len(&map)
        })
    }

    pub fn user_approvals(&self, who: Principal) -> Vec<(Principal, Nat)> {
        let mut buf: Vec<u8> = vec![];
        let owner = who.as_slice();
        buf.push(owner.len() as u8);
        buf.extend(owner);
        let mut result: Vec<(Principal, Nat)> = vec![];
        STABLE_MAP.with(|s| {
            let map = s.borrow();
            for (k, v) in self.0.range(Some(buf), None, &map) {
                result.push((self.decode_key(k).1, self.0.val_decode(&v)));
            }
            result
        })
    }
}

// TODO: a wrapper over `ic_helpers::TxError`, this is a most likely
// place to make tests fail in amm.
#[derive(CandidType, Debug, PartialEq, Deserialize)]
pub enum TxError {
    InsufficientBalance,
    InsufficientAllowance,
    NoAllowance,
    Unauthorized,
    AmountTooSmall,
    FeeExceededLimit,
    ApproveSucceededButNotifyFailed { tx_error: Box<TxError> },
    NotificationFailed { transaction_id: Nat },
    AlreadyActioned,
    NotificationDoesNotExist,
    TransactionDoesNotExist,
    BadFee { expected_fee: u64 },
    InsufficientFunds { balance: u64 },
    TxTooOld { allowed_window_nanos: u64 },
    TxCreatedInFuture,
    TxDuplicate { duplicate_of: u64 },
    SelfTransfer,
}

pub type TxReceipt = Result<Nat, TxError>;

// Notification receiver not set if None
#[derive(Debug, CandidType, Deserialize)]
pub struct PendingNotifications(pub StableMap);

impl Default for PendingNotifications {
    fn default() -> Self {
        Self(StableMap::new(
            *PEND_NOTICE_MAGIC,
            PEND_NOTICE_LAYOUT_VERSION,
        ))
    }
}

impl PendingNotifications {
    pub fn insert(&self, index: Nat, amount: Option<Principal>) {
        STABLE_MAP.with(|s| {
            let mut map = s.borrow_mut();
            self.0
                .insert::<Nat, Option<Principal>>(&index, &amount, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("failed to serialize value: {}", e))
                });
        });
    }

    pub fn remove(&self, index: &Nat) -> Option<Option<Principal>> {
        STABLE_MAP.with(|s| {
            let mut map = s.borrow_mut();
            self.0.remove::<Nat, Option<Principal>>(index, &mut map)
        })
    }

    pub fn get(&self, index: &Nat) -> Option<Option<Principal>> {
        STABLE_MAP.with(|s| {
            let map = s.borrow();
            self.0.get::<Nat, Option<Principal>>(index, &map)
        })
    }

    pub fn contains_key(&self, index: &Nat) -> bool {
        STABLE_MAP.with(|s| {
            let map = s.borrow();
            self.0.contains_key::<Nat>(index, &map)
        })
    }
}

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

#[derive(Debug, CandidType, Deserialize)]
pub struct AuctionInfoStable {
    pub auction_id: StableMap,
    pub auction_time: StableMap,
    pub tokens_distributed: StableMap,
    pub cycles_collected: StableMap,
    pub fee_ratio: StableMap,
    pub first_transaction_id: StableMap,
    pub last_transaction_id: StableMap,
}

impl Default for AuctionInfoStable {
    fn default() -> Self {
        Self {
            auction_id: StableMap::new(*AUCTION_ID_MAGIC, AUCTION_ID_LAYOUT_VERSION),
            auction_time: StableMap::new(*AUCTION_TIME_MAGIC, AUCTION_TIME_LAYOUT_VERSION),
            tokens_distributed: StableMap::new(*TOKENS_DIST_MAGIC, TOKENS_DIST_LAYOUT_VERSION),
            cycles_collected: StableMap::new(*CYCLES_COLLECT_MAGIC, CYCLES_COLLECT_LAYOUT_VERSION),
            fee_ratio: StableMap::new(*FEE_RATIO_MAGIC, FEE_RATIO_LAYOUT_VERSION),
            first_transaction_id: StableMap::new(*FIRST_TX_MAGIC, FIRST_TX_LAYOUT_VERSION),
            last_transaction_id: StableMap::new(*LAST_TX_MAGIC, LAST_TX_LAYOUT_VERSION),
        }
    }
}

impl AuctionInfoStable {
    pub fn get(&self, id: usize) -> Option<AuctionInfo> {
        STABLE_MAP.with(|s| {
            let map = s.borrow();
            let id = id as u64;
            let auction_id = self.auction_id.get::<u64, usize>(&id, &map);
            let auction_time = self.auction_time.get::<u64, Timestamp>(&id, &map);
            let tokens_distributed = self.tokens_distributed.get::<u64, Nat>(&id, &map);
            let cycles_collected = self.cycles_collected.get::<u64, u64>(&id, &map);
            let fee_ratio = self.fee_ratio.get::<u64, f64>(&id, &map);
            let first_transaction_id = self.first_transaction_id.get::<u64, Nat>(&id, &map);
            let last_transaction_id = self.last_transaction_id.get::<u64, Nat>(&id, &map);

            auction_id.map(|auction_id| AuctionInfo {
                auction_id,
                auction_time: auction_time.unwrap(),
                tokens_distributed: tokens_distributed.unwrap(),
                cycles_collected: cycles_collected.unwrap(),
                fee_ratio: fee_ratio.unwrap(),
                first_transaction_id: first_transaction_id.unwrap(),
                last_transaction_id: last_transaction_id.unwrap(),
            })
        })
    }

    pub fn len(&self) -> usize {
        STABLE_MAP.with(|s| {
            let map = s.borrow();
            self.auction_id.len(&map)
        })
    }

    pub fn push(&self, item: AuctionInfo) {
        let id = self.len() as u64;

        STABLE_MAP.with(|s| {
            let mut map = s.borrow_mut();
            self.auction_id
                .insert::<u64, usize>(&id, &item.auction_id, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("AuctionInfoStable insert error: {}", e))
                });
            self.auction_time
                .insert::<u64, Timestamp>(&id, &item.auction_time, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("AuctionInfoStable insert error: {}", e))
                });
            self.tokens_distributed
                .insert::<u64, Nat>(&id, &item.tokens_distributed, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("AuctionInfoStable insert error: {}", e))
                });
            self.cycles_collected
                .insert::<u64, u64>(&id, &item.cycles_collected, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("AuctionInfoStable insert error: {}", e))
                });
            self.fee_ratio
                .insert::<u64, f64>(&id, &item.fee_ratio, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("AuctionInfoStable insert error: {}", e))
                });
            self.first_transaction_id
                .insert::<u64, Nat>(&id, &item.first_transaction_id, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("AuctionInfoStable insert error: {}", e))
                });
            self.last_transaction_id
                .insert::<u64, Nat>(&id, &item.last_transaction_id, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("AuctionInfoStable insert error: {}", e))
                });
        });
    }
}

/// `PaginatedResult` is returned by paginated queries i.e `getTransactions`.
#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct PaginatedResult {
    /// The result is the transactions which is the `count` transactions starting from `next` if it exists.
    pub result: Vec<TxRecord>,

    /// This is  the next `id` of the transaction. The `next` is used as offset for the next query if it exits.
    pub next: Option<u128>,
}

// I want set the K,V type in struct by using PhantomData<T>, but it can't derive CandidType
// I also want to set the map: StableBTreeMap<RestrictedMemory<StableStorage>> as a field of the struct it
// can't skip CandidType and Deserialize.
#[derive(CandidType, Default, Debug, Clone, Deserialize)]
pub struct StableMap {
    pub magic: [u8; 3],
    version: u8,
}

impl StableMap {
    pub fn new(magic: [u8; 3], version: u8) -> Self {
        Self { magic, version }
    }

    pub fn key_encode<K: CandidType + serde::de::DeserializeOwned>(&self, key: &K) -> Vec<u8> {
        let buf = candid::encode_one(key).unwrap_or_else(|e| {
            ic_canister::ic_kit::ic::trap(&format!("failed to serialize key: {}", e))
        });
        let mut key = self.magic.to_vec();
        key.extend(&buf);
        key
    }

    pub fn val_encode<V: CandidType + serde::de::DeserializeOwned>(&self, val: &V) -> Vec<u8> {
        candid::encode_one(val).unwrap_or_else(|e| {
            ic_canister::ic_kit::ic::trap(&format!("failed to serialize value: {}", e))
        })
    }

    pub fn key_decode<K: CandidType + serde::de::DeserializeOwned>(&self, buf: &[u8]) -> K {
        let mut buf = buf.to_owned();
        let prefix: Vec<u8> = buf.drain(0..3).collect();
        assert_eq!(prefix, &self.magic);
        candid::decode_one(&buf).unwrap_or_else(|e| {
            ic_canister::ic_kit::ic::trap(&format!("failed to deserialize a key: {}", e))
        })
    }

    pub fn val_decode<V: CandidType + serde::de::DeserializeOwned>(&self, buf: &[u8]) -> V {
        candid::decode_one(buf).unwrap_or_else(|e| {
            ic_canister::ic_kit::ic::trap(&format!("failed to deserialize a value: {}", e))
        })
    }

    pub fn get<
        K: CandidType + serde::de::DeserializeOwned,
        V: CandidType + serde::de::DeserializeOwned,
    >(
        &self,
        key: &K,
        map: &StableBTreeMap<RestrictedMemory<StableStorage>>,
    ) -> Option<V> {
        let key = self.key_encode(key);
        map.get(&key).map(|v| self.val_decode::<V>(&v))
    }

    pub fn insert<
        K: CandidType + serde::de::DeserializeOwned,
        V: CandidType + serde::de::DeserializeOwned,
    >(
        &self,
        key: &K,
        value: &V,
        map: &mut StableBTreeMap<RestrictedMemory<StableStorage>>,
    ) -> Result<Option<V>, InsertError> {
        let key = self.key_encode(key);
        let value = self.val_encode(value);
        let result = map.insert(key, value)?;
        match result {
            Some(v) => Ok(Some(self.val_decode(&v))),
            None => Ok(None),
        }
    }

    pub fn contains_key<K: CandidType + serde::de::DeserializeOwned>(
        &self,
        key: &K,
        map: &StableBTreeMap<RestrictedMemory<StableStorage>>,
    ) -> bool {
        let key = self.key_encode(key);
        map.contains_key(&key)
    }

    pub fn remove<
        K: CandidType + serde::de::DeserializeOwned,
        V: CandidType + serde::de::DeserializeOwned,
    >(
        &self,
        key: &K,
        map: &mut StableBTreeMap<RestrictedMemory<StableStorage>>,
    ) -> Option<V> {
        let key = self.key_encode(key);
        map.remove(&key).map(|v| self.val_decode(&v))
    }

    pub fn clear(&self, map: &mut StableBTreeMap<RestrictedMemory<StableStorage>>) {
        let mut keys = vec![];
        for (k, _) in map.range(self.magic.to_vec(), None) {
            keys.push(k);
        }
        for i in keys.iter() {
            map.remove(i);
        }
    }

    pub fn total_len(map: &StableBTreeMap<RestrictedMemory<StableStorage>>) -> u64 {
        map.len()
    }

    pub fn len(&self, map: &StableBTreeMap<RestrictedMemory<StableStorage>>) -> usize {
        map.range(self.magic.to_vec(), None).count()
    }

    pub fn is_empty(&self, map: &StableBTreeMap<RestrictedMemory<StableStorage>>) -> bool {
        Self::total_len(map) == 0 || self.len(map) == 0
    }

    pub fn total_iter(
        map: &StableBTreeMap<RestrictedMemory<StableStorage>>,
    ) -> Iter<RestrictedMemory<StableStorage>> {
        map.iter()
    }

    pub fn range<'a>(
        &'a self,
        prefix: Option<Vec<u8>>,
        offset: Option<Vec<u8>>,
        map: &'a StableBTreeMap<RestrictedMemory<StableStorage>>,
    ) -> Iter<RestrictedMemory<StableStorage>> {
        let mut magic = self.magic.to_vec();
        magic.extend(&prefix.unwrap_or_default());
        map.range(magic, offset)
    }
}
