pub mod api;
mod error;
pub mod state;

pub use self::api::*;
pub use state::State;

pub fn idl() -> String {
    use crate::error::TokenFactoryError;
    use candid::Principal;
    use canister_sdk::{
        ic_canister::{generate_idl, Idl},
        ic_factory::{
            api::{FactoryCanister, UpgradeResult},
            error::FactoryError,
        },
        ic_helpers::{candid_header::CandidHeader, tokens::Tokens128},
    };
    use std::collections::HashMap;
    use token::types::Metadata;

    let canister_idl = generate_idl!();
    let mut factory_idl = <TokenFactoryCanister as FactoryCanister>::get_idl();
    factory_idl.merge(&canister_idl);

    candid::bindings::candid::compile(&factory_idl.env.env, &Some(factory_idl.actor))
}
