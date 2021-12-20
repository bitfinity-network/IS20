use crate::state::State;
use crate::types::TxError;
use ic_kit::ic;

pub fn check_caller_is_owner() -> Result<(), TxError> {
    if ic::caller() != State::get().stats().owner {
        Err(TxError::Unauthorized)
    } else {
        Ok(())
    }
}
