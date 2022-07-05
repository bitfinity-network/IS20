use std::{cell::RefCell, rc::Rc};

use candid::Principal;
use ic_canister::{init, Canister, PreUpdate};
use token_api::{
    canister::{TokenCanisterAPI, DEFAULT_AUCTION_PERIOD},
    state::CanisterState,
    types::Metadata,
};

#[derive(Debug, Clone, Canister)]
pub struct TokenCanister {
    #[id]
    principal: Principal,
    #[state]
    pub(crate) state: Rc<RefCell<CanisterState>>,
}

impl TokenCanister {
    #[init]
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
        self.state.borrow_mut().bidding_state.auction_period = DEFAULT_AUCTION_PERIOD;
    }
}

impl PreUpdate for TokenCanister {
    fn pre_update(&self, method_name: &str, method_type: ic_canister::MethodType) {
        token_api::canister::pre_update(self, method_name, method_type);
    }
}

impl TokenCanisterAPI for TokenCanister {
    fn state(&self) -> Rc<RefCell<CanisterState>> {
        self.state.clone()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ic_canister::ic_kit::MockContext;

    #[test]
    fn test_upgrade_from_previous() {
        use ic_storage::stable::write;

        MockContext::new().inject();

        write(&()).unwrap();
        let canister = TokenCanister::init_instance();
        canister.__post_upgrade_inst();
    }

    #[test]
    fn test_upgrade_from_current() {
        MockContext::new().inject();

        // Set a value on the state...
        let canister = TokenCanister::init_instance();
        let mut state = canister.state.borrow_mut();
        state.bidding_state.fee_ratio = 12345.0;
        drop(state);
        // ... write the state to stable storage
        canister.__pre_upgrade_inst();

        // Update the value without writing it to stable storage
        let mut state = canister.state.borrow_mut();
        state.bidding_state.fee_ratio = 0.0;
        drop(state);

        // Upgrade the canister should have the state
        // written before pre_upgrade
        canister.__post_upgrade_inst();
        let state = canister.state.borrow();
        assert_eq!(state.bidding_state.fee_ratio, 12345.0);
    }
}
