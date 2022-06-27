#![allow(dead_code)]

mod canister;
mod exports;
mod ledger;
mod principal;
mod state;
mod types;

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    use token::canister::TokenCanister;
    use token::exports::TokenCanisterExports;

    let trait_idl = <TokenCanisterExports as TokenCanister>::get_idl();

    let result = candid::bindings::candid::compile(&trait_idl.env.env, &Some(trait_idl.actor));
    print!("{result}");
}
