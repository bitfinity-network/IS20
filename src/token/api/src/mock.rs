use std::{cell::RefCell, rc::Rc};

#[cfg(feature = "auction")]
use canister_sdk::ic_auction::{
    api::Auction,
    error::AuctionError,
    state::{AuctionInfo, AuctionState},
};
use canister_sdk::{
    ic_canister::{self, Canister, PreUpdate},
    ic_helpers::tokens::Tokens128,
    ic_metrics::Interval,
    ic_storage::IcStorage,
};
use ic_exports::Principal;

use crate::{
    account::AccountInternal,
    canister::TokenCanisterAPI,
    state::{
        balances::{Balances, StableBalances},
        config::{Metadata, TokenConfig},
        ledger::LedgerData,
    },
};

#[derive(Debug, Clone, Canister)]
pub struct TokenCanisterMock {
    #[id]
    principal: Principal,
}

impl TokenCanisterMock {
    #[cfg_attr(coverage_nightly, no_coverage)]
    pub fn init(&self, metadata: Metadata, amount: Tokens128) {
        let owner_account = AccountInternal::new(metadata.owner, None);
        StableBalances.insert(owner_account, amount);

        LedgerData::mint(metadata.owner.into(), metadata.owner.into(), amount);

        TokenConfig::set_stable(metadata.into());

        #[cfg(feature = "auction")]
        {
            let auction_state = self.auction_state();
            auction_state.replace(AuctionState::new(
                Interval::Period {
                    seconds: crate::canister::DEFAULT_AUCTION_PERIOD_SECONDS,
                },
                canister_sdk::ic_kit::ic::caller(),
            ));
        }
    }
}

impl PreUpdate for TokenCanisterMock {
    #[cfg_attr(coverage_nightly, no_coverage)]
    fn pre_update(&self, method_name: &str, method_type: ic_canister::MethodType) {
        #[cfg(feature = "auction")]
        <Self as Auction>::canister_pre_update(self, method_name, method_type);
    }
}

#[cfg(feature = "auction")]
impl Auction for TokenCanisterMock {
    fn auction_state(&self) -> Rc<RefCell<AuctionState>> {
        AuctionState::get()
    }

    fn disburse_rewards(&self) -> Result<AuctionInfo, AuctionError> {
        crate::canister::is20_auction::disburse_rewards(&self.auction_state().borrow())
    }
}

impl TokenCanisterAPI for TokenCanisterMock {}
