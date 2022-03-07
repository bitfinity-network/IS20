use candid::{CandidType, Nat, Principal};
use serde::Deserialize;

#[allow(non_snake_case)]
#[derive(Deserialize, CandidType, Clone, Debug)]
pub struct Metadata {
    pub logo: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub totalSupply: Nat,
    pub owner: Principal,
    pub fee: Nat,
    pub feeTo: Principal,
    pub isTestToken: Option<bool>,
}
