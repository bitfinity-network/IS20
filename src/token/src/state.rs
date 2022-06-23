use crate::ledger::Ledger;
use crate::types::{Allowances, AuctionInfo, Map, StatsData, Timestamp};
use candid::{CandidType, Deserialize, Nat, Principal};
use common::types::Metadata;
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;
use stable_structures::{
    stable_storage::StableStorage, types::Address, RestrictedMemory, StableBTreeMap,
};
use std::collections::HashMap;

const BID_HEAD_MAGIC: &[u8; 3] = b"BDH";
const BID_HEAD_LAYOUT_VERSION: u8 = 1;

const BID_DATA_MAGIC: &[u8; 3] = b"BDD";
const BID_DATA_LAYOUT_VERSION: u8 = 1;

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
        match self.allowances.get(&owner) {
            Some(inner) => match inner.get(&spender) {
                Some(value) => value.clone(),
                None => Nat::from(0),
            },
            None => Nat::from(0),
        }
    }

    pub fn allowance_size(&self) -> usize {
        self.allowances
            .iter()
            .map(|(_, v)| v.len())
            .reduce(|accum, v| accum + v)
            .unwrap_or(0)
    }

    pub fn user_approvals(&self, who: Principal) -> Vec<(Principal, Nat)> {
        match self.allowances.get(&who) {
            Some(allow) => Vec::from_iter(allow.clone().into_iter()),
            None => Vec::new(),
        }
    }
}
impl Versioned for CanisterState {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}

#[derive(Debug, Default, CandidType, Deserialize)]
pub struct Balances(pub HashMap<Principal, Nat>);

impl Balances {
    pub fn balance_of(&self, who: &Principal) -> Nat {
        self.0.get(who).cloned().unwrap_or_else(|| Nat::from(0))
    }

    pub fn get_holders(&self, start: usize, limit: usize) -> Vec<(Principal, Nat)> {
        let mut balance = self
            .0
            .iter()
            .map(|(&k, v)| (k, v.clone()))
            .collect::<Vec<_>>();

        // Sort balance and principals by the balance
        balance.sort_by(|a, b| b.1.cmp(&a.1));

        let end = (start + limit).min(balance.len());
        balance[start..end].to_vec()
    }
}

#[derive(CandidType, Default, Debug, Clone, Deserialize)]
pub struct BiddingState {
    pub fee_ratio: f64,
    pub last_auction: Timestamp,
    pub auction_period: Timestamp,
    pub cycles_since_auction: u64,
    pub bids: HashMap<Principal, u64>,
}

impl BiddingState {
    pub fn is_auction_due(&self) -> bool {
        let curr_time = ic_canister::ic_kit::ic::time();
        let next_auction = self.last_auction + self.auction_period;
        curr_time >= next_auction
    }

    pub fn save_header(&self, memory: &RestrictedMemory<StableStorage>) {
        memory.write_struct::<BiddingStateHeader>(&BiddingStateHeader::from(self), Address(0));
    }

    pub fn load_header(&mut self, memory: &RestrictedMemory<StableStorage>) {
        let header: BiddingStateHeader = memory.read_struct(Address(0));
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
pub struct AuctionHistory(pub Vec<AuctionInfo>);
