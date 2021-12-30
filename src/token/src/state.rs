use crate::ledger::Ledger;
use crate::types::{Allowances, AuctionInfo, PendingNotifications, StatsData, Timestamp};
use candid::{CandidType, Deserialize, Nat, Principal};
use ic_storage::IcStorage;
use std::collections::HashMap;

#[derive(Default, CandidType, Deserialize, IcStorage)]
pub struct State {
    stats: StatsData,
    allowances: Allowances,
    ledger: Ledger,
    auction_history: AuctionHistory,
    notifications: PendingNotifications,
}

#[derive(Default, IcStorage, CandidType, Deserialize)]
pub struct Balances(pub HashMap<Principal, Nat>);

impl Balances {
    pub fn balance_of(&self, who: &Principal) -> Nat {
        self.0.get(who).cloned().unwrap_or_else(|| Nat::from(0))
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

    pub fn notifications_mut(&mut self) -> &mut PendingNotifications {
        &mut self.notifications
    }
}
