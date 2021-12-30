//! API methods of IS20 standard related to transaction notification mechanism.

use crate::api::dip20_transactions::transfer;
use crate::state::State;
use crate::types::{TxError, TxReceipt, TxRecord};
use candid::{candid_method, CandidType, Deserialize, Nat, Principal};
use ic_cdk_macros::*;
use ic_storage::IcStorage;

/// Notifies the transaction receiver about a previously performed transaction.
///
/// This method guarantees that a notification for the same transaction id can be sent only once.
/// It allows to use this method to reliably inform the transaction receiver without danger of
/// duplicate transaction attack.
///
/// In case the notification call fails, an [TxError::NotificationFailed] error is returned and
/// the transaction will still be marked as not notified.
///
/// If a notification request is made for a transaction that was already notified, a
/// [TxError::AlreadyNotified] error is returned.
#[update(name = "notify")]
#[candid_method(update, rename = "notify")]
async fn notify(transaction_id: Nat) -> TxReceipt {
    let state = State::get();
    let tx = {
        let mut state = state.borrow_mut();
        let tx = state
            .ledger()
            .get(&transaction_id)
            .ok_or(TxError::TransactionDoesNotExist)?;

        // We remove the notification here to prevent a concurrent call from being able to send the
        // notification again (while this call is await'ing). If the notification fails, we add the id
        // backed into the pending notifications list.
        if !state.notifications_mut().remove(&transaction_id) {
            return Err(TxError::AlreadyNotified);
        }

        tx
    };

    if send_notification(&tx).await.is_err() {
        state
            .borrow_mut()
            .notifications_mut()
            .insert(transaction_id);
        return Err(TxError::NotificationFailed);
    }

    Ok(tx.index)
}

/// Convenience method to make a transaction and notify the receiver with just one call.
///
/// If the notification fails for any reason, the transaction is still completed, but it will be
/// marked as not notified, so a [notify] call can be done later to re-request the notification of
/// this transaction.
#[update(name = "transferAndNotify")]
#[candid_method(update, rename = "transferAndNotify")]
async fn transfer_and_notify(to: Principal, amount: Nat) -> TxReceipt {
    let id = transfer(to, amount)?;
    notify(id).await
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

async fn send_notification(tx: &TxRecord) -> Result<(), ()> {
    let notification = TransactionNotification {
        tx_id: tx.index.clone(),
        from: tx.from,
        token_id: ic_kit::ic::id(),
        amount: tx.amount.clone(),
    };

    ic_kit::ic::call(tx.to, "transaction_notification", (notification,))
        .await
        .map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{default_canister_init, init_context};
    use ic_kit::mock_principals::{alice, bob};
    use ic_kit::{async_test, MockContext, RawHandler, RejectionCode};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[async_test]
    async fn notify_transaction() {
        const AMOUNT: u128 = 100;
        let is_notify_called = Rc::new(RefCell::new(false));
        let notify_copy = is_notify_called.clone();
        MockContext::new()
            .with_caller(alice())
            .with_handler(RawHandler::new(
                move |_, (notification,): (TransactionNotification,), canister_id, method_name| {
                    assert_eq!(method_name, "transaction_notification");
                    assert_eq!(*canister_id, bob());
                    assert_eq!(
                        notification,
                        TransactionNotification {
                            tx_id: notification.tx_id.clone(),
                            from: alice(),
                            amount: Nat::from(AMOUNT),
                            token_id: notification.token_id
                        }
                    );

                    *notify_copy.borrow_mut() = true;
                    Ok(())
                },
            ))
            .inject();

        default_canister_init();

        let id = transfer(bob(), Nat::from(AMOUNT)).unwrap();
        notify(id).await.unwrap();
        assert!(*is_notify_called.borrow());
    }

    #[async_test]
    async fn notify_non_existing() {
        init_context();
        let response = notify(Nat::from(10)).await;
        assert_eq!(response, Err(TxError::TransactionDoesNotExist));
    }

    #[async_test]
    async fn double_notification() {
        MockContext::new()
            .with_caller(alice())
            .with_constant_return_handler(())
            .inject();
        default_canister_init();
        let id = transfer(bob(), Nat::from(100)).unwrap();
        notify(id.clone()).await.unwrap();

        let response = notify(id).await;
        assert_eq!(response, Err(TxError::AlreadyNotified));
    }

    #[async_test]
    async fn notification_failure() {
        let context = MockContext::new()
            .with_caller(alice())
            .with_handler(RawHandler::new::<_, (), _>(|_, (): (), _, _| {
                Err((RejectionCode::Unknown, "".to_string()))
            }))
            .inject();
        default_canister_init();

        let id = transfer(bob(), Nat::from(100)).unwrap();
        let response = notify(id.clone()).await;
        assert_eq!(response, Err(TxError::NotificationFailed));

        context.clear_handlers();
        context.use_handler(RawHandler::new(|_, (): (), _, _| Ok(())));
        let response = notify(id.clone()).await;
        assert!(response.is_ok())
    }

    #[async_test]
    async fn transfer_and_notify_success() {
        let is_notify_called = Rc::new(RefCell::new(false));
        let notify_copy = is_notify_called.clone();
        MockContext::new()
            .with_caller(alice())
            .with_handler(RawHandler::new(
                move |_, (_,): (TransactionNotification,), _, _| {
                    *notify_copy.borrow_mut() = true;
                    Ok(())
                },
            ))
            .inject();

        default_canister_init();
        let id = transfer_and_notify(bob(), Nat::from(100)).await.unwrap();
        assert!(*is_notify_called.borrow());

        let response = notify(id.clone()).await;
        assert_eq!(response, Err(TxError::AlreadyNotified));
    }
}
