use factory::State;
use ic_storage::IcStorage;

#[tokio::test]
async fn test_set_token_bytecode_impl() {
    assert_eq!(State::get().borrow().token_wasm, None);

    factory::api::set_token_bytecode_impl(vec![12, 3]).await;

    assert_eq!(State::get().borrow().token_wasm, Some(vec![12, 3]));
}
