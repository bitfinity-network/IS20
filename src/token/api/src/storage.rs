use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{btreemap, cell, log, DefaultMemoryImpl};

pub type Memory = VirtualMemory<DefaultMemoryImpl>;

pub type StableCell<T> = cell::Cell<T, Memory>;
pub type StableBTreeMap<K, V> = btreemap::BTreeMap<K, V, Memory>;
pub type StableLog = log::Log<Memory, Memory>;

thread_local! {
    // The memory manager is used for simulating multiple memories. Given a `MemoryId` it can
    // return a memory that can be used by stable structures.
    static MEMORY_MANAGER: MemoryManager<DefaultMemoryImpl> =
        MemoryManager::init(DefaultMemoryImpl::default());
}

// Return memory by `MemoryId`.
// Each instance of stable structures must have unique `MemoryId`;
pub fn get_memory_by_id(id: MemoryId) -> Memory {
    MEMORY_MANAGER.with(|mng| mng.get(id))
}
