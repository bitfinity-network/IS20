use crate::types::TxRecord;
use candid::types::{Compound, Serializer, Type};
use candid::Principal;
use ic_cdk::export::candid::{CandidType, Deserialize, Nat};
use ic_certified_map::{HashTree, RbTree};
use serde::de::{SeqAccess, Visitor};
use serde::Deserializer;
use std::fmt;
use std::fmt::Formatter;

#[cfg(target_arch = "wasm32")]
use ic_certified_map::AsHashTree;

const MAX_TREE_SIZE: usize = 100_000;

#[derive(Default, Clone)]
pub struct History {
    tree: RbTree<Vec<u8>, Vec<u8>>,
    tree_size: usize,
}

impl History {
    pub fn insert(&mut self, tx: TxRecord) {
        self.tree
            .insert(get_key_bytes(&tx.index), get_tx_bytes(&tx));
        self.tree_size += 1;
        if self.tree_size > MAX_TREE_SIZE {
            self.remove_oldest_tx();
        }
    }

    fn remove_oldest_tx(&mut self) {
        let key = self.tree.iter().map(|(key, _)| key).next().unwrap().clone();
        self.tree.delete(&key);
        self.tree_size -= 1;
    }

    pub fn get(&self, id: &Nat) -> Option<TxRecord> {
        self.tree.get(&get_key_bytes(id)).map(|v| tx_from_bytes(v))
    }

    pub fn get_range(&self, start: &Nat, limit: &Nat) -> Vec<TxRecord> {
        fn extract_values(hash_tree: &HashTree, aggr: &mut Vec<TxRecord>) {
            match hash_tree {
                HashTree::Fork(boxed) => {
                    extract_values(&boxed.as_ref().0, aggr);
                    extract_values(&boxed.as_ref().1, aggr);
                }
                HashTree::Leaf(v) => {
                    aggr.push(tx_from_bytes(&v.to_vec()));
                }
                HashTree::Labeled(_, child) => extract_values(child, aggr),
                _ => {}
            }
        }

        let witness = self.tree.value_range(
            &get_key_bytes(&start.clone()),
            &get_key_bytes(&(start.clone() + limit.clone() - 1)),
        );
        let mut result = vec![];
        extract_values(&witness, &mut result);

        result
    }

    pub fn iter(&self) -> impl Iterator<Item = TxRecord> + '_ {
        self.tree.iter().map(|(_, value)| tx_from_bytes(value))
    }

    pub fn sign(&self) {
        #[cfg(target_arch = "wasm32")]
        ic_cdk::api::set_certified_data(&self.tree.root_hash());
    }

    pub fn get_witness(&self, id: &Nat) -> Option<HashTree> {
        if self.tree.get(&get_key_bytes(id)).is_some() {
            Some(self.tree.witness(&get_key_bytes(id)))
        } else {
            None
        }
    }
}

fn get_key_bytes(key: &Nat) -> Vec<u8> {
    key.0.to_bytes_be()
}

fn get_tx_bytes(tx: &TxRecord) -> Vec<u8> {
    use ic_cdk::export::candid::Encode;
    Encode!(tx).unwrap()
}

fn tx_from_bytes(bytes: &[u8]) -> TxRecord {
    use ic_cdk::export::candid::Decode;
    Decode!(bytes, TxRecord).unwrap()
}

impl CandidType for History {
    fn _ty() -> Type {
        Type::Vec(Box::new(TxRecord::ty()))
    }

    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: Serializer,
    {
        let mut vec = serializer.serialize_vec(self.tree_size)?;
        for record in self.iter() {
            vec.serialize_element(&record)?;
        }

        Ok(())
    }
}

impl<'de> Deserialize<'de> for History {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VecVisitor {}

        impl<'de> Visitor<'de> for VecVisitor {
            type Value = History;

            fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
                write!(formatter, "vector of elements")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut tree = RbTree::new();
                let mut tree_size = 0;
                while let Some(v) = seq.next_element::<TxRecord>()? {
                    tree.insert(get_key_bytes(&v.index), get_tx_bytes(&v));
                    tree_size += 1;
                }

                Ok(History { tree, tree_size })
            }
        }

        deserializer.deserialize_seq(VecVisitor {})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Operation;
    use ic_certified_map::AsHashTree;
    use ic_kit::MockContext;

    #[test]
    fn serialize_and_deserialize_history() {
        MockContext::new().inject();
        let mut history = History::default();
        history.insert(TxRecord::mint(
            Nat::from(0u8),
            Principal::anonymous(),
            Principal::management_canister(),
            Nat::from(100500u64),
        ));
        history.insert(TxRecord::burn(
            Nat::from(1u8),
            Principal::anonymous(),
            Principal::anonymous(),
            Nat::from(1234u64),
        ));

        let serialized = ic_cdk::export::candid::encode_args((history.clone(),))
            .expect("Failed to serialize history.");
        let (deserialized,) = ic_cdk::export::candid::decode_args::<(History,)>(&serialized)
            .expect("Failed to deserialize history.");

        assert_eq!(deserialized.tree.root_hash(), history.tree.root_hash());
        assert_eq!(deserialized.tree_size, history.tree_size);
    }

    #[ignore]
    #[test]
    fn deserialize_large_history() {
        MockContext::new().inject();
        let mut history = History::default();
        const COUNT: usize = 50000;

        let mut vector_history = vec![];

        let start = std::time::SystemTime::now();
        for i in 0..COUNT {
            history.insert(TxRecord::mint(
                Nat::from(i),
                Principal::anonymous(),
                Principal::management_canister(),
                Nat::from(100500u64),
            ));
            vector_history.push(TxRecord::mint(
                Nat::from(i),
                Principal::anonymous(),
                Principal::management_canister(),
                Nat::from(100500u64),
            ));
        }

        println!(
            "History of length {COUNT} generated in {} milliseconds",
            start.elapsed().unwrap().as_millis()
        );

        let start = std::time::SystemTime::now();
        let serialized = ic_cdk::export::candid::encode_args((history.clone(),))
            .expect("Failed to serialize history.");
        println!(
            "Hash tree history serialized in {} milliseconds",
            start.elapsed().unwrap().as_millis()
        );

        let start = std::time::SystemTime::now();
        let (_,) = ic_cdk::export::candid::decode_args::<(History,)>(&serialized)
            .expect("Failed to deserialize history.");
        println!(
            "Hash tree history deserialized in {} milliseconds",
            start.elapsed().unwrap().as_millis()
        );

        let start = std::time::SystemTime::now();
        let serialized = ic_cdk::export::candid::encode_args((vector_history.clone(),))
            .expect("Failed to serialize history.");
        println!(
            "Vector history serialized in {} milliseconds",
            start.elapsed().unwrap().as_millis()
        );

        let start = std::time::SystemTime::now();
        let (_,) = ic_cdk::export::candid::decode_args::<(Vec<TxRecord>,)>(&serialized)
            .expect("Failed to deserialize history.");
        println!(
            "Vector history deserialized in {} milliseconds",
            start.elapsed().unwrap().as_millis()
        );
    }

    #[test]
    fn get_tx() {
        MockContext::new().inject();
        let mut history = History::default();
        history.insert(TxRecord::mint(
            Nat::from(0u64),
            Principal::anonymous(),
            Principal::management_canister(),
            Nat::from(100500u64),
        ));
        history.insert(TxRecord::burn(
            Nat::from(1u64),
            Principal::anonymous(),
            Principal::anonymous(),
            Nat::from(1234u64),
        ));

        assert_eq!(
            history.get(&Nat::from(0u64)).unwrap().operation,
            Operation::Mint
        );
        assert_eq!(
            history.get(&Nat::from(1u64)).unwrap().operation,
            Operation::Burn
        );
    }

    #[test]
    fn get_tx_range() {
        MockContext::new().inject();
        let mut history = History::default();
        const COUNT: usize = 40;

        for i in 0..COUNT {
            history.insert(TxRecord::mint(
                Nat::from(i),
                Principal::anonymous(),
                Principal::management_canister(),
                Nat::from(100500u64),
            ));
        }

        let range = history.get_range(&Nat::from(10u64), &Nat::from(20u64));
        assert_eq!(range.len(), 20);

        for i in 0..20 {
            assert_eq!(range[i].index, Nat::from(10 + i));
        }
    }

    #[test]
    fn remove_oldest_tx() {
        MockContext::new().inject();
        let mut history = History::default();
        const COUNT: usize = 20;

        for i in 0..COUNT {
            history.insert(TxRecord::mint(
                Nat::from(i * 2),
                Principal::anonymous(),
                Principal::management_canister(),
                Nat::from(100500u64),
            ));
        }

        for i in 0..COUNT {
            history.insert(TxRecord::mint(
                Nat::from(i * 2 + 1),
                Principal::anonymous(),
                Principal::management_canister(),
                Nat::from(100500u64),
            ));
        }

        assert_eq!(history.tree_size, 40);
        assert!(history.get(&Nat::from(0u64)).is_some());

        history.remove_oldest_tx();

        assert_eq!(history.tree_size, 39);
        assert!(history.get(&Nat::from(0u64)).is_none());
        assert!(history.get(&Nat::from(1u64)).is_some());
    }
}
