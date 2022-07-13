use std::collections::HashMap;

use candid::{CandidType, Deserialize, Principal};
use ic_helpers::ledger::{AccountIdentifier, Subaccount};
use ic_helpers::tokens::Tokens128;
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;

use crate::account::{Account, SUB_ACCOUNT_ZERO};
use crate::ledger::Ledger;
use crate::types::{AuctionInfo, Claims, Cycles, Metadata, StatsData, Timestamp};

#[derive(Debug, Default, CandidType, Deserialize, IcStorage)]
pub struct CanisterState {
    pub bidding_state: BiddingState,
    pub balances: Balances,
    pub auction_history: AuctionHistory,
    pub stats: StatsData,
    pub ledger: Ledger,
    pub claims: Claims,
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

    pub fn claim_amount(&self, account: AccountIdentifier) -> Tokens128 {
        self.claims
            .get(&account)
            .copied()
            .unwrap_or(Tokens128::ZERO)
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
            .insert(subaccount.unwrap_or(SUB_ACCOUNT_ZERO), token);
    }

    pub fn get_mut(&mut self, account: Account) -> Option<&mut Tokens128> {
        self.0
            .get_mut(&account.account)
            .and_then(|subaccounts| subaccounts.get_mut(&account.subaccount))
    }

    pub fn get_mut_or_insert_default(&mut self, account: Account) -> &mut Tokens128 {
        self.0
            .entry(account.account)
            .or_default()
            .entry(account.subaccount)
            .or_default()
    }

    pub fn balance_of(&self, account: Account) -> Tokens128 {
        self.0
            .get(&account.account)
            .and_then(|subaccounts| subaccounts.get(&account.subaccount))
            .copied()
            .unwrap_or_default()
    }

    pub fn get_holders(&self, start: usize, limit: usize) -> Vec<(Account, Tokens128)> {
        let mut holders = self
            .0
            .iter()
            .flat_map(|(principal, subaccounts)| {
                subaccounts
                    .iter()
                    .map(|(subaccount, token)| {
                        (Account::new(*principal, Some(*subaccount)), *token)
                    })
                    .collect::<Vec<_>>()
            })
            .skip(start)
            .take(limit)
            .collect::<Vec<_>>();
        holders.sort_by(|a, b| b.1.cmp(&a.1));
        holders
    }

    pub fn remove(&mut self, account: Account) {
        if let Some(subaccounts) = self.0.get_mut(&account.account) {
            subaccounts.remove(&account.subaccount);
        }

        if self
            .0
            .get(&account.account)
            .map(|subaccounts| subaccounts.is_empty())
            .unwrap_or(true)
        {
            self.0.remove(&account.account);
        }
    }

    pub fn set_balance(&mut self, account: Account, token: Tokens128) {
        self.0
            .get_mut(&account.account)
            .map(|subaccounts| subaccounts.insert(account.subaccount, token));
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
