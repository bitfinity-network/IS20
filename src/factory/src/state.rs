use canister_sdk::{
    ic_cdk::export::candid::CandidType,
    ic_exports::Principal,
    ic_factory::{v1::FactoryStateV1, FactoryState},
    ic_storage::{stable::Versioned, IcStorage},
};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(CandidType, Deserialize, IcStorage, Default, Debug)]
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
    type Previous = StableStateV1;

    fn upgrade(prev_state: Self::Previous) -> Self {
        Self {
            base_factory_state: FactoryState::upgrade(prev_state.base_factory_state),
            token_factory_state: prev_state.token_factory_state,
        }
    }
}

#[derive(CandidType, Deserialize, Default)]
pub struct StableStateV1 {
    pub token_factory_state: State,
    pub base_factory_state: FactoryStateV1,
}

impl Versioned for StableStateV1 {
    type Previous = ();

    fn upgrade(_: Self::Previous) -> Self {
        Self::default()
    }
}
