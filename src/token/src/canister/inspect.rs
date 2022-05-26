use crate::state::CanisterState;
use candid::{Nat, Principal};
use ic_cdk_macros::inspect_message;
use ic_storage::IcStorage;

static PUBLIC_METHODS: &[&str] = &[
    "allowance",
    "auctionInfo",
    "balanceOf",
    "biddingInfo",
    "decimals",
    "getAllowanceSize",
    "getHolders",
    "getMetadata",
    "getTokenInfo",
    "getTransaction",
    "getTransactions",
    "getUserApprovals",
    "getUserTransactionAmount",
    "getUserTransactions",
    "historySize",
    "logo",
    "name",
    "owner",
    "symbol",
    "totalSupply",
    "isTestToken",
];

static OWNER_METHODS: &[&str] = &[
    "mint",
    "setAuctionPeriod",
    "setFee",
    "setFeeTo",
    "setLogo",
    "setMinCycles",
    "setName",
    "setOwner",
    "toggleTest",
];

static TRANSACTION_METHODS: &[&str] = &[
    "approve",
    "approveAndNotify",
    "burn",
    "transfer",
    "transferIncludeFee",
];

/// This function checks if the canister should accept ingress message or not. We allow query
/// calls for anyone, but update calls have different checks to see, if it's reasonable to spend
/// canister cycles on accepting this call. Check the comments in this method for details on
/// the checks for different methods.
#[cfg(not(feature = "no_api"))]
#[inspect_message]
fn inspect_message() {
    let method = ic_cdk::api::call::method_name();

    let state = CanisterState::get();
    let state = state.borrow();
    let caller = ic_cdk::api::caller();

    match &method[..] {
        // These are query methods, so no checks are needed.
        "mint" if state.stats.is_test_token => ic_cdk::api::call::accept_message(),
        m if PUBLIC_METHODS.contains(&m) => ic_cdk::api::call::accept_message(),
        // Owner
        m if OWNER_METHODS.contains(&m) && caller == state.stats.owner => {
            ic_cdk::api::call::accept_message()
        }
        // Not owner
        m if OWNER_METHODS.contains(&m) => {
            ic_cdk::println!("Owner method is called not by an owner. Rejecting.")
        }
        m if TRANSACTION_METHODS.contains(&m) => {
            // These methods requires that the caller have tokens.
            let state = CanisterState::get();
            let state = state.borrow();
            let balances = &state.balances;
            if !balances.0.contains_key(&caller) {
                ic_cdk::println!("Transaction method is not called by a stakeholder. Rejecting.");
                return;
            }

            // Anything but the `burn` method
            if caller == state.stats.owner || m != "burn" {
                ic_cdk::api::call::accept_message();
                return;
            }

            // It's the `burn` method and the caller isn't the owner.
            let from = ic_cdk::api::call::arg_data::<(Option<Principal>, Nat)>().0;
            if from.is_some() {
                ic_cdk::println!("Only the owner can burn other's tokens. Rejecting.");
                return;
            }

            ic_cdk::api::call::accept_message();
        }
        "transferFrom" => {
            // Check if the caller has allowance for this transfer.
            let allowances = &state.allowances;
            let (from, _, value) = ic_cdk::api::call::arg_data::<(Principal, Principal, Nat)>();
            if let Some(user_allowances) = allowances.get(&caller) {
                if let Some(allowance) = user_allowances.get(&from) {
                    if value <= *allowance {
                        ic_cdk::api::call::accept_message();
                    } else {
                        ic_cdk::println!("Allowance amount is less then the requested transfer amount. Rejecting.");
                    }
                } else {
                    ic_cdk::println!("Caller is not allowed to transfer tokens for the requested principal. Rejecting.");
                }
            } else {
                ic_cdk::println!("Caller is not allowed to transfer tokens for the requested principal. Rejecting.");
            }
        }
        "notify" => {
            // This method can only be called if the notification id is in the pending notifications
            // list.
            let notifications = &state.ledger.notifications;
            let (tx_id,) = ic_cdk::api::call::arg_data::<(Nat,)>();

            if notifications.contains_key(&tx_id) {
                ic_cdk::api::call::accept_message();
            } else {
                ic_cdk::println!("No pending notification with the given id. Rejecting.");
            }
        }
        "runAuction" => {
            // We allow running auction only to the owner or any of the cycle bidders.
            let state = CanisterState::get();
            let state = state.borrow();
            let bidding_state = &state.bidding_state;
            if bidding_state.is_auction_due()
                && (bidding_state.bids.contains_key(&caller) || caller == state.stats.owner)
            {
                ic_cdk::api::call::accept_message();
            } else {
                ic_cdk::println!("Auction is not due yet or auction run method is called not by owner or bidder. Rejecting.");
            }
        }
        "bidCycles" => {
            // We reject this message, because a call with cycles cannot be made through ingress,
            // only from the wallet canister.
        }
        _ => {
            ic_cdk::println!("The method called is not listed in the access checks. This is probably a code error.");
        }
    }
}
