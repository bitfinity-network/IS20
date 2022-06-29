use crate::state::{LEDGER_HEADER, STABLE_MAP};
use crate::types::{PaginatedResult, PendingNotifications, TxId, TxRecord, TxRecordStable};
use candid::{CandidType, Deserialize, Principal};
use ic_helpers::tokens::Tokens128;
use stable_structures::{stable_storage::StableStorage, RestrictedMemory};

const MAX_HISTORY_LENGTH: usize = 1_000_000;
const HISTORY_REMOVAL_BATCH_SIZE: usize = 10_000;
const LEDGER_HEAD_MAGIC: &[u8; 3] = b"LER";
const LEDGER_HEAD_LAYOUT_VERSION: u8 = 1;

#[derive(Debug, Default, CandidType, Deserialize)]
pub struct Ledger {
    history: TxRecordStable,
    vec_offset: u64,
    pub notifications: PendingNotifications,
}

impl Ledger {
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> u64 {
        self.vec_offset + self.history.len() as u64
    }

    fn next_id(&self) -> TxId {
        self.vec_offset + self.history.len() as u64
    }

    pub fn get(&self, id: TxId) -> Option<TxRecord> {
        self.history.get(self.get_index(id)?)
    }

    pub fn get_transactions(
        &self,
        who: Option<Principal>,
        count: usize,
        transaction_id: Option<TxId>,
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
            Some(transactions.remove(count).index)
        } else {
            None
        };

        PaginatedResult {
            result: transactions,
            next: next_id,
        }
    }

    fn get_index(&self, id: TxId) -> Option<usize> {
        if id < self.vec_offset || id > usize::MAX as TxId {
            None
        } else {
            Some((id) as usize)
        }
    }

    pub fn get_len_user_history(&self, user: Principal) -> usize {
        STABLE_MAP.with(|s| {
            let map = s.borrow();
            let mut size = 0;
            for (k, _) in self.history.index.range(None, None, &map) {
                let key = self.history.index.key_decode::<u64>(&k) as usize;
                let tx = self.history.get(key).unwrap();
                if tx.to == user || tx.from == user || tx.caller == Some(user) {
                    size += 1;
                }
            }
            size
        })
    }

    pub fn transfer(
        &mut self,
        from: Principal,
        to: Principal,
        amount: Tokens128,
        fee: Tokens128,
    ) -> TxId {
        let id = self.next_id();
        self.push(TxRecord::transfer(id, from, to, amount, fee));

        id
    }

    pub fn batch_transfer(
        &mut self,
        from: Principal,
        transfers: Vec<(Principal, Tokens128)>,
        fee: Tokens128,
    ) -> Vec<TxId> {
        transfers
            .into_iter()
            .map(|(to, amount)| self.transfer(from, to, amount, fee))
            .collect()
    }

    pub fn transfer_from(
        &mut self,
        caller: Principal,
        from: Principal,
        to: Principal,
        amount: Tokens128,
        fee: Tokens128,
    ) -> TxId {
        let id = self.next_id();
        self.push(TxRecord::transfer_from(id, caller, from, to, amount, fee));

        id
    }

    pub fn approve(
        &mut self,
        from: Principal,
        to: Principal,
        amount: Tokens128,
        fee: Tokens128,
    ) -> TxId {
        let id = self.next_id();
        self.push(TxRecord::approve(id, from, to, amount, fee));

        id
    }

    pub fn mint(&mut self, from: Principal, to: Principal, amount: Tokens128) -> TxId {
        let id = self.len();
        self.push(TxRecord::mint(id, from, to, amount));

        id
    }

    pub fn burn(&mut self, caller: Principal, from: Principal, amount: Tokens128) -> TxId {
        let id = self.next_id();
        self.push(TxRecord::burn(id, caller, from, amount));

        id
    }

    pub fn auction(&mut self, to: Principal, amount: Tokens128) {
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
        self.history.push(record.clone(), self.len());
        self.notifications.insert(record.index, None);

        if self.history.len() > MAX_HISTORY_LENGTH + HISTORY_REMOVAL_BATCH_SIZE {
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
                self.notifications.remove(&record.index);
            }
            for key in keys.iter() {
                self.history.remove(*key);
            }
            self.vec_offset += HISTORY_REMOVAL_BATCH_SIZE as u64;
            LEDGER_HEADER.with(|l| {
                self.save_header(&l.borrow());
            });
        }
    }
}

struct LedgerHeader {
    magic: [u8; 3],
    version: u8,
    vec_offset: u64,
}

impl From<&Ledger> for LedgerHeader {
    fn from(value: &Ledger) -> Self {
        Self {
            magic: *LEDGER_HEAD_MAGIC,
            version: LEDGER_HEAD_LAYOUT_VERSION,
            vec_offset: value.vec_offset,
        }
    }
}
