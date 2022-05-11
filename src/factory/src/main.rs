mod api;
mod error;
mod state;

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    use crate::error::TokenFactoryError;
    use candid::{Nat, Principal};
    use ic_helpers::factory::error::FactoryError;

    std::print!("{}", ic_canister::generate_idl!());
}
