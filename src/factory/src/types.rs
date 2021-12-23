use serde::{Deserialize, Serialize};
use std::hash::Hash;

#[derive(Clone, Serialize, Deserialize, Eq, Hash, PartialEq)]
pub struct TokenKey(String, String);

impl TokenKey {
    pub fn new(name: String, symbol: String) -> Self {
        Self(name, symbol)
    }

    pub fn name(&self) -> String {
        self.0.clone()
    }

    pub fn symbol(&self) -> String {
        self.1.clone()
    }
}

impl From<(String, String)> for TokenKey {
    fn from(info: (String, String)) -> Self {
        Self::new(info.0, info.1)
    }
}

impl From<TokenKey> for (String, String) {
    fn from(key: TokenKey) -> Self {
        (key.0, key.1)
    }
}
