use crate::state::State;
use canister_sdk::{
    ic_cdk, ic_cdk_macros::inspect_message, ic_factory::FactoryState, ic_storage::IcStorage,
};

#[inspect_message]
fn inspect_message() {
    let state = State::get();
    let state = state.borrow();
    let factory = FactoryState::get();
    let factory = factory.borrow();

    if ic_cdk::api::call::method_name() == "set_token_bytecode" {
        if factory.controller() == canister_sdk::ic_kit::ic::caller() {
            return ic_cdk::api::call::accept_message();
        }

        ic_cdk::trap(&format!(
            "The caller {} is not a factory controller {}.",
            canister_sdk::ic_kit::ic::caller(),
            factory.controller()
        ));
    }

    match state.token_wasm {
        Some(_) => ic_cdk::api::call::accept_message(),
        None => ic_cdk::trap("The factory hasn't been completely intialized yet."),
    }
}
