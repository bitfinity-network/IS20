pub mod api;
mod error;
pub mod state;

pub use self::api::*;
pub use state::State;

/// This is a marker added to the wasm to distinguish it from other canisters
#[no_mangle]
pub static CANISTER_MARKER: &str = "IS20_FACTORY_CANISTER";

pub fn idl() -> String {
    use crate::error::TokenFactoryError;
    use canister_sdk::{
        ic_canister::{generate_idl, Idl},
        ic_factory::{
            api::{FactoryCanister, UpgradeResult},
            error::FactoryError,
        },
        ic_helpers::{candid_header::CandidHeader, tokens::Tokens128},
    };
    use ic_exports::Principal;
    use std::collections::HashMap;
    use token::state::config::Metadata;

    let canister_idl = generate_idl!();
    let mut factory_idl = <TokenFactoryCanister as FactoryCanister>::get_idl();
    factory_idl.merge(&canister_idl);

    candid::bindings::candid::compile(&factory_idl.env.env, &Some(factory_idl.actor))
}
