use crate::ledger::history::History;
use crate::types::TxRecord;
use candid::{CandidType, Deserialize, Nat, Principal};
use ic_certified_map::HashTree;

mod history;

#[derive(Default, CandidType, Deserialize)]
pub struct Ledger {
    history: History,
    next_id: u128,
}

impl Ledger {
    pub fn len(&self) -> Nat {
        Nat::from(self.next_id)
    }

    fn next_id(&self) -> Nat {
        Nat::from(self.next_id)
    }

    pub fn get(&self, id: &Nat) -> Option<TxRecord> {
        self.history.get(id)
    }

    pub fn get_range(&self, start: &Nat, limit: &Nat) -> Vec<TxRecord> {
        self.history.get_range(start, limit)
    }

    pub fn iter(&self) -> impl Iterator<Item = TxRecord> + '_ {
        self.history.iter()
    }

    pub fn transfer(&mut self, from: Principal, to: Principal, amount: Nat, fee: Nat) -> Nat {
        let id = self.next_id();
        self.push(TxRecord::transfer(id.clone(), from, to, amount, fee));

        id
    }

    pub fn transfer_from(
        &mut self,
        caller: Principal,
        from: Principal,
        to: Principal,
        amount: Nat,
        fee: Nat,
    ) -> Nat {
        let id = self.next_id();
        self.push(TxRecord::transfer_from(
            id.clone(),
            caller,
            from,
            to,
            amount,
            fee,
        ));

        id
    }

    pub fn approve(&mut self, from: Principal, to: Principal, amount: Nat, fee: Nat) -> Nat {
        let id = self.next_id();
        self.push(TxRecord::approve(id.clone(), from, to, amount, fee));

        id
    }

    pub fn mint(&mut self, from: Principal, to: Principal, amount: Nat) -> Nat {
        let id = self.len();
        self.push(TxRecord::mint(id.clone(), from, to, amount));

        id
    }

    pub fn burn(&mut self, caller: Principal, amount: Nat) -> Nat {
        let id = self.next_id();
        self.push(TxRecord::burn(id.clone(), caller, amount));

        id
    }

    pub fn auction(&mut self, to: Principal, amount: Nat) {
        let id = self.next_id();
        self.push(TxRecord::auction(id, to, amount))
    }

    fn push(&mut self, record: TxRecord) {
        self.history.insert(record);
        self.history.sign();
        self.next_id += 1;
    }

    pub fn get_witness(&self, id: &Nat) -> Option<HashTree> {
        self.history.get_witness(id)
    }
}
