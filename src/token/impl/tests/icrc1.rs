use canister_sdk::{
    ic_canister::Canister,
    ic_helpers::tokens::Tokens128,
    ic_kit::{
        mock_principals::{alice, bob, john},
        MockContext,
    },
};
use ic_exports::Principal;
use is20_token_canister::canister::TokenCanister;
use token_api::{
    account::Account,
    canister::TokenCanisterAPI,
    error::TransferError,
    state::config::{Metadata, StandardRecord, Value},
    state::{
        balances::{Balances, StableBalances},
        config::TokenConfig,
        ledger::{LedgerData, TransferArgs},
    },
};

fn init() -> (Metadata, TokenCanister, &'static mut MockContext) {
    let context = canister_sdk::ic_kit::MockContext::new().inject();

    let principal = Principal::from_text("mfufu-x6j4c-gomzb-geilq").unwrap();
    let canister = TokenCanister::from_principal(principal);
    context.update_id(canister.principal());

    // Refresh canister's state.
    TokenConfig::set_stable(TokenConfig::default());
    StableBalances.clear();
    LedgerData::clear();

    let meta = Metadata {
        decimals: 11,
        fee: 127.into(),
        fee_to: alice(),
        name: "Testo".into(),
        symbol: "TST".into(),
        owner: alice(),
        is_test_token: None,
    };
    canister.init(meta.clone(), 1_000_000_000.into());
    (meta, canister, context)
}

#[test]
fn meta_fields_getting() {
    let (meta, canister, _) = init();

    assert_eq!(canister.icrc1_name(), meta.name);
    assert_eq!(canister.icrc1_symbol(), meta.symbol);
    assert_eq!(canister.icrc1_decimals(), meta.decimals);
    assert_eq!(canister.icrc1_fee(), meta.fee);
    assert_eq!(canister.icrc1_total_supply(), 1_000_000_000.into());
    assert_eq!(
        canister.icrc1_balance_of(Account::new(bob(), None)),
        0.into()
    );
    assert_eq!(
        canister.icrc1_balance_of(Account::new(alice(), None)),
        1_000_000_000.into()
    );
}

#[test]
fn supported_standards() {
    let (_, canister, _) = init();

    let standards = canister.icrc1_supported_standards();
    assert!(standards.contains(&StandardRecord {
        name: "ICRC-1".to_string(),
        url: "https://github.com/dfinity/ICRC-1".to_string(),
    }));
    assert!(standards.contains(&StandardRecord {
        name: "IS20".to_string(),
        url: "https://github.com/infinity-swap/is20".to_string(),
    }));
}

#[test]
fn metadata() {
    let (_, canister, _) = init();

    let metadata = canister.icrc1_metadata();
    assert!(metadata.contains(&("icrc1:symbol".to_string(), Value::Text("TST".to_string()))));
    assert!(metadata.contains(&("icrc1:name".to_string(), Value::Text("Testo".to_string()))));
    assert!(metadata.contains(&("icrc1:decimals".to_string(), Value::Nat(11.into()))));
    assert!(metadata.contains(&("icrc1:fee".to_string(), Value::Nat(127.into()))));
}

fn transfer(canister: &TokenCanister, to: Principal, amount: u128) {
    canister
        .icrc1_transfer(TransferArgs {
            from_subaccount: None,
            to: Account::new(to, None),
            amount: amount.into(),
            fee: None,
            memo: None,
            created_at_time: None,
        })
        .unwrap();
}

#[test]
fn normal_transfer() {
    let (meta, canister, ctx) = init();

    // This transfer is actually mint transfer which we don't want to test here, so we skip it.
    ctx.update_caller(alice());
    transfer(&canister, bob(), 10_000);

    ctx.update_caller(bob());
    transfer(&canister, john(), 5_000);

    assert_eq!(
        canister.icrc1_balance_of(Account::new(bob(), None)),
        (Tokens128::from(5_000) - meta.fee).unwrap()
    );
    assert_eq!(
        canister.icrc1_balance_of(Account::new(john(), None)),
        Tokens128::from(5_000)
    );
}

#[test]
fn bad_fee_transfer() {
    let (meta, canister, ctx) = init();

    // This transfer is actually mint transfer which we don't want to test here, so we skip it.
    ctx.update_caller(alice());
    transfer(&canister, bob(), 10_000);

    ctx.update_caller(bob());
    let result = canister.icrc1_transfer(TransferArgs {
        from_subaccount: None,
        to: Account::new(john(), None),
        amount: 1000.into(),
        fee: Some(126.into()),
        memo: None,
        created_at_time: None,
    });

    assert_eq!(
        result,
        Err(TransferError::BadFee {
            expected_fee: meta.fee
        })
    );
}

#[test]
fn too_old_transfer() {
    let (_, canister, ctx) = init();

    // This transfer is actually mint transfer which we don't want to test here, so we skip it.
    ctx.update_caller(alice());
    transfer(&canister, bob(), 10_000);

    let curr_ts = canister_sdk::ic_kit::ic::time();
    ctx.update_caller(bob());
    let result = canister.icrc1_transfer(TransferArgs {
        from_subaccount: None,
        to: Account::new(john(), None),
        amount: 1000.into(),
        fee: None,
        memo: None,
        created_at_time: Some(curr_ts - 10 * 60 * 1_000_000_000),
    });

    assert_eq!(result, Err(TransferError::TooOld))
}

#[test]
fn created_in_future() {
    let (_, canister, ctx) = init();

    // This transfer is actually mint transfer which we don't want to test here, so we skip it.
    ctx.update_caller(alice());
    transfer(&canister, bob(), 10_000);

    let curr_ts = canister_sdk::ic_kit::ic::time();
    ctx.update_caller(bob());
    let result = canister.icrc1_transfer(TransferArgs {
        from_subaccount: None,
        to: Account::new(john(), None),
        amount: 1000.into(),
        fee: None,
        memo: None,
        created_at_time: Some(curr_ts + 3 * 60 * 1_000_000_000),
    });

    assert_eq!(
        result,
        Err(TransferError::CreatedInFuture {
            ledger_time: curr_ts
        })
    )
}

#[test]
fn duplicate_check() {
    let (_, canister, ctx) = init();

    // This transfer is actually mint transfer which we don't want to test here, so we skip it.
    ctx.update_caller(alice());
    transfer(&canister, bob(), 10_000);

    let curr_ts = canister_sdk::ic_kit::ic::time();
    ctx.update_caller(bob());
    let tx_id = canister
        .icrc1_transfer(TransferArgs {
            from_subaccount: None,
            to: Account::new(john(), None),
            amount: 1000.into(),
            fee: None,
            memo: None,
            created_at_time: Some(curr_ts),
        })
        .unwrap();

    let result = canister.icrc1_transfer(TransferArgs {
        from_subaccount: None,
        to: Account::new(john(), None),
        amount: 1000.into(),
        fee: None,
        memo: None,
        created_at_time: Some(curr_ts),
    });

    assert_eq!(
        result,
        Err(TransferError::Duplicate {
            duplicate_of: tx_id
        })
    );
}

#[test]
fn mint_and_burn_transfers() {
    let (_, canister, ctx) = init();

    let original_balance = canister.icrc1_balance_of(Account::new(alice(), None));

    // This transfer is actually mint transfer which we don't want to test here, so we skip it.
    ctx.update_caller(alice());
    transfer(&canister, bob(), 10_000);

    assert_eq!(
        canister.icrc1_balance_of(Account::new(alice(), None)),
        original_balance,
    );

    ctx.update_caller(bob());
    transfer(&canister, alice(), 5_000);

    assert_eq!(
        canister.icrc1_balance_of(Account::new(alice(), None)),
        original_balance,
    );
    assert_eq!(
        canister.icrc1_balance_of(Account::new(bob(), None)),
        Tokens128::from(5000),
    );
}
