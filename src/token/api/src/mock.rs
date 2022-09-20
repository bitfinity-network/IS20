use std::{cell::RefCell, rc::Rc};

use candid::Principal;
#[cfg(feature = "auction")]
use ic_auction::{
    api::Auction,
    error::AuctionError,
    state::{AuctionInfo, AuctionState},
};
use ic_canister::{Canister, PreUpdate};
use ic_helpers::{metrics::Interval, tokens::Tokens128};
#[cfg(feature = "auction")]
use ic_storage::IcStorage;

use crate::{canister::TokenCanisterAPI, state::CanisterState, types::Metadata};

#[derive(Debug, Clone, Canister)]
pub struct TokenCanisterMock {
    #[id]
    principal: Principal,

    #[state]
    pub(crate) state: Rc<RefCell<CanisterState>>,
}

impl TokenCanisterMock {
    #[cfg_attr(coverage_nightly, no_coverage)]
    pub fn init(&self, metadata: Metadata, amount: Tokens128) {
        self.state
            .borrow_mut()
            .balances
            .insert(metadata.owner, None, amount);

        self.state
            .borrow_mut()
            .ledger
            .mint(metadata.owner.into(), metadata.owner.into(), amount);

        self.state.borrow_mut().stats = metadata.into();

        #[cfg(feature = "auction")]
        {
            let auction_state = self.auction_state();
            auction_state.replace(AuctionState::new(
                Interval::Period {
                    seconds: crate::canister::DEFAULT_AUCTION_PERIOD_SECONDS,
                },
                ic_canister::ic_kit::ic::caller(),
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
        crate::canister::is20_auction::disburse_rewards(
            &mut self.state().borrow_mut(),
            &self.auction_state().borrow(),
        )
    }
}

impl TokenCanisterAPI for TokenCanisterMock {
    #[cfg_attr(coverage_nightly, no_coverage)]
    fn state(&self) -> Rc<RefCell<CanisterState>> {
        self.state.clone()
    }
}
