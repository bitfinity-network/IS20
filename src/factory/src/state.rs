use crate::types::TokenKey;
use serde::{Deserialize, Serialize};

ic_helpers::init_state!(State, TokenKey, Settings, "token.wasm");

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Settings;
