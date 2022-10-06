use candid::Principal;
use canister_sdk::{
    ic_auction::{
        api::Auction,
        error::AuctionError,
        state::{AuctionInfo, AuctionState},
    },
    ic_canister::{self, init, post_upgrade, pre_upgrade, query, Canister, PreUpdate},
    ic_helpers::{
        candid_header::{candid_header, CandidHeader},
        tokens::Tokens128,
    },
    ic_metrics::{Interval, Metrics},
    ic_storage::IcStorage,
};
#[cfg(feature = "export-api")]
use canister_sdk::{ic_cdk, ic_cdk_macros::inspect_message};
use std::{cell::RefCell, rc::Rc};
use token_api::{
    account::AccountInternal,
    canister::{TokenCanisterAPI, DEFAULT_AUCTION_PERIOD_SECONDS},
    state::{
        balances::{Balances, StableBalances},
        stats::{Metadata, StatsData},
        CanisterState,
    },
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
        let owner_account = AccountInternal::new(owner, None);

        StableBalances.insert(owner_account, amount);

        self.state().borrow_mut().ledger.mint(
            AccountInternal::from(owner),
            AccountInternal::from(owner),
            amount,
        );

        StatsData::set_stable(metadata.into());

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
        // All required canister state stored in stable memory, so no need to save/load anything.
    }

    #[post_upgrade]
    fn post_upgrade(&self) {
        // All required canister state stored in stable memory, so no need to save/load anything.
    }

    #[query]
    pub fn state_check(&self) -> CandidHeader {
        candid_header::<CanisterState>()
    }
}

#[cfg(feature = "export-api")]
#[inspect_message]
fn inspect_message() {
    use canister_sdk::ic_cdk;
    use token_api::canister::AcceptReason;

    let method = ic_cdk::api::call::method_name();
    let caller = ic_cdk::api::caller();

    let accept_reason = match TokenCanister::inspect_message(&method, caller) {
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
        <Self as Auction>::canister_pre_update(self, method_name, method_type);
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
        token_api::canister::is20_auction::disburse_rewards(
            &mut self.state().borrow_mut(),
            &self.auction_state().borrow(),
        )
    }
}

impl Metrics for TokenCanister {}
