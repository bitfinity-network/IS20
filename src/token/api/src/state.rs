use std::collections::HashMap;

use candid::{CandidType, Deserialize, Principal};
use ic_helpers::tokens::Tokens128;
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;

use crate::ledger::Ledger;
use crate::types::{
    Account, Allowances, AuctionInfo, Cycles, Metadata, StatsData, Subaccount, Timestamp,
};

#[derive(Debug, Default, CandidType, Deserialize, IcStorage)]
pub struct CanisterState {
    pub bidding_state: BiddingState,
    pub balances: Balances,
    pub auction_history: AuctionHistory,
    pub stats: StatsData,
    pub allowances: Allowances,
    pub ledger: Ledger,
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
}

impl Versioned for CanisterState {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}

#[derive(Debug, Default, CandidType, Deserialize)]
pub struct Balances(pub HashMap<Principal, HashMap<Subaccount, Tokens128>>);

impl Balances {
    pub fn insert(
        &mut self,
        principal: Principal,
        subaccount: Option<Subaccount>,
        token: Tokens128,
    ) {
        self.0
            .entry(principal)
            .or_default()
            .insert(subaccount.unwrap_or_default(), token);
    }

    pub fn balance_of(&self, who: &Principal, who_subaccount: Option<Subaccount>) -> Tokens128 {
        let who_subaccount = who_subaccount.unwrap_or_default();
        *self
            .0
            .get(&who)
            .and_then(|subaccount| subaccount.get(&who_subaccount))
            .unwrap_or(&Tokens128::default())
    }

    pub fn get_holders(&self) -> Vec<Principal> {
        self.0.keys().cloned().collect()
    }

    pub fn get_balances(&self, who: &Principal) -> HashMap<Subaccount, Tokens128> {
        self.0.get(who).cloned().unwrap_or_default()
    }

    pub fn remove(&mut self, account: Account) {
        if let Some(subaccount) = account.subaccount {
            self.0
                .get_mut(&account.account)
                .map(|subaccounts| subaccounts.remove(&subaccount));
        } else {
            self.0.remove(&account.account);
        }
    }

    pub fn set_balance(&mut self, account: Account, token: Tokens128) {
        if let Some(subaccount) = account.subaccount {
            self.0
                .get_mut(&account.account)
                .map(|subaccounts| subaccounts.insert(subaccount, token));
        } else {
            self.0
                .get_mut(&account.account)
                .map(|subaccounts| subaccounts.insert(Subaccount::default(), token));
        }
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

#[derive(Debug, Default, CandidType, Deserialize)]
pub struct AuctionHistory(pub Vec<AuctionInfo>);
