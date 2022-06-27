use std::cell::RefCell;
use std::rc::Rc;

use ic_canister::generate_exports;
use ic_canister::Canister;
use ic_canister::PreUpdate;
use ic_cdk::export::candid::Principal;

use crate::canister::TokenCanister;
use crate::state::CanisterState;

#[derive(Debug, Clone, Canister)]
pub struct TokenCanisterExports {
    #[id]
    principal: Principal,

    #[state]
    pub(crate) state: Rc<RefCell<CanisterState>>,
}

impl PreUpdate for TokenCanisterExports {
    fn pre_update(&self, _method_name: &str, _method_type: ic_canister::MethodType) {
        if let Err(auction_error) = self.runAuction() {
            ic_cdk::println!("Auction error: {auction_error:#?}");
        }
    }
}

impl TokenCanister for TokenCanisterExports {
    fn state(&self) -> Rc<RefCell<CanisterState>> {
        self.state.clone()
    }
}

generate_exports!(TokenCanisterExports);

#[cfg(test)]
mod test {
    use super::*;
    use ic_canister::ic_kit::MockContext;

    #[test]
    fn test_upgrade_from_previous() {
        use ic_storage::stable::write;

        MockContext::new().inject();

        write(&()).unwrap();
        let canister = TokenCanisterExports::init_instance();
        canister.__post_upgrade_inst();
    }

    #[test]
    fn test_upgrade_from_current() {
        MockContext::new().inject();

        // Set a value on the state...
        let canister = TokenCanisterExports::init_instance();
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
