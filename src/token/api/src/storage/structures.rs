use std::collections::{hash_map::Entry, HashMap};

use candid::Principal;
use canister_sdk::ic_kit::ic;
use ic_stable_structures::{btreemap, cell, log, memory_manager::MemoryId, Storable};

use super::Memory;

pub struct StableCell<T: Storable> {
    data: HashMap<Principal, cell::Cell<T, Memory>>,
    default_value: T,
    memory_id: MemoryId,
}

impl<T: Storable> StableCell<T> {
    pub fn new(memory_id: MemoryId, default_value: T) -> Self {
        Self {
            data: HashMap::default(),
            default_value,
            memory_id,
        }
    }

    pub fn get(&self) -> &T {
        let canister_id = ic::id();
        self.data
            .get(&canister_id)
            .map(|cell| cell.get())
            .unwrap_or(&self.default_value)
    }

    /// Updates value in stable memory.
    pub fn set(&mut self, value: T) {
        let canister_id = ic::id();
        match self.data.entry(canister_id) {
            Entry::Occupied(mut entry) => {
                entry
                    .get_mut()
                    .set(value)
                    .expect("failed to set value to stable cell");
            }
            Entry::Vacant(entry) => {
                let memory = super::get_memory_by_id(self.memory_id);
                entry.insert(cell::Cell::init(memory, value).expect("failed to init stable cell"));
            }
        };
    }
}

// pub struct StableBTreeMap<K: Storable, V: Storable> {
//     data: HashMap<Principal, btreemap::BTreeMap<Memory, K, V>>,
//     memory_id: MemoryId,
// }

// impl<K: Storable + Clone, V: Storable + Clone> StableBTreeMap<K, V> {
//     fn get(&mut self) -> T {
//         let canister_id = ic::id();
//         self.data
//             .entry(canister_id)
//             .or_insert_with(|| {
//                 let memory = super::get_memory_by_id(self.memory_id);
//                 cell::Cell::init(memory, self.default.clone()).expect("cell initialization error")
//             })
//             .get()
//             .clone()
//     }

//     fn set(&mut self, value: T) {
//         let canister_id = ic::id();
//         self.data
//             .entry(canister_id)
//             .or_insert_with(|| {
//                 let memory = super::get_memory_by_id(self.memory_id);
//                 cell::Cell::init(memory, self.default.clone()).expect("cell initialization error")
//             })
//             .set(value)
//             .expect("failed to set value to stable cell");
//     }
// }

pub type StableBTreeMap<K, V> = btreemap::BTreeMap<Memory, K, V>;
pub type StableLog = log::Log<Memory, Memory>;
