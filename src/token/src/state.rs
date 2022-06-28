use crate::ledger::Ledger;
use crate::types::{Allowances, AuctionInfoStable, StableMap, StatsData, Timestamp};
use candid::{CandidType, Deserialize, Nat, Principal};
use common::types::Metadata;
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;
use stable_structures::{stable_storage::StableStorage, RestrictedMemory, StableBTreeMap};
use std::cell::RefCell;

const BID_HEAD_MAGIC: &[u8; 3] = b"BHD";
const BID_HEAD_LAYOUT_VERSION: u8 = 1;

const BID_DATA_MAGIC: &[u8; 3] = b"BDA";
const BID_DATA_LAYOUT_VERSION: u8 = 1;

const BALANCES_MAGIC: &[u8; 3] = b"BAS";
const BALANCES_LAYOUT_VERSION: u8 = 1;

thread_local! {
    pub static BIDDING_STATE_HEADER: RefCell<RestrictedMemory<StableStorage>> = RefCell::new(RestrictedMemory::new(StableStorage::default(), 0..1));
    pub static LEDGER_HEADER: RefCell<RestrictedMemory<StableStorage>> = RefCell::new(RestrictedMemory::new(StableStorage::default(), 1..2));
    pub static STATS_DATA_HEADER: RefCell<RestrictedMemory<StableStorage>> = RefCell::new(RestrictedMemory::new(StableStorage::default(), 2..35)); // logo <= 2 MiB, others < 64 Kib total < 33 pages
    pub static STABLE_MAP: RefCell<StableBTreeMap<RestrictedMemory<StableStorage>>> = RefCell::new(StableBTreeMap::new(RestrictedMemory::new(StableStorage::default(), 35..131072), 64, 64));
}

#[derive(Debug, Default, CandidType, Deserialize, IcStorage)]
pub struct CanisterState {
    pub(crate) bidding_state: BiddingState,
    pub(crate) balances: Balances,
    pub(crate) auction_history: AuctionHistory,
    pub(crate) stats: StatsData,
    pub(crate) allowances: Allowances,
    pub(crate) ledger: Ledger,
}

impl CanisterState {
    pub fn get_metadata(&self) -> Metadata {
        Metadata {
            logo: self.stats.logo.clone(),
            name: self.stats.name.clone(),
            symbol: self.stats.symbol.clone(),
            decimals: self.stats.decimals,
            totalSupply: self.stats.total_supply.clone(),
            owner: self.stats.owner,
            fee: self.stats.fee.clone(),
            feeTo: self.stats.fee_to,
            isTestToken: Some(self.stats.is_test_token),
        }
    }

    pub fn allowance(&self, owner: Principal, spender: Principal) -> Nat {
        match self.allowances.get(&owner, &spender) {
            Some(v) => v,
            None => Nat::from(0),
        }
    }

    pub fn allowance_size(&self) -> usize {
        self.allowances.len()
    }

    pub fn user_approvals(&self, who: Principal) -> Vec<(Principal, Nat)> {
        self.allowances.user_approvals(who)
    }
}
impl Versioned for CanisterState {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}

#[derive(Debug, CandidType, Deserialize)]
pub struct Balances(pub StableMap);

impl Default for Balances {
    fn default() -> Self {
        Self(StableMap::new(*BALANCES_MAGIC, BALANCES_LAYOUT_VERSION))
    }
}

impl Balances {
    pub fn balance_of(&self, who: &Principal) -> Nat {
        STABLE_MAP.with(|s| {
            let map = s.borrow();
            self.0
                .get::<Principal, Nat>(who, &map)
                .unwrap_or_else(|| Nat::from(0))
        })
    }

    pub fn get_holders(&self, start: usize, limit: usize) -> Vec<(Principal, Nat)> {
        let mut balance = STABLE_MAP.with(|s| {
            let map = s.borrow();
            self.0
                .range(None, None, &map)
                .map(|(k, v)| {
                    (
                        self.0.key_decode::<Principal>(&k),
                        self.0.val_decode::<Nat>(&v),
                    )
                })
                .collect::<Vec<_>>()
        });

        // Sort balance and principals by the balance
        balance.sort_by(|a, b| b.1.cmp(&a.1));

        let end = (start + limit).min(balance.len());
        balance[start..end].to_vec()
    }

    pub fn insert(&self, user: Principal, amount: Nat) {
        STABLE_MAP.with(|s| {
            let mut map = s.borrow_mut();
            self.0
                .insert::<Principal, Nat>(&user, &amount, &mut map)
                .unwrap_or_else(|e| {
                    ic_canister::ic_kit::ic::trap(&format!("failed to serialize value: {}", e))
                });
        });
    }

    pub fn len(&self) -> usize {
        STABLE_MAP.with(|s| {
            let map = s.borrow();
            self.0.len(&map)
        })
    }

    pub fn remove(&self, user: &Principal) {
        STABLE_MAP.with(|s| {
            let mut map = s.borrow_mut();
            self.0.remove::<Principal, Nat>(user, &mut map);
        });
    }

    pub fn get(&self, user: &Principal) -> Option<Nat> {
        STABLE_MAP.with(|s| {
            let map = s.borrow();
            self.0.get::<Principal, Nat>(user, &map)
        })
    }

    pub fn contains_key(&self, user: &Principal) -> bool {
        STABLE_MAP.with(|s| {
            let map = s.borrow();
            self.0.contains_key::<Principal>(user, &map)
        })
    }
}

#[derive(CandidType, Debug, Clone, Deserialize)]
pub struct BiddingState {
    pub fee_ratio: f64,
    pub last_auction: Timestamp,
    pub auction_period: Timestamp,
    pub cycles_since_auction: u64,
    pub bids: StableMap,
}

impl Default for BiddingState {
    fn default() -> Self {
        Self {
            fee_ratio: f64::default(),
            last_auction: Timestamp::default(),
            auction_period: Timestamp::default(),
            cycles_since_auction: u64::default(),
            bids: StableMap::new(*BID_DATA_MAGIC, BID_DATA_LAYOUT_VERSION),
        }
    }
}

impl BiddingState {
    pub fn is_auction_due(&self) -> bool {
        let curr_time = ic_canister::ic_kit::ic::time();
        let next_auction = self.last_auction + self.auction_period;
        curr_time >= next_auction
    }

    pub fn save_header(&self, memory: &RestrictedMemory<StableStorage>) {
        memory.write_struct::<BiddingStateHeader>(&BiddingStateHeader::from(self), 0);
    }

    pub fn load_header(&mut self, memory: &RestrictedMemory<StableStorage>) {
        let header: BiddingStateHeader = memory.read_struct(0);
        assert_eq!(&header.magic, BID_HEAD_MAGIC, "Bad magic.");
        assert_eq!(
            header.version, BID_HEAD_LAYOUT_VERSION,
            "Unsupported version."
        );
        self.fee_ratio = header.fee_ratio;
        self.last_auction = header.last_auction;
        self.auction_period = header.auction_period;
        self.cycles_since_auction = header.cycles_since_auction;
    }
}

struct BiddingStateHeader {
    magic: [u8; 3],
    version: u8,
    fee_ratio: f64,
    last_auction: Timestamp,
    auction_period: Timestamp,
    cycles_since_auction: u64,
}

impl From<&BiddingState> for BiddingStateHeader {
    fn from(value: &BiddingState) -> Self {
        Self {
            magic: *BID_HEAD_MAGIC,
            version: BID_HEAD_LAYOUT_VERSION,
            fee_ratio: value.fee_ratio,
            last_auction: value.last_auction,
            auction_period: value.auction_period,
            cycles_since_auction: value.cycles_since_auction,
        }
    }
}

#[derive(Debug, Default, CandidType, Deserialize)]
pub struct AuctionHistory(pub AuctionInfoStable);
