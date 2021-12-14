use crate::types::TxRecord;
use candid::{CandidType, Deserialize, Nat};

#[derive(Default, CandidType, Deserialize)]
pub struct Ledger(Vec<TxRecord>);

impl Ledger {
    pub fn push(&mut self, entry: TxRecord) -> Nat {
        let idx = self.0.len();
        self.0.push(entry);

        Nat::from(idx)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}
