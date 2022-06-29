use std::{cell::RefCell, rc::Rc};

use candid::Principal;
use ic_canister::{Canister, PreUpdate};

use crate::{canister::TokenCanisterAPI, state::CanisterState, types::Metadata};

#[derive(Debug, Clone, Canister)]
pub struct TokenCanisterMock {
    #[id]
    principal: Principal,
    #[state]
    pub(crate) state: Rc<RefCell<CanisterState>>,
}

impl TokenCanisterMock {
    pub fn init(&self, metadata: Metadata) {
        self.state
            .borrow_mut()
            .balances
            .0
            .insert(metadata.owner, metadata.totalSupply);

        self.state
            .borrow_mut()
            .ledger
            .mint(metadata.owner, metadata.owner, metadata.totalSupply);

        self.state.borrow_mut().stats = metadata.into();
        self.state.borrow_mut().bidding_state.auction_period =
            crate::canister::DEFAULT_AUCTION_PERIOD;
    }
}

impl PreUpdate for TokenCanisterMock {
    fn pre_update(&self, method_name: &str, _method_type: ic_canister::MethodType) {
        if method_name != "runAuction" {
            if let Err(auction_error) = self.runAuction() {
                ic_cdk::println!("Auction error: {auction_error:#?}");
            }
        }
    }
}

impl TokenCanisterAPI for TokenCanisterMock {
    fn state(&self) -> Rc<RefCell<CanisterState>> {
        self.state.clone()
    }
}
