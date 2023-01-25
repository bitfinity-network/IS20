use std::borrow::Cow;
use std::cell::RefCell;

use candid::{CandidType, Decode, Encode, Principal};
use ic_stable_structures::{BoundedStorable, MemoryId, StableBTreeMap, StableCell, Storable};
use serde::Deserialize;

#[derive(CandidType, Deserialize, Default, Debug)]
pub struct State {}

impl State {
    pub fn reset(&mut self) {
        TOKENS_MAP.with(|map| map.borrow_mut().clear());
        WASM_CELL.with(|cell| {
            cell.borrow_mut()
                .set(StorableWasm::default())
                .expect("failed to reset token wasm in stable memory")
        });
    }

    pub fn get_token(&self, name: String) -> Option<Principal> {
        Self::check_name(&name).then_some(())?;

        TOKENS_MAP
            .with(|map| map.borrow().get(&StringKey(name)))
            .map(|principal| principal.0)
    }

    pub fn remove_token(&self, name: String) -> Option<Principal> {
        Self::check_name(&name).then_some(())?;

        TOKENS_MAP
            .with(|map| map.borrow_mut().remove(&StringKey(name)))
            .map(|principal| principal.0)
    }

    pub fn insert_token(&mut self, name: String, principal: Principal) {
        TOKENS_MAP.with(|map| {
            map.borrow_mut()
                .insert(StringKey(name), PrincipalValue(principal))
        });
    }

    pub fn get_token_wasm(&self) -> Option<Vec<u8>> {
        WASM_CELL.with(|cell| cell.borrow().get().0.clone())
    }

    pub fn set_token_wasm(&mut self, wasm: Option<Vec<u8>>) {
        WASM_CELL.with(|cell| {
            cell.borrow_mut()
                .set(StorableWasm(wasm))
                .expect("failed to set token canister wasm to stable storage");
        });
    }

    fn check_name(name: &str) -> bool {
        name.as_bytes().len() <= MAX_TOKEN_LEN_IN_BYTES
    }
}

#[derive(Default, Deserialize, CandidType)]
struct StorableWasm(Option<Vec<u8>>);

impl Storable for StorableWasm {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Encode!(self)
            .expect("failed to encode StorableWasm for stable storage")
            .into()
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        Decode!(&bytes, Self).expect("failed to decode StorableWasm from stable storage")
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct StringKey(String);

impl Storable for StringKey {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        self.0.as_bytes().into()
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        StringKey(String::from_bytes(bytes))
    }
}

pub const MAX_TOKEN_LEN_IN_BYTES: usize = 1024;

impl BoundedStorable for StringKey {
    const MAX_SIZE: u32 = MAX_TOKEN_LEN_IN_BYTES as _;

    const IS_FIXED_SIZE: bool = false;
}

struct PrincipalValue(Principal);

impl Storable for PrincipalValue {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        self.0.as_slice().into()
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        PrincipalValue(Principal::from_slice(&bytes))
    }
}

impl BoundedStorable for PrincipalValue {
    const MAX_SIZE: u32 = 29;
    const IS_FIXED_SIZE: bool = false;
}

// starts with 10 because 0..10 reserved for `ic-factory` state.
const WASM_MEMORY_ID: MemoryId = MemoryId::new(10);
const TOKENS_MEMORY_ID: MemoryId = MemoryId::new(11);

thread_local! {
    static WASM_CELL: RefCell<StableCell<StorableWasm>> = {
            RefCell::new(StableCell::new(WASM_MEMORY_ID, StorableWasm::default())
                .expect("failed to initialize wasm stable storage"))
    };

    static TOKENS_MAP: RefCell<StableBTreeMap<StringKey, PrincipalValue>> =
        RefCell::new(StableBTreeMap::new(TOKENS_MEMORY_ID));
}

pub fn get_state() -> State {
    State::default()
}

#[cfg(test)]
mod tests {
    use candid::Principal;
    use canister_sdk::ic_kit::MockContext;
    use ic_stable_structures::Storable;

    use crate::state::{PrincipalValue, StorableWasm};
    use crate::State;

    use super::StringKey;

    #[test]
    fn string_key_serialization() {
        let key = StringKey("".into());
        let deserialized = StringKey::from_bytes(key.to_bytes());
        assert_eq!(key.0, deserialized.0);

        let key = StringKey("TEST_KEY".into());
        let deserialized = StringKey::from_bytes(key.to_bytes());
        assert_eq!(key.0, deserialized.0);

        let long_key = StringKey(String::from_iter(std::iter::once('c').cycle().take(512)));
        let deserialized = StringKey::from_bytes(long_key.to_bytes());
        assert_eq!(long_key.0, deserialized.0);
    }

    #[test]
    fn principal_value_serialization() {
        let val = PrincipalValue(Principal::anonymous());
        let deserialized = PrincipalValue::from_bytes(val.to_bytes());
        assert_eq!(val.0, deserialized.0);

        let val = PrincipalValue(Principal::management_canister());
        let deserialized = PrincipalValue::from_bytes(val.to_bytes());
        assert_eq!(val.0, deserialized.0);
    }

    #[test]
    fn storable_wasm_serialization() {
        let val = StorableWasm(None);
        let deserialized = StorableWasm::from_bytes(val.to_bytes());
        assert_eq!(val.0, deserialized.0);

        let val = StorableWasm(Some(vec![]));
        let deserialized = StorableWasm::from_bytes(val.to_bytes());
        assert_eq!(val.0, deserialized.0);

        let val = StorableWasm(Some((1..255).collect()));
        let deserialized = StorableWasm::from_bytes(val.to_bytes());
        assert_eq!(val.0, deserialized.0);
    }

    fn init_state() -> State {
        MockContext::new().inject();
        let mut state = State::default();
        state.reset();
        state
    }

    #[test]
    fn insert_get_remove_tokens() {
        let mut state = init_state();

        state.insert_token("anon".into(), Principal::anonymous());
        state.insert_token("mng".into(), Principal::management_canister());

        assert_eq!(state.get_token("anon".into()), Some(Principal::anonymous()));
        assert_eq!(
            state.get_token("mng".into()),
            Some(Principal::management_canister())
        );
        assert_eq!(state.get_token("other".into()), None);

        assert_eq!(
            state.remove_token("mng".into()),
            Some(Principal::management_canister())
        );
        assert_eq!(state.get_token("anon".into()), Some(Principal::anonymous()));
        assert_eq!(state.get_token("mng".into()), None);
    }

    #[test]
    fn set_get_token_wasm() {
        let mut state = init_state();

        state.set_token_wasm(None);
        assert_eq!(state.get_token_wasm(), None);

        state.set_token_wasm(Some(vec![]));
        assert_eq!(state.get_token_wasm(), Some(vec![]));

        state.set_token_wasm(Some(vec![123; 2048]));
        assert_eq!(state.get_token_wasm(), Some(vec![123; 2048]));
    }
}
