#![allow(dead_code)]

mod canister;

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    use crate::canister::TokenCanister;
    use token_api::canister::TokenCanisterAPI;

    let trait_idl = <TokenCanister as TokenCanisterAPI>::get_idl();

    let result = candid::bindings::candid::compile(&trait_idl.env.env, &Some(trait_idl.actor));
    print!("{result}");
}
