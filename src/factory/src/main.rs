mod api;
mod state;

#[cfg(not(any(target_arch = "wasm32", test)))]
use candid::Principal;
#[cfg(not(any(target_arch = "wasm32", test)))]
use common::types::Metadata;

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    candid::export_service!();
    std::print!("{}", __export_service());
}
