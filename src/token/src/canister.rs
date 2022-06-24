use std::cell::RefCell;
use std::rc::Rc;

use ic_canister::generate_exports;
use ic_canister::Canister;
use ic_cdk::export::candid::Principal;

use crate::core::ISTokenCanister;
use crate::state::CanisterState;

pub mod erc20_transactions;

#[cfg(not(feature = "no_api"))]
mod inspect;

pub mod is20_auction;
pub mod is20_notify;
pub mod is20_transactions;

#[derive(Debug, Clone, Canister)]
pub struct TokenCanister {
    #[id]
    principal: Principal,

    #[state]
    pub(crate) state: Rc<RefCell<CanisterState>>,
}

impl ISTokenCanister for TokenCanister {
    fn state(&self) -> Rc<RefCell<CanisterState>> {
        self.state.clone()
    }
}

generate_exports!(TokenCanister);

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_upgrade_from_previous() {
        use ic_storage::stable::write;
        write(&()).unwrap();
        let canister = TokenCanister::init_instance();
        canister.__post_upgrade_inst();
    }

    #[test]
    fn test_upgrade_from_current() {
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
