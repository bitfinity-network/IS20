mod api;
mod state;
mod error;

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    use candid::{Nat, Principal};
    use common::types::Metadata;
    use ic_helpers::factory::error::FactoryError;
    use crate::error::TokenFactoryError;

    candid::export_service!();
    std::print!("{}", __export_service());
}
