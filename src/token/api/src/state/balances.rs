use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;

use candid::{CandidType, Deserialize, Principal};
use canister_sdk::ic_helpers::tokens::Tokens128;
use ic_stable_structures::{BoundedStorable, MemoryId, StableMultimap, Storable};

use crate::account::{AccountInternal, Subaccount};

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

    /// Remove all balances.
    fn clear(&mut self) {
        for (account, _) in self.list_balances(0, usize::MAX) {
            self.remove(&account);
        }
    }
}

/// Store balances in stable memory.
pub struct StableBalances;

impl StableBalances {
    #[cfg(feature = "claim")]
    pub fn get_claimable_amount(holder: Principal, subaccount: Option<Subaccount>) -> Tokens128 {
        use canister_sdk::ledger::{AccountIdentifier, Subaccount as SubaccountIdentifier};

        use crate::account::DEFAULT_SUBACCOUNT;

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
        let principal_key = PrincipalKey(account.owner);
        let subaccount_key = SubaccountKey(account.subaccount);
        MAP.with(|map| {
            map.borrow_mut()
                .insert(&principal_key, &subaccount_key, &token.amount)
        });
    }

    /// Get amount of tokens for the specified account from stable memory.
    fn get(&self, account: &AccountInternal) -> Option<Tokens128> {
        let principal_key = PrincipalKey(account.owner);
        let subaccount_key = SubaccountKey(account.subaccount);
        MAP.with(|map| map.borrow_mut().get(&principal_key, &subaccount_key))
            .map(Tokens128::from)
    }

    /// Remove specified account balance from the stable memory.
    fn remove(&mut self, account: &AccountInternal) -> Option<Tokens128> {
        let principal_key = PrincipalKey(account.owner);
        let subaccount_key = SubaccountKey(account.subaccount);
        MAP.with(|map| map.borrow_mut().remove(&principal_key, &subaccount_key))
            .map(Tokens128::from)
    }

    fn get_subaccounts(&self, owner: Principal) -> HashMap<Subaccount, Tokens128> {
        MAP.with(|map| {
            map.borrow()
                .range(&PrincipalKey(owner))
                .map(|(subaccount, amount)| (subaccount.0, Tokens128::from(amount)))
                .collect()
        })
    }

    fn list_balances(&self, start: usize, limit: usize) -> Vec<(AccountInternal, Tokens128)> {
        MAP.with(|map| {
            map.borrow()
                .iter()
                .skip(start)
                .take(limit)
                .map(|(principal, subaccount, amount)| {
                    (
                        AccountInternal::new(principal.0, Some(subaccount.0)),
                        Tokens128::from(amount),
                    )
                })
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

    fn clear(&mut self) {
        self.0.clear()
    }
}

const BALANCES_MEMORY_ID: MemoryId = MemoryId::new(1);
const PRINCIPAL_MAX_LENGTH_IN_BYTES: usize = 29;
const SUBACCOUNT_MAX_LENGTH_IN_BYTES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PrincipalKey(Principal);

impl Storable for PrincipalKey {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        self.0.as_slice().into()
    }

    /// Expected `Principal::from_slice(&bytes)` is a correct operation.
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        PrincipalKey(Principal::from_slice(&bytes))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SubaccountKey(Subaccount);

impl Storable for SubaccountKey {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        self.0.as_slice().into()
    }

    /// Expected `bytes.len() == 32`.
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        let mut buf = [0u8; SUBACCOUNT_MAX_LENGTH_IN_BYTES];
        buf.copy_from_slice(&bytes);
        Self(buf)
    }
}

impl BoundedStorable for PrincipalKey {
    const MAX_SIZE: u32 = PRINCIPAL_MAX_LENGTH_IN_BYTES as _;
    const IS_FIXED_SIZE: bool = false;
}

impl BoundedStorable for SubaccountKey {
    const MAX_SIZE: u32 = SUBACCOUNT_MAX_LENGTH_IN_BYTES as _;
    const IS_FIXED_SIZE: bool = true;
}

thread_local! {
    static MAP: RefCell<StableMultimap<PrincipalKey, SubaccountKey, u128>> =
        RefCell::new(StableMultimap::new(BALANCES_MEMORY_ID));
}
