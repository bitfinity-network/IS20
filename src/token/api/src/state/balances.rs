use std::{borrow::Cow, cell::RefCell, collections::HashMap};

use candid::{CandidType, Deserialize, Principal};
use canister_sdk::ic_helpers::tokens::Tokens128;
use ic_stable_structures::{memory_manager::MemoryId, Storable};

use crate::{
    account::{AccountInternal, Subaccount},
    storage::StableBTreeMap,
};

pub trait Balances {
    /// Write or re-write amount of tokens for specified account.
    fn insert(&mut self, account: AccountInternal, token: Tokens128);

    /// Get amount of tokens for the specified account.
    fn get(&self, account: &AccountInternal) -> Option<Tokens128>;

    /// Remove specified account balance.
    fn remove(&mut self, account: &AccountInternal) -> Option<Tokens128>;

    /// Get list of `limit` balances, starting with `start`.
    fn list_balances(&self, start: usize, limit: usize) -> Vec<(AccountInternal, Tokens128)>;

    /// Get amount of tokens for the specified account.
    /// If account is not present, return zero.
    fn balance_of(&self, account: &AccountInternal) -> Tokens128 {
        self.get(account).unwrap_or_default()
    }

    /// Update balances according to `updates` iterator.
    fn apply_updates(&mut self, updates: impl IntoIterator<Item = (AccountInternal, Tokens128)>) {
        for (account, amount) in updates {
            self.insert(account, amount);
        }
    }

    /// List subaccounts for the given principal.
    fn get_subaccounts(&self, owner: Principal) -> HashMap<Subaccount, Tokens128> {
        self.list_balances(0, usize::MAX)
            .into_iter()
            .filter(|(account, _)| account.owner == owner)
            .map(|(account, amount)| (account.subaccount, amount))
            .collect()
    }

    /// Return sum of all balances.
    fn total_supply(&self) -> Tokens128 {
        self.list_balances(0, usize::MAX)
            .into_iter()
            .fold(Tokens128::ZERO, |a, b| {
                (a + b.1).expect("total supply integer overflow") // Checked at mint
            })
    }

    /// Get balances map: holder -> subaccount -> tokens.
    fn get_holders(&self) -> HashMap<Principal, HashMap<Subaccount, Tokens128>> {
        let mut holders: HashMap<Principal, HashMap<Subaccount, Tokens128>> = HashMap::new();
        for (account, amount) in self.list_balances(0, usize::MAX) {
            holders
                .entry(account.owner)
                .or_default()
                .insert(account.subaccount, amount);
        }
        holders
    }
}

/// Store balances in stable memory.
pub struct StableBalances;

impl StableBalances {
    #[cfg(feature = "claim")]
    pub fn get_claimable_amount(holder: Principal, subaccount: Option<Subaccount>) -> Tokens128 {
        use crate::account::DEFAULT_SUBACCOUNT;
        use canister_sdk::ledger_canister::{
            AccountIdentifier, Subaccount as SubaccountIdentifier,
        };

        let claim_subaccount = AccountIdentifier::new(
            canister_sdk::ic_kit::ic::caller().into(),
            Some(SubaccountIdentifier(
                subaccount.unwrap_or(DEFAULT_SUBACCOUNT),
            )),
        )
        .to_address();

        let account = AccountInternal::new(holder, Some(claim_subaccount));
        Self.balance_of(&account)
    }
}

impl Balances for StableBalances {
    /// Write or re-write amount of tokens for specified account to stable memory.
    fn insert(&mut self, account: AccountInternal, token: Tokens128) {
        MAP.with(|map| map.borrow_mut().insert(account.into(), token.amount))
            .expect("unable to insert new balance to stable storage");
        // Key and value have fixed byte size, so the only possible error is OOM.
    }

    /// Get amount of tokens for the specified account from stable memory.
    fn get(&self, account: &AccountInternal) -> Option<Tokens128> {
        let amount = MAP.with(|map| map.borrow_mut().get(&(*account).into()));
        amount.map(Tokens128::from)
    }

    /// Remove specified account balance from the stable memory.
    fn remove(&mut self, account: &AccountInternal) -> Option<Tokens128> {
        let amount = MAP.with(|map| map.borrow_mut().remove(&(*account).into()));
        amount.map(Tokens128::from)
    }

    fn list_balances(&self, start: usize, limit: usize) -> Vec<(AccountInternal, Tokens128)> {
        MAP.with(|map| {
            map.borrow()
                .list(start, limit)
                .into_iter()
                .map(|(key, amount)| (key.into(), Tokens128::from(amount)))
                .collect()
        })
    }
}

/// We are saving the `Balances` in this format, as we want to support `Principal` supporting `Subaccount`.
#[derive(Debug, Default, CandidType, Deserialize)]
pub struct LocalBalances(HashMap<AccountInternal, Tokens128>);

impl LocalBalances {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}

impl FromIterator<(AccountInternal, Tokens128)> for LocalBalances {
    fn from_iter<T: IntoIterator<Item = (AccountInternal, Tokens128)>>(iter: T) -> Self {
        Self(HashMap::from_iter(iter))
    }
}

impl Balances for LocalBalances {
    fn insert(&mut self, account: AccountInternal, token: Tokens128) {
        self.0.insert(account, token);
    }

    fn get(&self, account: &AccountInternal) -> Option<Tokens128> {
        self.0.get(account).copied()
    }

    fn list_balances(&self, start: usize, limit: usize) -> Vec<(AccountInternal, Tokens128)> {
        let mut holders = self
            .0
            .iter()
            .skip(start)
            .take(limit)
            .map(|(account, tokens)| (*account, *tokens))
            .collect::<Vec<_>>();
        holders.sort_by(|a, b| b.1.cmp(&a.1));
        holders
    }

    fn remove(&mut self, account: &AccountInternal) -> Option<Tokens128> {
        self.0.remove(account)
    }

    fn total_supply(&self) -> Tokens128 {
        self.0.iter().fold(
            Tokens128::ZERO,
            |a, b| (a + b.1).expect("total supply integer overflow"), // Checked at mint
        )
    }
}

const BALANCES_MEMORY_ID: MemoryId = MemoryId::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Key {
    pub principal: Principal,
    pub subaccount: Subaccount,
}

impl From<AccountInternal> for Key {
    fn from(account: AccountInternal) -> Self {
        Self {
            principal: account.owner,
            subaccount: account.subaccount,
        }
    }
}

impl From<Key> for AccountInternal {
    fn from(key: Key) -> Self {
        Self {
            owner: key.principal,
            subaccount: key.subaccount,
        }
    }
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
        let mut buffer = vec![0u8; KEY_BYTES_LEN];
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
    static MAP: RefCell<StableBTreeMap<Key, u128>> =
        RefCell::new(StableBTreeMap::new(BALANCES_MEMORY_ID, KEY_BYTES_LEN as _, VALUE_BYTES_LEN as _));
}

#[cfg(test)]
mod tests {
    use candid::Principal;
    use ic_stable_structures::Storable;

    use super::Key;

    #[test]
    fn serialization_deserialization() {
        let key = Key {
            principal: Principal::anonymous(),
            subaccount: [42; 32],
        };

        let desirialized = Key::from_bytes(key.to_bytes().into_owned());
        assert_eq!(desirialized, key);
    }
}
