use candid::{Nat, Principal};

use crate::state::{
    balances::{Balances, StableBalances},
    config::TokenConfig,
};

static OWNER_METHODS: &[&str] = &[
    "set_auction_period",
    "set_fee",
    "set_fee_to",
    "set_logo",
    "set_min_cycles",
    "set_name",
    "set_symbol",
    "set_owner",
];

static TRANSACTION_METHODS: &[&str] = &["burn", "icrc1_transfer"];

/// Reason why the method may be accepted.
#[derive(Debug, Clone, Copy)]
pub enum AcceptReason {
    /// The call is a part of the IS20 API and can be performed.
    Valid,
    /// The method isn't a part of the IS20 API, and may require further validation.
    NotIS20Method,
}

/// This function checks if the canister should accept ingress message or not. We allow query
/// calls for anyone, but update calls have different checks to see, if it's reasonable to spend
/// canister cycles on accepting this call. Check the comments in this method for details on
/// the checks for different methods.
pub fn inspect_message(method: &str, caller: Principal) -> Result<AcceptReason, &'static str> {
    let stats = TokenConfig::get_stable();
    match method {
        // These are query methods, so no checks are needed.
        #[cfg(feature = "mint_burn")]
        "mint" if stats.is_test_token => Ok(AcceptReason::Valid),
        #[cfg(feature = "mint_burn")]
        "mint" if caller == stats.owner => Ok(AcceptReason::Valid),
        #[cfg(feature = "mint_burn")]
        "mint" => Err("Only the owner can mint"),
        // Owner
        m if OWNER_METHODS.contains(&m) && caller == stats.owner => Ok(AcceptReason::Valid),
        // Not owner
        m if OWNER_METHODS.contains(&m) => {
            Err("Owner method is called not by an owner. Rejecting.")
        }
        #[cfg(any(feature = "transfer", feature = "mint_burn"))]
        m if TRANSACTION_METHODS.contains(&m) => {
            // These methods requires that the caller have tokens.

            if StableBalances.get_subaccounts(caller).is_empty() {
                return Err("Transaction method is not called by a stakeholder. Rejecting.");
            }

            // Anything but the `burn` method
            if caller == stats.owner || m != "burn" {
                return Ok(AcceptReason::Valid);
            }

            // It's the `burn` method and the caller isn't the owner.
            let from = canister_sdk::ic_cdk::api::call::arg_data::<(Option<Principal>, Nat)>().0;
            if from.is_some() {
                return Err("Only the owner can burn other's tokens. Rejecting.");
            }

            Ok(AcceptReason::Valid)
        }
        "bid_cycles" => {
            // We reject this message, because a call with cycles cannot be made through ingress,
            // only from the wallet canister.
            Err("Call with cycles cannot be made through ingress.")
        }
        _ => Ok(AcceptReason::NotIS20Method),
    }
}
