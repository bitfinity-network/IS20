use candid::Principal;
use canister_sdk::{
    ic_auction::{
        api::Auction,
        error::AuctionError,
        state::{AuctionInfo, AuctionState},
    },
    ic_canister::{self, init, query, Canister, PreUpdate},
    ic_helpers::{
        candid_header::{candid_header, CandidHeader},
        tokens::Tokens128,
    },
    ic_metrics::{Interval, Metrics},
    ic_storage::IcStorage,
};
#[cfg(feature = "export_api")]
use canister_sdk::{ic_cdk, ic_cdk_macros::inspect_message};
use std::{cell::RefCell, rc::Rc};
use token_api::{
    account::AccountInternal,
    canister::{TokenCanisterAPI, DEFAULT_AUCTION_PERIOD_SECONDS},
    state::{
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
        self.state()
            .borrow_mut()
            .balances
            .insert(owner, None, amount);

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

    // #[pre_upgrade]
    // fn pre_upgrade(&self) {
    //     let token_state = self.state().replace(CanisterState::default());
    //     let auction_state = self.auction_state().replace(AuctionState::default());

    //     if let Err(err) = ic_storage::stable::write(&StableState {
    //         token_state,
    //         auction_state,
    //     }) {
    //         canister_sdk::ic_kit::ic::trap(&format!(
    //             "Error while serializing state to the stable storage: {err}"
    //         ));
    //     }
    // }

    // #[post_upgrade]
    // fn post_upgrade(&self) {
    //     let stable_state = ic_storage::stable::read::<StableState>().unwrap_or_else(|err| {
    //         canister_sdk::ic_kit::ic::trap(&format!(
    //             "Error while deserializing state from the stable storage: {err}"
    //         ));
    //     });

    //     let StableState {
    //         token_state,
    //         auction_state,
    //     } = stable_state;

    //     self.state().replace(token_state);
    //     self.auction_state().replace(auction_state);
    // }

    #[query]
    pub fn state_check(&self) -> CandidHeader {
        candid_header::<CanisterState>()
    }
}

#[cfg(feature = "export_api")]
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
