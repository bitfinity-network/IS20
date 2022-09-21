use std::collections::HashMap;

use candid::Nat;
use candid::{CandidType, Deserialize, Principal};
use canister_sdk::ic_auction::state::{AuctionInfo, AuctionState};
use canister_sdk::ic_helpers::tokens::Tokens128;
use canister_sdk::ic_storage::stable::Versioned;
use canister_sdk::ic_storage::IcStorage;
use canister_sdk::ledger_canister::AccountIdentifier;
use canister_sdk::ledger_canister::Subaccount as SubaccountIdentifier;

use crate::account::{AccountInternal, Subaccount, DEFAULT_SUBACCOUNT};
use crate::ledger::Ledger;
use crate::types::{Claims, Metadata, StatsData, Value};

#[derive(Debug, Default, CandidType, Deserialize, IcStorage)]
pub struct CanisterState {
    pub balances: Balances,
    pub stats: StatsData,
    pub ledger: Ledger,

    // We leave this field here to not introduce a new version of the state.
    #[deprecated(note = "claims are now stored in owner's subaccounts.")]
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

    pub fn get_claimable_amount(
        &self,
        holder: Principal,
        subaccount: Option<Subaccount>,
    ) -> Tokens128 {
        let claim_subaccount = AccountIdentifier::new(
            canister_sdk::ic_kit::ic::caller().into(),
            Some(SubaccountIdentifier(
                subaccount.unwrap_or(DEFAULT_SUBACCOUNT),
            )),
        )
        .to_address();

        let claim_account = AccountInternal::new(holder, Some(claim_subaccount));
        self.balances.balance_of(claim_account)
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

    pub fn get_mut(&mut self, account: AccountInternal) -> Option<&mut Tokens128> {
        self.0
            .get_mut(&account.owner)
            .and_then(|subaccounts| subaccounts.get_mut(&account.subaccount))
    }

    pub fn get_mut_or_insert_default(&mut self, account: AccountInternal) -> &mut Tokens128 {
        self.0
            .entry(account.owner)
            .or_default()
            .entry(account.subaccount)
            .or_default()
    }

    pub fn balance_of(&self, account: AccountInternal) -> Tokens128 {
        self.0
            .get(&account.owner)
            .and_then(|subaccounts| subaccounts.get(&account.subaccount))
            .copied()
            .unwrap_or_default()
    }

    pub fn get_holders(&self, start: usize, limit: usize) -> Vec<(AccountInternal, Tokens128)> {
        let mut holders = self
            .0
            .iter()
            .flat_map(|(principal, subaccounts)| {
                subaccounts
                    .iter()
                    .map(|(subaccount, token)| {
                        (AccountInternal::new(*principal, Some(*subaccount)), *token)
                    })
                    .collect::<Vec<_>>()
            })
            .skip(start)
            .take(limit)
            .collect::<Vec<_>>();
        holders.sort_by(|a, b| b.1.cmp(&a.1));
        holders
    }

    pub fn remove(&mut self, account: AccountInternal) {
        if let Some(subaccounts) = self.0.get_mut(&account.owner) {
            subaccounts.remove(&account.subaccount);
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

    pub fn set_balance(&mut self, account: AccountInternal, amount: Tokens128) {
        self.0
            .entry(account.owner)
            .or_default()
            .insert(account.subaccount, amount);
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
                self.set_balance(AccountInternal::new(*principal, Some(*subaccount)), *amount);
            }
        }
    }

    pub fn list_subaccounts(&self, account: Principal) -> HashMap<Subaccount, Tokens128> {
        self.0.get(&account).cloned().unwrap_or_default()
    }
}

/// A wrapper over stable state that is used only during upgrade process.
/// Since we have two different stable states (canister and auction), we need
/// to wrap it in this struct during canister upgrade.
#[derive(CandidType, Deserialize, Default, IcStorage)]
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
