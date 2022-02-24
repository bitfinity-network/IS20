#[cfg(not(any(target_arch = "wasm32", test)))]
use crate::api::is20_auction::{AuctionError, BiddingInfo};
#[cfg(not(any(target_arch = "wasm32", test)))]
use crate::types::{AuctionInfo, TokenInfo, TxError, TxReceipt, TxRecord};
#[cfg(not(any(target_arch = "wasm32", test)))]
use candid::{Nat, Principal};

#[cfg(feature = "api")]
mod api;
mod ledger;
mod state;
mod types;
mod utils;

#[cfg(not(any(target_arch = "wasm32", test)))]
use common::types::Metadata;

#[cfg(test)]
pub mod tests;

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    candid::export_service!();
    std::print!("{}", __export_service());
}
