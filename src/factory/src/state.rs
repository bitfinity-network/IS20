use std::cell::RefCell;

use candid::{CandidType, Decode, Encode, Principal};
use ic_stable_structures::{BoundedStorable, MemoryId, StableBTreeMap, StableCell, Storable};
use serde::Deserialize;

#[derive(CandidType, Deserialize, Default, Debug)]
pub struct State {}

impl State {
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

    pub fn insert_token(&self, name: String, principal: Principal) {
        TOKENS_MAP.with(|map| {
            map.borrow_mut()
                .insert(StringKey(name), PrincipalValue(principal))
                .expect("failed to insert token canister to stable storage");
        });
    }

    pub fn get_token_wasm(&self) -> Option<Vec<u8>> {
        WASM_CELL.with(|cell| cell.borrow().get().0.clone())
    }

    pub fn set_token_wasm(&self, wasm: Option<Vec<u8>>) {
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
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Encode!(self)
            .expect("failed to encode StorableWasm for stable storage")
            .into()
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        Decode!(&bytes, Self).expect("failed to decode StorableWasm from stable storage")
    }
}

struct StringKey(String);

impl Storable for StringKey {
    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        self.0.as_bytes().into()
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        StringKey(String::from_bytes(bytes))
    }
}

pub const MAX_TOKEN_LEN_IN_BYTES: usize = 1024;

impl BoundedStorable for StringKey {
    fn max_size() -> u32 {
        MAX_TOKEN_LEN_IN_BYTES as _
    }
}

struct PrincipalValue(Principal);

impl Storable for PrincipalValue {
    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        self.0.as_slice().into()
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        PrincipalValue(Principal::from_slice(&bytes))
    }
}

impl BoundedStorable for PrincipalValue {
    fn max_size() -> u32 {
        // max bytes count in Principal
        29
    }
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
