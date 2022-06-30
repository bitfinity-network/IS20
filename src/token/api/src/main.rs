#![allow(dead_code)]

mod canister;
mod ledger;
mod mock;
mod principal;
mod state;
mod types;

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    use crate::canister::{TokenCanisterAPI, TokenCanisterExports};

    let trait_idl = <TokenCanisterExports as TokenCanisterAPI>::get_idl();

    let result = candid::bindings::candid::compile(&trait_idl.env.env, &Some(trait_idl.actor));
    print!("{result}");
}
