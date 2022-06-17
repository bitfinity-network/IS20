use crate::ledger::Ledger;
use crate::types::{Allowances, AuctionInfo, Cycles, Metadata, StatsData, Timestamp};
use candid::{CandidType, Deserialize, Principal};
use ic_helpers::tokens::Tokens128;
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;
use std::collections::HashMap;

#[derive(Default, CandidType, Deserialize, IcStorage)]
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
            totalSupply: self.stats.total_supply,
            owner: self.stats.owner,
            fee: self.stats.fee,
            feeTo: self.stats.fee_to,
            isTestToken: Some(self.stats.is_test_token),
        }
    }

    pub fn allowance(&self, owner: Principal, spender: Principal) -> Tokens128 {
        match self.allowances.get(&owner) {
            Some(inner) => match inner.get(&spender) {
                Some(value) => value.clone(),
                None => Tokens128::from(0u128),
            },
            None => Tokens128::from(0u128),
        }
    }

    pub fn allowance_size(&self) -> usize {
        self.allowances
            .iter()
            .map(|(_, v)| v.len())
            .reduce(|accum, v| accum + v)
            .unwrap_or(0)
    }

    pub fn user_approvals(&self, who: Principal) -> Vec<(Principal, Tokens128)> {
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

#[derive(Default, CandidType, Deserialize)]
pub struct Balances(pub HashMap<Principal, Tokens128>);

impl Balances {
    pub fn balance_of(&self, who: &Principal) -> Tokens128 {
        self.0
            .get(who)
            .cloned()
            .unwrap_or_else(|| Tokens128::from(0u128))
    }

    pub fn get_holders(&self, start: usize, limit: usize) -> Vec<(Principal, Tokens128)> {
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
    pub cycles_since_auction: Cycles,
    pub bids: HashMap<Principal, Cycles>,
}

impl BiddingState {
    pub fn is_auction_due(&self) -> bool {
        let curr_time = ic_canister::ic_kit::ic::time();
        let next_auction = self.last_auction + self.auction_period;
        curr_time >= next_auction
    }
}

#[derive(Default, CandidType, Deserialize)]
pub struct AuctionHistory(pub Vec<AuctionInfo>);
