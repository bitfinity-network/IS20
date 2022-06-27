#![allow(dead_code)]

mod canister;
mod core;
mod ledger;
mod principal;
mod state;
mod types;

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    use token::canister::TokenCanister;
    use token::core::ISTokenCanister;

    let trait_idl = <TokenCanister as ISTokenCanister>::get_idl();

    let result = candid::bindings::candid::compile(&trait_idl.env.env, &Some(trait_idl.actor));
    print!("{result}");
}
