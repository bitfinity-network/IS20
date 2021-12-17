//! This module contains common methods, used in different modules' unit tests.

use candid::Nat;
use ic_kit::mock_principals::alice;
use ic_kit::MockContext;

pub fn default_canister_init() {
    crate::init(
        "".into(),
        "".into(),
        "".into(),
        8,
        Nat::from(1000),
        alice(),
        Nat::from(0),
        alice(),
    );
}

pub fn init_context() -> &'static mut MockContext {
    let context = MockContext::new().with_caller(alice()).inject();
    default_canister_init();
    context
}
