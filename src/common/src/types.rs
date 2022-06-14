use candid::{CandidType, Principal};
use ic_helpers::tokens::Tokens128;
use serde::Deserialize;

#[allow(non_snake_case)]
#[derive(Deserialize, CandidType, Clone, Debug)]
pub struct Metadata {
    pub logo: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub totalSupply: Tokens128,
    pub owner: Principal,
    pub fee: Tokens128,
    pub feeTo: Principal,
    pub isTestToken: Option<bool>,
}
