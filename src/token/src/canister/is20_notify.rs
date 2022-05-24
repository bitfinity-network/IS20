//! API methods of IS20 standard related to transaction notification mechanism.

use crate::canister::TokenCanister;
use crate::types::{TxError, TxReceipt, TxRecord};
use candid::{CandidType, Deserialize, Nat, Principal};
use ic_canister::virtual_canister_call;
use ic_cdk::api::call::CallResult;

pub(crate) async fn notify(canister: &TokenCanister, transaction_id: Nat) -> TxReceipt {
    let tx = {
        let mut state = canister.state.borrow_mut();
        let tx = state
            .ledger
            .get(&transaction_id)
            .ok_or(TxError::TransactionDoesNotExist)?;

        // We remove the notification here to prevent a concurrent call from being able to send the
        // notification again (while this call is await'ing). If the notification fails, we add the id
        // backed into the pending notifications list.
        if !state.notifications.remove(&transaction_id) {
            return Err(TxError::AlreadyNotified);
        }

        tx
    };

    match send_notification(&tx).await {
        Ok(()) => Ok(tx.index),
        Err((_, description)) => {
            canister
                .state
                .borrow_mut()
                .notifications
                .insert(transaction_id);
            Err(TxError::NotificationFailed {
                cdk_msg: description,
            })
        }
    }
}

pub(crate) async fn transfer_and_notify(
    canister: &TokenCanister,
    to: Principal,
    amount: Nat,
    fee_limit: Option<Nat>,
) -> TxReceipt {
    let id = canister.transfer(to, amount, fee_limit)?;
    notify(canister, id).await
}

#[derive(CandidType, Deserialize, Debug, PartialEq)]
pub struct TransactionNotification {
    /// Transaction id.
    pub tx_id: Nat,

    /// Id of the principal (user, canister) that owns the tokens being transferred.
    pub from: Principal,

    /// Id of the token canister.
    pub token_id: Principal,

    /// Amount of tokens being transferred.
    pub amount: Nat,
}

async fn send_notification(tx: &TxRecord) -> CallResult<()> {
    let notification = TransactionNotification {
        tx_id: tx.index.clone(),
        from: tx.from,
        token_id: ic_kit::ic::id(),
        amount: tx.amount.clone(),
    };

    virtual_canister_call!(tx.to, "transaction_notification", (notification,), ()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::types::Metadata;
    use ic_canister::{register_failing_virtual_responder, register_virtual_responder, Canister};
    use ic_kit::mock_principals::{alice, bob};
    use ic_kit::MockContext;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

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
    async fn notify_transaction() {
        const AMOUNT: u128 = 100;

        let is_notified = Rc::new(AtomicBool::new(false));
        let is_notified_clone = is_notified.clone();
        register_virtual_responder(
            bob(),
            "transaction_notification",
            move |(notification,): (TransactionNotification,)| {
                is_notified.swap(true, Ordering::Relaxed);
                assert_eq!(notification.amount, AMOUNT);
            },
        );

        let canister = test_canister();

        let id = canister.transfer(bob(), Nat::from(AMOUNT), None).unwrap();
        canister.notify(id).await.unwrap();
        assert!(is_notified_clone.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn notify_non_existing() {
        let canister = test_canister();
        let response = canister.notify(Nat::from(10)).await;
        assert_eq!(response, Err(TxError::TransactionDoesNotExist));
    }

    #[tokio::test]
    async fn double_notification() {
        let counter = Rc::new(AtomicU32::new(0));
        let counter_copy = counter.clone();
        register_virtual_responder(
            bob(),
            "transaction_notification",
            move |_: (TransactionNotification,)| {
                counter.fetch_add(1, Ordering::Relaxed);
            },
        );
        let canister = test_canister();
        let id = canister.transfer(bob(), Nat::from(100), None).unwrap();
        canister.notify(id.clone()).await.unwrap();

        let response = canister.notify(id).await;
        assert_eq!(response, Err(TxError::AlreadyNotified));
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
        let id = canister.transfer(bob(), Nat::from(100u32), None).unwrap();
        let response = canister.notify(id.clone()).await;
        assert!(response.is_err());

        register_virtual_responder(
            bob(),
            "transaction_notification",
            move |_: (TransactionNotification,)| {},
        );
        let response = canister.notify(id.clone()).await;
        assert!(response.is_ok())
    }

    #[tokio::test]
    async fn transfer_and_notify_success() {
        let is_notified = Rc::new(AtomicBool::new(false));
        let is_notified_clone = is_notified.clone();
        register_virtual_responder(
            bob(),
            "transaction_notification",
            move |_: (TransactionNotification,)| {
                is_notified.swap(true, Ordering::Relaxed);
            },
        );

        let canister = test_canister();
        let id = canister
            .transferAndNotify(bob(), Nat::from(100), None)
            .await
            .unwrap();
        assert!(is_notified_clone.load(Ordering::Relaxed));

        let response = canister.notify(id.clone()).await;
        assert_eq!(response, Err(TxError::AlreadyNotified));
    }
}
