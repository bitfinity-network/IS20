use canister_sdk::{
    ic_auction::{
        api::Auction,
        error::AuctionError,
        state::{AuctionInfo, AuctionState},
    },
    ic_canister::{self, init, post_upgrade, pre_upgrade, Canister, PreUpdate},
    ic_helpers::tokens::Tokens128,
    ic_metrics::{Interval, Metrics, MetricsStorage},
    ic_storage::IcStorage,
};
#[cfg(feature = "export-api")]
use canister_sdk::{ic_cdk, ic_cdk_macros::inspect_message};
use ic_exports::Principal;
use std::{cell::RefCell, rc::Rc};
use token_api::{
    account::AccountInternal,
    canister::{TokenCanisterAPI, DEFAULT_AUCTION_PERIOD_SECONDS},
    state::{
        balances::{Balances, StableBalances},
        config::{Metadata, TokenConfig},
        ledger::LedgerData,
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

        StableBalances.clear();
        StableBalances.insert(owner_account, amount);

        LedgerData::mint(
            AccountInternal::from(owner),
            AccountInternal::from(owner),
            amount,
        );

        TokenConfig::set_stable(metadata.into());

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

impl TokenCanisterAPI for TokenCanister {}

impl Auction for TokenCanister {
    fn auction_state(&self) -> Rc<RefCell<AuctionState>> {
        AuctionState::get()
    }

    fn disburse_rewards(&self) -> Result<AuctionInfo, AuctionError> {
        token_api::canister::is20_auction::disburse_rewards(&self.auction_state().borrow())
    }
}

impl Metrics for TokenCanister {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        MetricsStorage::get()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use canister_sdk::ic_kit::MockContext;

    #[test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    fn test_upgrade_from_current() {
        MockContext::new().inject();

        // Set a value on the state and write it to stable storage.
        let canister = TokenCanister::init_instance();
        let mut stats = TokenConfig::get_stable();
        stats.name = "To Kill a Mockingbird".to_string();
        TokenConfig::set_stable(stats);

        canister.pre_upgrade();
        canister.post_upgrade();

        // Upgrade the canister should have the state
        // written before pre_upgrade
        assert_eq!(
            TokenConfig::get_stable().name,
            "To Kill a Mockingbird".to_string()
        );
    }
}
