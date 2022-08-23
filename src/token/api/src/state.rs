use std::collections::HashMap;

use candid::Nat;
use candid::{CandidType, Deserialize, Principal};
use ic_helpers::ledger::AccountIdentifier;
use ic_helpers::ledger::Subaccount as SubaccountIdentifier;
use ic_helpers::tokens::Tokens128;
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;

use crate::account::{Account, Subaccount, DEFAULT_SUBACCOUNT};
use crate::error::TxError;
use crate::ledger::Ledger;
use crate::types::{AuctionInfo, Claims, Cycles, Metadata, StatsData, Timestamp, Value};

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
    pub fn icrc1_metadata(&self) -> Vec<(String, Value)> {
        vec![
            (
                "icrc1:symbol".to_string(),
                Value::Text(self.stats.symbol.clone()),
            ),
            (
                "icrc1:name".to_string(),
                Value::Text(self.stats.name.clone()),
            ),
            (
                "icrc1:decimals".to_string(),
                Value::Nat(Nat::from(self.stats.decimals)),
            ),
            (
                "icrc1:fee".to_string(),
                Value::Nat(self.stats.fee.amount.into()),
            ),
        ]
    }

    pub fn get_metadata(&self) -> Metadata {
        Metadata {
            logo: self.stats.logo.clone(),
            name: self.stats.name.clone(),
            symbol: self.stats.symbol.clone(),
            decimals: self.stats.decimals,
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

    pub fn get_claim(&self, subaccount: Option<Subaccount>) -> Result<Tokens128, TxError> {
        let acc = AccountIdentifier::new(
            ic_canister::ic_kit::ic::caller().into(),
            Some(SubaccountIdentifier(
                subaccount.unwrap_or(DEFAULT_SUBACCOUNT),
            )),
        );
        self.claims
            .get(&acc)
            .ok_or(TxError::AccountNotFound)
            .copied()
    }
}

impl Versioned for CanisterState {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}

/// We are saving the `Balances` in this format, as we want to support `Principal` supporting `Subaccount`.
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
            .insert(subaccount.unwrap_or(DEFAULT_SUBACCOUNT), token);
    }

    pub fn get_mut(&mut self, account: Account) -> Option<&mut Tokens128> {
        self.0.get_mut(&account.owner).and_then(|subaccounts| {
            subaccounts.get_mut(&account.subaccount.unwrap_or(DEFAULT_SUBACCOUNT))
        })
    }

    pub fn get_mut_or_insert_default(&mut self, account: Account) -> &mut Tokens128 {
        self.0
            .entry(account.owner)
            .or_default()
            .entry(account.subaccount.unwrap_or(DEFAULT_SUBACCOUNT))
            .or_default()
    }

    pub fn balance_of(&self, account: Account) -> Tokens128 {
        self.0
            .get(&account.owner)
            .and_then(|subaccounts| {
                subaccounts.get(&account.subaccount.unwrap_or(DEFAULT_SUBACCOUNT))
            })
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
        if let Some(subaccounts) = self.0.get_mut(&account.owner) {
            subaccounts.remove(&account.subaccount.unwrap_or(DEFAULT_SUBACCOUNT));
        }

        if self
            .0
            .get(&account.owner)
            .map(|subaccounts| subaccounts.is_empty())
            .unwrap_or(true)
        {
            self.0.remove(&account.owner);
        }
    }

    pub fn set_balance(&mut self, account: Account, amount: Tokens128) {
        self.0
            .entry(account.owner)
            .or_default()
            .insert(account.subaccount.unwrap_or(DEFAULT_SUBACCOUNT), amount);
    }

    pub fn total_supply(&self) -> Tokens128 {
        self.0
            .iter()
            .flat_map(|(_, subaccounts)| subaccounts.values())
            .fold(Tokens128::ZERO, |a, b| (a + b).unwrap_or(Tokens128::ZERO))
    }

    pub(crate) fn apply_change(&mut self, change: &Balances) {
        for (principal, subaccounts) in &change.0 {
            for (subaccount, amount) in subaccounts {
                self.set_balance(Account::new(*principal, Some(*subaccount)), *amount);
            }
        }
    }
}

#[derive(CandidType, Default, Debug, Clone, Deserialize)]
pub struct BiddingState {
    pub fee_ratio: FeeRatio,
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

#[derive(CandidType, Default, Debug, Copy, Clone, Deserialize, PartialEq)]
pub struct FeeRatio(f64);

impl FeeRatio {
    pub fn new(value: f64) -> Self {
        let adj_value = if value < 0.0 {
            0.0
        } else if value > 1.0 {
            1.0
        } else {
            value
        };

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
