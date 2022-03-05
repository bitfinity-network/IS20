//! This module contains common methods, used in different modules' unit tests.

use candid::Nat;
use common::types::Metadata;
use ic_kit::mock_principals::{alice, john};
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
        isTestToken: None,
    });
}

pub fn init_as_test_token() {
    crate::api::init(Metadata {
        logo: "".to_string(),
        name: "".to_string(),
        symbol: "".to_string(),
        decimals: 8,
        totalSupply: Nat::from(1000),
        owner: alice(),
        fee: Nat::from(0),
        feeTo: alice(),
        isTestToken: Some(true),
    });
}

pub fn init_with_fee() {
    crate::api::init(Metadata {
        logo: "".to_string(),
        name: "".to_string(),
        symbol: "".to_string(),
        decimals: 8,
        totalSupply: Nat::from(1000),
        owner: alice(),
        fee: Nat::from(100),
        feeTo: john(),
        isTestToken: None,
    });
}

pub fn init_context() -> &'static mut MockContext {
    let context = MockContext::new().with_caller(alice()).inject();
    default_canister_init();
    context
}
