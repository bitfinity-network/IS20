//! This module contains common methods, used in different modules' unit tests.

use candid::Nat;
use common::types::Metadata;
use ic_kit::mock_principals::alice;
use ic_kit::MockContext;

pub fn default_canister_init() {
    crate::api::init(Metadata {
        logo: "".to_string(),
        name: "".to_string(),
        symbol: "".to_string(),
        decimals: 8,
        totalSupply: Nat::from(1000),
        owner: alice(),
        fee: Nat::from(0),
        feeTo: alice(),
    });
}

pub fn init_context() -> &'static mut MockContext {
    let context = MockContext::new().with_caller(alice()).inject();
    default_canister_init();
    context
}
