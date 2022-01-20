use ic_cdk::export::candid::CandidType;
use ic_helpers::factory::Factory;
use ic_storage::IcStorage;
use serde::Deserialize;

#[derive(CandidType, Deserialize, IcStorage, Default)]
pub struct State {
    pub factory: Factory<String>,
}

pub fn get_token_bytecode() -> &'static [u8] {
    include_bytes!("token.wasm")
}

ic_helpers::impl_factory_state_management!(State, get_token_bytecode());
