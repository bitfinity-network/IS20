use crate::state::STABLE_MAP;
use crate::types::{PaginatedResult, PendingNotifications, TxRecord, TxRecordStable};
use candid::{CandidType, Deserialize, Nat, Principal};
use num_traits::ToPrimitive;
use stable_structures::{stable_storage::StableStorage, RestrictedMemory};

const MAX_HISTORY_LENGTH: usize = 1_000_000;
const HISTORY_REMOVAL_BATCH_SIZE: usize = 10_000;
const LEDGER_HEAD_MAGIC: &[u8; 3] = b"LER";
const LEDGER_HEAD_LAYOUT_VERSION: u8 = 1;

#[derive(Debug, Default, CandidType, Deserialize)]
pub struct Ledger {
    history: TxRecordStable,
    vec_offset: Nat,
    pub notifications: PendingNotifications,
}

impl Ledger {
    pub fn len(&self) -> Nat {
        self.vec_offset.clone() + self.history.len()
    }

    fn next_id(&self) -> Nat {
        self.vec_offset.clone() + self.history.len()
    }

    pub fn get(&self, id: &Nat) -> Option<TxRecord> {
        self.history.get(self.get_index(id)?)
    }

    pub fn get_transactions(
        &self,
        who: Option<Principal>,
        count: u32,
        transaction_id: Option<u128>,
    ) -> PaginatedResult {
        let count = count as usize;
        let mut buf = vec![];
        STABLE_MAP.with(|s| {
            let map = s.borrow();
            for (k, _) in self.history.index.range(None, None, &map) {
                let key = self.history.index.key_decode::<u64>(&k) as usize;
                buf.push(self.history.get(key).unwrap());
            }
        });

        let mut transactions = buf
            .iter()
            .rev()
            .filter(|tx| who.map_or(true, |c| c == tx.from || c == tx.to || Some(c) == tx.caller))
            .filter(|tx| transaction_id.map_or(true, |id| id >= tx.index))
            .take(count + 1)
            .cloned()
            .collect::<Vec<_>>();

        let next_id = if transactions.len() == count + 1 {
            Some(transactions.remove(count).index.0.to_u128().unwrap())
        } else {
            None
        };

        PaginatedResult {
            result: transactions,
            next: next_id,
        }
    }

    // pub fn iter(&self) -> impl DoubleEndedIterator<Item = &TxRecord> {
    //     self.history.iter()
    // }

    fn get_index(&self, id: &Nat) -> Option<usize> {
        if *id < self.vec_offset {
            None
        } else {
            let index = id.clone() - self.vec_offset.clone();
            index.0.to_usize()
        }
    }

    pub fn get_len_user_history(&self, user: Principal) -> Nat {
        let mut buf = vec![];
        STABLE_MAP.with(|s| {
            let map = s.borrow();
            for (k, _) in self.history.index.range(None, None, &map) {
                let key = self.history.index.key_decode::<u64>(&k) as usize;
                buf.push(self.history.get(key).unwrap());
            }
        });

        buf.iter()
            .filter(|tx| tx.to == user || tx.from == user || tx.caller == Some(user))
            .count()
            .into()
    }

    pub fn transfer(&mut self, from: Principal, to: Principal, amount: Nat, fee: Nat) -> Nat {
        let id = self.next_id();
        self.push(TxRecord::transfer(id.clone(), from, to, amount, fee));

        id
    }

    pub fn batch_transfer(
        &mut self,
        from: Principal,
        transfers: Vec<(Principal, Nat)>,
        fee: Nat,
    ) -> Vec<Nat> {
        transfers
            .into_iter()
            .map(|(to, amount)| self.transfer(from, to, amount, fee.clone()))
            .collect()
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

    pub fn save_header(&self, memory: &RestrictedMemory<StableStorage>) {
        memory.write_struct::<LedgerHeader>(&LedgerHeader::from(self), 0);
    }

    pub fn load_header(&mut self, memory: &RestrictedMemory<StableStorage>) {
        let header: LedgerHeader = memory.read_struct(0);
        assert_eq!(&header.magic, LEDGER_HEAD_MAGIC, "Bad magic.");
        assert_eq!(
            header.version, LEDGER_HEAD_LAYOUT_VERSION,
            "Unsupported version."
        );
        self.vec_offset = header.vec_offset;
    }

    fn push(&mut self, record: TxRecord) {
        self.history.push(record.clone());
        self.notifications.insert(record.index, None);

        if self.len() > MAX_HISTORY_LENGTH + HISTORY_REMOVAL_BATCH_SIZE {
            // We remove first `HISTORY_REMOVAL_BATCH_SIZE` from the history at one go, to prevent
            // often relocation of the history vec.
            // This removal code can later be changed to moving old history records into another
            // storage.
            let mut buf = vec![];
            let mut keys = vec![];
            STABLE_MAP.with(|s| {
                let map = s.borrow();
                for (i, (k, _)) in self.history.index.range(None, None, &map).enumerate() {
                    if i >= HISTORY_REMOVAL_BATCH_SIZE {
                        break;
                    };
                    let key = self.history.index.key_decode::<u64>(&k) as usize;
                    keys.push(key);
                    buf.push(self.history.get(key).unwrap());
                }
            });

            for record in buf.iter() {
                self.notifications.remove(&record.index.clone());
            }
            for key in keys.iter() {
                self.history.remove(*key);
            }
            // todo add the vec_offset to id when find the TxRecordStable.
            self.vec_offset += HISTORY_REMOVAL_BATCH_SIZE;
        }
    }
}

struct LedgerHeader {
    magic: [u8; 3],
    version: u8,
    vec_offset: Nat,
}

impl From<&Ledger> for LedgerHeader {
    fn from(value: &Ledger) -> Self {
        Self {
            magic: *LEDGER_HEAD_MAGIC,
            version: LEDGER_HEAD_LAYOUT_VERSION,
            vec_offset: value.vec_offset.clone(),
        }
    }
}
