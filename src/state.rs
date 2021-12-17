use crate::ledger::Ledger;
use crate::types::{Allowances, Balances, PendingNotifications, StatsData};
use candid::{CandidType, Deserialize};

#[derive(Default, CandidType, Deserialize)]
pub struct State {
    stats: StatsData,
    balances: Balances,
    allowances: Allowances,
    ledger: Ledger,
    notifications: PendingNotifications,
}

impl State {
    pub fn get() -> &'static mut Self {
        ic_kit::ic::get_mut()
    }

    pub fn stats(&self) -> &StatsData {
        &self.stats
    }

    pub fn stats_mut(&mut self) -> &mut StatsData {
        &mut self.stats
    }

    pub fn balances(&self) -> &Balances {
        &self.balances
    }

    pub fn balances_mut(&mut self) -> &mut Balances {
        &mut self.balances
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

    pub fn store(&self) {
        ic_cdk::storage::stable_save((&self,)).unwrap();
    }

    pub fn load() {
        let (state,) = ic_cdk::storage::stable_restore().unwrap();
        *State::get() = state;
    }
}
