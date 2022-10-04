use std::{borrow::Cow, cell::RefCell};

use candid::Principal;
use canister_sdk::ic_helpers::tokens::Tokens128;
use ic_stable_structures::{memory_manager::MemoryId, Storable};

use crate::{
    account::{Subaccount, DEFAULT_SUBACCOUNT},
    storage::{self, StableBTreeMap},
};

/// Store balances in stable memory.
pub struct Balances;

impl Balances {
    /// Write or re-write amount of tokens for specified account to stable memory.
    pub fn insert(principal: Principal, subaccount: Option<Subaccount>, token: Tokens128) {
        let subaccount = subaccount.unwrap_or(DEFAULT_SUBACCOUNT);
        let key = Key {
            principal,
            subaccount,
        };

        MAP.with(|map| {
            map.borrow_mut()
                .insert(key, token.amount)
                .expect("balance insert failed") // key and value bytes len always less then MAX size
        });
    }

    /// Get amount of tokens for the specified account from stable memory.
    pub fn get(principal: Principal, subaccount: Option<Subaccount>) -> Option<Tokens128> {
        let subaccount = subaccount.unwrap_or(DEFAULT_SUBACCOUNT);
        let key = Key {
            principal,
            subaccount,
        };

        let amount = MAP.with(|map| map.borrow_mut().get(&key));
        amount.map(Tokens128::from)
    }

    /// Remove specified account balance from the stable memory.
    pub fn remove(principal: Principal, subaccount: Option<Subaccount>) -> Option<Tokens128> {
        let subaccount = subaccount.unwrap_or(DEFAULT_SUBACCOUNT);
        let key = Key {
            principal,
            subaccount,
        };

        let amount = MAP.with(|map| map.borrow_mut().remove(&key));
        amount.map(Tokens128::from)
    }
}

const BALANCES_MEMORY_ID: MemoryId = MemoryId::new(1);

struct Key {
    pub principal: Principal,
    pub subaccount: Subaccount,
}

const SUBACCOUNT_LEN: usize = 32;
const SUBACCOUNT_OFFSET: usize = 32;

const KEY_BYTES_LEN: usize = 64;
const VALUE_BYTES_LEN: usize = 16;

impl Storable for Key {
    /// Memory layout:
    /// | principal len (1 byte) | principal data (31 byte) | subaccount (32 bytes) |
    /// Principal data maximum len is 29;
    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buffer = Vec::with_capacity(KEY_BYTES_LEN);
        let principal_bytes = self.principal.as_slice();

        buffer[0] = principal_bytes.len() as u8;
        buffer[1..principal_bytes.len() + 1].copy_from_slice(principal_bytes);
        buffer[SUBACCOUNT_OFFSET..].copy_from_slice(&self.subaccount);

        Cow::Owned(buffer)
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        let principal_len = bytes[0] as usize;
        let account = Principal::from_slice(&bytes[1..principal_len + 1]);
        let mut subaccount = [0u8; SUBACCOUNT_LEN];
        subaccount.copy_from_slice(&bytes[SUBACCOUNT_OFFSET..]);
        Self {
            principal: account,
            subaccount,
        }
    }
}

thread_local! {
    static MAP: RefCell<StableBTreeMap<Key, u128>> = {
            let memory = storage::get_memory_by_id(BALANCES_MEMORY_ID);
            let map = StableBTreeMap::init(memory, KEY_BYTES_LEN as u32, VALUE_BYTES_LEN as u32);
            RefCell::new(map)
    }
}
