use crate::types::TxRecord;
use candid::{CandidType, Deserialize, Nat, Principal};

#[derive(Default, CandidType, Deserialize)]
pub struct Ledger(pub Vec<TxRecord>);

impl Ledger {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn transfer(&mut self, from: Principal, to: Principal, amount: Nat, fee: Nat) -> Nat {
        let id = Nat::from(self.len());
        self.0
            .push(TxRecord::transfer(id.clone(), from, to, amount, fee));

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
        let id = Nat::from(self.len());
        self.0.push(TxRecord::transfer_from(
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
        let id = Nat::from(self.len());
        self.0
            .push(TxRecord::approve(id.clone(), from, to, amount, fee));

        id
    }

    pub fn mint(&mut self, from: Principal, to: Principal, amount: Nat) -> Nat {
        let id = Nat::from(self.len());
        self.0.push(TxRecord::mint(id.clone(), from, to, amount));

        id
    }

    pub fn burn(&mut self, caller: Principal, amount: Nat) -> Nat {
        let id = Nat::from(self.len());
        self.0.push(TxRecord::burn(id.clone(), caller, amount));

        id
    }
}
