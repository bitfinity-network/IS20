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

    pub fn to_vec(&self) -> Vec<TxRecord> {
        self.history.iter().collect::<Vec<TxRecord>>()
    }

    pub fn get_len_user_history(&self, user: Principal) -> Nat {
        self.history
            .iter()
            .filter(|tx| tx.to == user || tx.from == user || tx.caller == Some(user))
            .count()
            .into()
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

    pub fn burn(&mut self, caller: Principal, from: Principal, amount: Nat) -> Nat {
        let id = self.next_id();
        self.push(TxRecord::burn(id.clone(), caller, from, amount));

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
