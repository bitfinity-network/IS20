mod api;
mod error;
mod state;

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    use crate::{api::TokenFactoryCanister, error::TokenFactoryError};
    use candid::Principal;
    use ic_factory::api::FactoryCanister;
    use ic_factory::error::FactoryError;
    use ic_helpers::candid_header::CandidHeader;
    use ic_helpers::tokens::Tokens128;
    use token::types::Metadata;

    let canister_idl = ic_canister::generate_idl!();
    let mut factory_idl = <TokenFactoryCanister as FactoryCanister>::get_idl();
    factory_idl.merge(&canister_idl);

    let result = candid::bindings::candid::compile(&factory_idl.env.env, &Some(factory_idl.actor));
    println!("{result}");
}
