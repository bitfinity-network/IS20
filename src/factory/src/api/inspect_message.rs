use crate::state;
use canister_sdk::{ic_cdk, ic_cdk_macros::inspect_message, ic_factory::FactoryState};

#[inspect_message]
fn inspect_message() {
    let state = state::get_state();
    let factory = FactoryState::default();

    if ic_cdk::api::call::method_name() == "set_token_bytecode" {
        if factory.controller() == canister_sdk::ic_kit::ic::caller() {
            return ic_cdk::api::call::accept_message();
        }

        ic_cdk::trap(&format!(
            "the caller {} is not a factory controller {}",
            canister_sdk::ic_kit::ic::caller(),
            factory.controller()
        ));
    }

    match state.get_token_wasm() {
        Some(_) => ic_cdk::api::call::accept_message(),
        None => ic_cdk::trap("the factory hasn't been completely intialized yet"),
    }
}
