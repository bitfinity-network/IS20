use std::collections::HashMap;

use candid::Nat;
use candid::{CandidType, Deserialize, Principal};
use ic_auction::state::AuctionState;
use ic_helpers::ledger::AccountIdentifier;
use ic_helpers::ledger::Subaccount as SubaccountIdentifier;
use ic_helpers::tokens::Tokens128;
use ic_storage::stable::Versioned;
use ic_storage::IcStorage;

use crate::account::{Account, Subaccount, DEFAULT_SUBACCOUNT};
use crate::error::TxError;
use crate::ledger::Ledger;
use crate::types::{Claims, Metadata, StatsData, Value};

#[derive(Debug, Default, CandidType, Deserialize, IcStorage)]
pub struct CanisterState {
    pub balances: Balances,
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
            fee_to: self.stats.fee_to,
            is_test_token: Some(self.stats.is_test_token),
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

    pub fn set_balance(&mut self, account: Account, token: Tokens128) {
        self.0.get_mut(&account.owner).map(|subaccounts| {
            subaccounts.insert(account.subaccount.unwrap_or(DEFAULT_SUBACCOUNT), token)
        });
    }

    pub fn total_supply(&self) -> Tokens128 {
        self.0
            .iter()
            .flat_map(|(_, subaccounts)| subaccounts.values())
            .fold(Tokens128::ZERO, |a, b| (a + b).unwrap_or(Tokens128::ZERO))
    }
}

/// A wrapper over stable state that is used only during upgrade process.
/// Since we have two different stable states (canister and auction), we need
/// to wrap it in this struct during canister upgrade.
#[derive(CandidType, Deserialize, Default)]
pub struct StableState {
    pub token_state: CanisterState,
    pub auction_state: AuctionState,
}

impl Versioned for StableState {
    type Previous = ();

    fn upgrade(_prev_state: Self::Previous) -> Self {
        Self::default()
    }
}
