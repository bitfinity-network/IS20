mod api;
mod error;
mod state;

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    use crate::error::TokenFactoryError;
    use candid::{Nat, Principal};
    use common::types::Metadata;
    use ic_helpers::factory::error::FactoryError;

    candid::export_service!();
    std::print!("{}", __export_service());
}
