use std::{cell::RefCell, rc::Rc};

use candid::Principal;
use ic_auction::{
    api::Auction,
    error::AuctionError,
    state::{AuctionInfo, AuctionState},
};
use ic_canister::{Canister, PreUpdate};
use ic_helpers::{metrics::Interval, tokens::Tokens128};
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

        let auction_state = self.auction_state();
        auction_state.replace(AuctionState::new(
            Interval::Period {
                seconds: crate::canister::DEFAULT_AUCTION_PERIOD_SECONDS,
            },
            ic_canister::ic_kit::ic::caller(),
        ));
    }
}

impl PreUpdate for TokenCanisterMock {
    fn pre_update(&self, method_name: &str, method_type: ic_canister::MethodType) {
        crate::canister::pre_update(self, method_name, method_type)
    }
}

impl Auction for TokenCanisterMock {
    fn auction_state(&self) -> Rc<RefCell<AuctionState>> {
        AuctionState::get()
    }

    fn disburse_rewards(&self) -> Result<AuctionInfo, AuctionError> {
        crate::canister::is20_auction::disburse_rewards(self)
    }
}

impl TokenCanisterAPI for TokenCanisterMock {
    fn state(&self) -> Rc<RefCell<CanisterState>> {
        self.state.clone()
    }
}
