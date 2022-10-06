pub mod balances;
pub mod stats;

#[cfg(feature = "claim")]
use crate::account::{Subaccount, DEFAULT_SUBACCOUNT};
use candid::Nat;
#[cfg(feature = "claim")]
use candid::Principal;
use candid::{CandidType, Deserialize};
#[cfg(feature = "auction")]
use canister_sdk::ic_auction::state::{AuctionInfo, AuctionState};
#[cfg(feature = "claim")]
use canister_sdk::ledger_canister::{AccountIdentifier, Subaccount as SubaccountIdentifier};
use canister_sdk::{
    ic_helpers::tokens::Tokens128,
    ic_storage::{stable::Versioned, IcStorage},
};

use crate::ledger::Ledger;

use stats::Value;

use self::stats::{Metadata, StatsData};

#[derive(Debug, Default, CandidType, Deserialize, IcStorage)]
pub struct CanisterState {
    pub ledger: Ledger,
}

impl CanisterState {
    pub fn icrc1_metadata(&self) -> Vec<(String, Value)> {
        let stats = StatsData::get_stable();
        vec![
            (
                "icrc1:symbol".to_string(),
                Value::Text(stats.symbol.clone()),
            ),
            ("icrc1:name".to_string(), Value::Text(stats.name.clone())),
            (
                "icrc1:decimals".to_string(),
                Value::Nat(Nat::from(stats.decimals)),
            ),
            ("icrc1:fee".to_string(), Value::Nat(stats.fee.amount.into())),
        ]
    }

    pub fn get_metadata(&self) -> Metadata {
        let stats = StatsData::get_stable();
        Metadata {
            logo: stats.logo.clone(),
            name: stats.name.clone(),
            symbol: stats.symbol.clone(),
            decimals: stats.decimals,
            owner: stats.owner,
            fee: stats.fee,
            fee_to: stats.fee_to,
            is_test_token: Some(stats.is_test_token),
        }
    }

    #[cfg(feature = "claim")]
    pub fn get_claimable_amount(
        &self,
        holder: Principal,
        subaccount: Option<Subaccount>,
    ) -> Tokens128 {
        use crate::{
            account::AccountInternal,
            state::balances::{Balances, StableBalances},
        };

        let claim_subaccount = AccountIdentifier::new(
            canister_sdk::ic_kit::ic::caller().into(),
            Some(SubaccountIdentifier(
                subaccount.unwrap_or(DEFAULT_SUBACCOUNT),
            )),
        )
        .to_address();

        let account = AccountInternal::new(holder, Some(claim_subaccount));
        StableBalances.balance_of(&account)
    }
}

impl Versioned for CanisterState {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}

/// A wrapper over stable state that is used only during upgrade process.
/// Since we have two different stable states (canister and auction), we need
/// to wrap it in this struct during canister upgrade.
#[derive(CandidType, Deserialize, Default, IcStorage)]
pub struct StableState {
    pub token_state: CanisterState,
    #[cfg(feature = "auction")]
    pub auction_state: AuctionState,
}

impl Versioned for StableState {
    type Previous = ();

    fn upgrade(_prev_state: Self::Previous) -> Self {
        Self::default()
    }
}

#[cfg(feature = "auction")]
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
