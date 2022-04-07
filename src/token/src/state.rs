use crate::ledger::Ledger;
use crate::types::{Allowances, AuctionInfo, PendingNotifications, StatsData, Timestamp};
use candid::{CandidType, Deserialize, Nat, Principal};
use common::types::Metadata;
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;
use std::collections::HashMap;

#[derive(Default, CandidType, Deserialize, IcStorage)]
pub struct State {
    pub(crate) stats: StatsData,
    allowances: Allowances,
    pub(crate) ledger: Ledger,
    auction_history: AuctionHistory,
    pub notifications: PendingNotifications,
}

impl Versioned for State {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}

#[derive(Default, IcStorage, CandidType, Deserialize)]
pub struct Balances(pub HashMap<Principal, Nat>);

impl Balances {
    pub fn balance_of(&self, who: &Principal) -> Nat {
        self.0.get(who).cloned().unwrap_or_else(|| Nat::from(0))
    }

    pub fn get_holders(&self, start: usize, limit: usize) -> Vec<(Principal, Nat)> {
        let mut balance = Vec::new();

        for (k, v) in &self.0 {
            balance.push((*k, v.clone()));
        }
        balance.sort_by(|a, b| b.1.cmp(&a.1));
        let limit: usize = if start + limit > balance.len() {
            balance.len() - start
        } else {
            limit
        };

        balance[start..start + limit].to_vec()
    }
}

#[derive(CandidType, Default, Debug, Clone, Deserialize, IcStorage)]
pub struct BiddingState {
    pub fee_ratio: f64,
    pub last_auction: Timestamp,
    pub auction_period: Timestamp,
    pub cycles_since_auction: u64,
    pub bids: HashMap<Principal, u64>,
}

impl BiddingState {
    pub fn is_auction_due(&self) -> bool {
        let curr_time = ic_kit::ic::time();
        let next_auction = self.last_auction + self.auction_period;
        curr_time >= next_auction
    }
}

#[derive(Default, IcStorage, CandidType, Deserialize)]
pub struct AuctionHistory(pub Vec<AuctionInfo>);

impl State {
    pub fn stats(&self) -> &StatsData {
        &self.stats
    }

    pub fn stats_mut(&mut self) -> &mut StatsData {
        &mut self.stats
    }

    pub fn allowances(&self) -> &Allowances {
        &self.allowances
    }

    pub fn allowances_mut(&mut self) -> &mut Allowances {
        &mut self.allowances
    }

    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    pub fn ledger_mut(&mut self) -> &mut Ledger {
        &mut self.ledger
    }

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
        match self.allowances().get(&owner) {
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
