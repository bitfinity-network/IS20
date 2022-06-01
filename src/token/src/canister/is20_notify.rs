//! API methods of IS20 standard related to transaction notification mechanism.

use candid::{Nat, Principal};
use ic_canister::virtual_canister_notify;

use crate::canister::TokenCanister;
use crate::types::{TxError, TxReceipt};

pub(crate) async fn approve_and_notify(
    canister: &TokenCanister,
    spender: Principal,
    value: Nat,
) -> TxReceipt {
    let transaction_id = canister.approve(spender, value)?;
    notify(canister, transaction_id.clone(), spender)
        .await
        .map_err(|e| TxError::ApproveSucceededButNotifyFailed {
            tx_error: Box::from(e),
        })
}

/// This is a one-way call
pub(crate) async fn notify(
    canister: &TokenCanister,
    transaction_id: Nat,
    to: Principal,
) -> TxReceipt {
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

    match virtual_canister_notify!(to, "transaction_notification", (tx,), ()).await {
        Ok(_) => Ok(transaction_id),
        Err(_) => Err(TxError::NotificationFailed { transaction_id }),
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    use common::types::Metadata;
    use ic_canister::{register_virtual_responder, Canister};
    use ic_kit::mock_principals::{alice, bob};
    use ic_kit::MockContext;

    use crate::types::TxRecord;
    use super::*;

    fn test_canister() -> TokenCanister {
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

        let is_notified = Rc::new(AtomicBool::new(false));
        let is_notified_clone = is_notified.clone();
        let counter = Rc::new(AtomicU32::new(0));
        let counter_copy = counter.clone();
        register_virtual_responder(
            bob(),
            "transaction_notification",
            move |(notification,): (TxRecord,)| {
                is_notified.swap(true, Ordering::Relaxed);
                counter.fetch_add(1, Ordering::Relaxed);
                assert_eq!(notification.amount, AMOUNT);
            },
        );

        let canister = test_canister();

        canister
            .approveAndNotify(bob(), Nat::from(AMOUNT))
            .await
            .unwrap();
        assert!(is_notified_clone.load(Ordering::Relaxed));
        assert_eq!(counter_copy.load(Ordering::Relaxed), 1);
    }
}
