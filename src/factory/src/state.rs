use candid::Principal;
use ic_cdk::export::candid::CandidType;
use ic_factory::FactoryState;
use ic_storage::{stable::Versioned, IcStorage};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(CandidType, Deserialize, IcStorage, Default)]
pub struct State {
    pub token_wasm: Option<Vec<u8>>,
    /// Associated list of token name and its principal
    pub tokens: HashMap<String, Principal>,
}

#[derive(CandidType, Deserialize, Default)]
pub struct StableState {
    pub token_factory_state: State,
    pub base_factory_state: FactoryState,
}

impl Versioned for StableState {
    type Previous = ();

    fn upgrade(_: Self::Previous) -> Self {
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
