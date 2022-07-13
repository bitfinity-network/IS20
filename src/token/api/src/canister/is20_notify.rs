//! API methods of IS20 standard related to transaction notification mechanism.

use candid::Principal;
use ic_canister::virtual_canister_notify;

use crate::account::Account;
use crate::types::{TxError, TxId, TxReceipt};

use super::TokenCanisterAPI;

pub(crate) async fn consume_notification(
    canister: &impl TokenCanisterAPI,
    transaction_id: TxId,
) -> TxReceipt {
    let state = canister.state();
    let mut state = state.borrow_mut();
    match state.ledger.notifications.get(&transaction_id) {
        Some(Some(x)) if *x != ic_canister::ic_kit::ic::caller() => {
            return Err(TxError::Unauthorized);
        }
        Some(_) => {
            if state.ledger.notifications.remove(&transaction_id).is_none() {
                return Err(TxError::AlreadyActioned);
            }
        }
        None => return Err(TxError::NotificationDoesNotExist),
    }

    Ok(transaction_id)
}

/// This is a one-way call
pub(crate) async fn notify(
    canister: &impl TokenCanisterAPI,
    transaction_id: TxId,
    to: Principal,
) -> TxReceipt {
    let tx = canister
        .state()
        .borrow()
        .ledger
        .get(transaction_id)
        .ok_or(TxError::TransactionDoesNotExist)?;

    if Account::from(ic_canister::ic_kit::ic::caller()) != tx.from {
        return Err(TxError::Unauthorized);
    }

    match canister
        .state()
        .borrow_mut()
        .ledger
        .notifications
        .get_mut(&transaction_id)
    {
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
    use std::sync::atomic::{AtomicU32, Ordering};

    use ic_canister::ic_kit::mock_principals::{alice, bob};
    use ic_canister::ic_kit::MockContext;
    use ic_canister::{register_failing_virtual_responder, register_virtual_responder, Canister};
    use ic_helpers::tokens::Tokens128;

    use crate::mock::*;
    use crate::types::{Metadata, TxRecord};

    use super::*;

    fn test_canister() -> TokenCanisterMock {
        MockContext::new().with_caller(alice()).inject();

        let canister = TokenCanisterMock::init_instance();
        canister.init(Metadata {
            logo: "".to_string(),
            name: "".to_string(),
            symbol: "".to_string(),
            decimals: 8,
            totalSupply: Tokens128::from(1000),
            owner: alice(),
            fee: Tokens128::from(0),
            feeTo: alice(),
            isTestToken: None,
        });

        canister
    }

    #[tokio::test]
    async fn notify_non_existing() {
        let canister = test_canister();
        let response = canister.notify(10, bob()).await;
        assert_eq!(response, Err(TxError::TransactionDoesNotExist));
    }

    #[tokio::test]
    async fn double_notification() {
        let counter = Rc::new(AtomicU32::new(0));
        let counter_copy = counter.clone();
        register_virtual_responder(bob(), "transaction_notification", move |_: (TxRecord,)| {
            counter.fetch_add(1, Ordering::Relaxed);
        });
        let canister = test_canister();
        let id = canister
            .icrc1_transfer(None, bob(), None, Tokens128::from(100), None)
            .unwrap();
        canister.notify(id, bob()).await.unwrap();

        MockContext::new().with_caller(bob()).inject();
        let _ = canister.consume_notification(id).await;

        MockContext::new().with_caller(alice()).inject();
        let response = canister.notify(id, bob()).await;
        assert_eq!(response, Err(TxError::AlreadyActioned));
        assert_eq!(counter_copy.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn notification_failure() {
        register_failing_virtual_responder(
            bob(),
            "transaction_notification",
            "something's wrong".into(),
        );

        let canister = test_canister();
        let id = canister
            .icrc1_transfer(None, bob(), None, Tokens128::from(100), None)
            .unwrap();
        let response = canister.notify(id, bob()).await;
        assert_eq!(
            response,
            Err(TxError::NotificationFailed { transaction_id: 1 })
        );

        register_virtual_responder(bob(), "transaction_notification", move |_: (TxRecord,)| {});
        let response = canister.notify(id, bob()).await;
        assert!(response.is_ok())
    }
}
