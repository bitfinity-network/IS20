use candid::Principal;
use ic_cdk::export::candid::CandidType;
use ic_storage::{stable::Versioned, IcStorage};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(CandidType, Deserialize, IcStorage, Default)]
pub struct State {
    pub token_wasm: Option<Vec<u8>>,
    /// Associated list of token name and its principal
    pub tokens: HashMap<String, Principal>,
}

impl Versioned for State {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}

#[cfg(target_arch = "wasm32")]
pub fn _get_token_bytecode() -> Vec<u8> {
    State::get()
        .borrow()
        .token_wasm
        .clone()
        .expect("the token bytecode should be set before accessing it")
}
