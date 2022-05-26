//! API methods of IS20 standard related to transaction notification mechanism.

use crate::canister::TokenCanister;
use crate::types::{TxError, TxReceipt, TxRecord};
use candid::{CandidType, Deserialize, Nat, Principal};
use ic_canister::canister_notify;
use ic_storage::{stable::Versioned, IcStorage};
use ic_canister::{update, query, Canister};
use std::cell::RefCell;

#[derive(Default, CandidType, Deserialize, IcStorage)]
struct State {
    value: Nat,
}

impl Versioned for State {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}

#[derive(Clone, Canister)]
pub struct TestCanister {
    #[id]
    principal: Principal,
    #[state(stable_store = true)]
    state: std::rc::Rc<RefCell<State>>,
}

impl TestCanister {  
    #[update]
    fn transaction_notification(&mut self, tx: TxRecord) {
        RefCell::borrow_mut(&self.state).value += tx.amount;
    }
    #[query]
    fn get_value(&self) -> Nat {
        self.state.borrow().value.clone()
    }
}


pub(crate) fn approve_and_notify(
    canister: &TokenCanister,
    spender: Principal,
    value: Nat,
) -> TxReceipt {
    let transaction_id = canister.approve(spender, value)?;
    notify(canister, transaction_id.clone(), spender).map_err(|e| {
        TxError::ApproveSucceededButNotifyFailed {
            tx_error: Box::from(e),
        }
    })
}

pub(crate) async fn consume_notification(
    canister: &TokenCanister,
    transaction_id: Nat,
) -> TxReceipt {
    let mut state = canister.state.borrow_mut();

    match state.ledger.notifications.get(&transaction_id) {
        Some(Some(x)) if *x != ic_kit::ic::caller() => return Err(TxError::Unauthorized),
        Some(x) => {
            if state.ledger.notifications.remove(&transaction_id).is_none() {
                return Err(TxError::AlreadyActioned);
            }
        }
        None => return Err(TxError::NotificationDoesNotExist),
    }

    Ok(transaction_id)
}

/// This is a one-way call
pub(crate) fn notify(canister: &TokenCanister, transaction_id: Nat, to: Principal) -> TxReceipt {
    let mut state = canister.state.borrow_mut();
    let tx = state
        .ledger
        .get(&transaction_id)
        .ok_or(TxError::TransactionDoesNotExist)?;

    if ic_kit::ic::caller() != tx.from {
        return Err(TxError::Unauthorized);
    }

    match state.ledger.notifications.get_mut(&transaction_id) {
        Some(Some(dest)) if *dest != to => return Err(TxError::Unauthorized),
        Some(x) => *x = Some(to),
        None => return Err(TxError::AlreadyActioned),
    }

    let mut canister_to = TestCanister::from_principal(to);
    match canister_notify!(canister_to.transaction_notification(tx.clone()), ()) {
        Ok(()) => Ok(tx.index),
        Err(e) => Err(TxError::NotificationFailed { transaction_id }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::types::Metadata;
    use ic_kit::mock_principals::alice;
    use ic_kit::MockContext;

    fn token_canister() -> TokenCanister {
        MockContext::new().with_caller(alice()).inject();

        let canister = TokenCanister::init_instance();
        canister.init(Metadata {
            logo: "".to_string(),
            name: "".to_string(),
            symbol: "".to_string(),
            decimals: 8,
            totalSupply: Nat::from(1000),
            owner: alice(),
            fee: Nat::from(0),
            feeTo: alice(),
            isTestToken: None,
        });

        canister
    }

    #[tokio::test]
    async fn approve_notify() {
        const AMOUNT: u128 = 100;

        let canister1 = token_canister();
        let canister2 = TestCanister::init_instance();

        assert_eq!(canister1.approveAndNotify(canister2.principal(), Nat::from(AMOUNT)).unwrap(), Nat::from(1));
        assert_eq!(canister2.__get_value().await.unwrap(), Nat::from(AMOUNT));
    }
}
