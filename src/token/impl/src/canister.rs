use candid::Principal;
use ic_auction::{
    api::Auction,
    error::AuctionError,
    state::{AuctionInfo, AuctionState},
};
use ic_canister::{init, post_upgrade, pre_upgrade, Canister, PreUpdate};

#[cfg(not(feature = "no_api"))]
use ic_cdk_macros::inspect_message;

use ic_canister::query;
use ic_helpers::{
    candid_header::{candid_header, CandidHeader},
    metrics::{Interval, Metrics},
    tokens::Tokens128,
};
use ic_storage::IcStorage;
use std::{cell::RefCell, rc::Rc};
use token_api::{
    account::AccountInternal,
    canister::{TokenCanisterAPI, DEFAULT_AUCTION_PERIOD_SECONDS},
    state::{CanisterState, StableState},
    types::Metadata,
};

#[derive(Debug, Clone, Canister)]
#[canister_no_upgrade_methods]
pub struct TokenCanister {
    #[id]
    principal: Principal,
}

impl TokenCanister {
    #[init]
    pub fn init(&self, metadata: Metadata, amount: Tokens128) {
        let owner = metadata.owner;
        self.state()
            .borrow_mut()
            .balances
            .insert(owner, None, amount);

        self.state().borrow_mut().ledger.mint(
            AccountInternal::from(owner),
            AccountInternal::from(owner),
            amount,
        );

        self.state().borrow_mut().stats = metadata.into();

        let auction_state = self.auction_state();
        auction_state.replace(AuctionState::new(
            Interval::Period {
                seconds: DEFAULT_AUCTION_PERIOD_SECONDS,
            },
            owner,
        ));
    }

    #[pre_upgrade]
    fn pre_upgrade(&self) {
        let token_state = self.state().replace(CanisterState::default());
        let auction_state = self.auction_state().replace(AuctionState::default());

        if let Err(err) = ic_storage::stable::write(&StableState {
            token_state,
            auction_state,
        }) {
            ic_canister::ic_kit::ic::trap(&format!(
                "Error while serializing state to the stable storage: {err}"
            ));
        }
    }

    #[post_upgrade]
    fn post_upgrade(&self) {
        let stable_state = ic_storage::stable::read::<StableState>().unwrap_or_else(|err| {
            ic_canister::ic_kit::ic::trap(&format!(
                "Error while deserializing state from the stable storage: {err}"
            ));
        });

        let StableState {
            token_state,
            auction_state,
        } = stable_state;

        self.state().replace(token_state);
        self.auction_state().replace(auction_state);
    }

    #[query]
    pub fn state_check(&self) -> CandidHeader {
        candid_header::<CanisterState>()
    }
}

#[cfg(not(feature = "no_api"))]
#[inspect_message]
fn inspect_message() {
    use ic_storage::IcStorage;
    use token_api::canister::AcceptReason;

    let method = ic_cdk::api::call::method_name();

    let state = CanisterState::get();
    let state = state.borrow();
    let caller = ic_cdk::api::caller();

    let accept_reason = match TokenCanister::inspect_message(&state, &method, caller) {
        Ok(accept_reason) => accept_reason,
        Err(msg) => ic_cdk::trap(msg),
    };

    match accept_reason {
        AcceptReason::Valid => ic_cdk::api::call::accept_message(),
        AcceptReason::NotIS20Method => ic_cdk::trap("Unknown method"),
    }
}

impl PreUpdate for TokenCanister {
    fn pre_update(&self, method_name: &str, method_type: ic_canister::MethodType) {
        token_api::canister::pre_update(self, method_name, method_type);

        self.update_metrics();
    }
}

impl TokenCanisterAPI for TokenCanister {
    fn state(&self) -> Rc<RefCell<CanisterState>> {
        CanisterState::get()
    }
}

impl Auction for TokenCanister {
    fn auction_state(&self) -> Rc<RefCell<AuctionState>> {
        AuctionState::get()
    }

    fn disburse_rewards(&self) -> Result<AuctionInfo, AuctionError> {
        token_api::canister::is20_auction::disburse_rewards(self)
    }
}

impl Metrics for TokenCanister {}

#[cfg(test)]
mod test {
    use super::*;
    use ic_canister::ic_kit::MockContext;

    #[test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    fn test_upgrade_from_current() {
        MockContext::new().inject();

        // Set a value on the state...
        let canister = TokenCanister::init_instance();
        canister.state().borrow_mut().stats.name = "To Kill a Mockingbird".to_string();
        // ... write the state to stable storage
        canister.__pre_upgrade();

        // Update the value without writing it to stable storage
        canister.state().borrow_mut().stats.name = "David Copperfield".to_string();

        // Upgrade the canister should have the state
        // written before pre_upgrade
        canister.__post_upgrade();
        assert_eq!(
            canister.state().borrow().stats.name,
            "To Kill a Mockingbird".to_string()
        );
    }
}
