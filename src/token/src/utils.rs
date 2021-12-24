use crate::state::State;
use crate::types::TxError;
use ic_kit::ic;
use ic_storage::IcStorage;

pub fn check_caller_is_owner() -> Result<(), TxError> {
    let state = State::get();
    if ic::caller() != state.borrow().stats().owner {
        Err(TxError::Unauthorized)
    } else {
        Ok(())
    }
}
