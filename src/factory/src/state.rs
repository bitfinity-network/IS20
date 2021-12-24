use serde::{Deserialize, Serialize};

ic_helpers::init_state!(State, String, Settings, "token.wasm");

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Settings;
