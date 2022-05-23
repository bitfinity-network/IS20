//! API methods of IS20 standard related to transaction notification mechanism.

use crate::canister::TokenCanister;
use crate::types::{TxError, TxReceipt, TxRecord};
use candid::{CandidType, Deserialize, Nat, Principal};
use ic_canister::virtual_canister_call_oneway;
use ic_cdk::api::call::CallResult;

pub(crate) fn approve_and_notify(
    canister: &TokenCanister,
    spender: Principal,
    value: Nat,
) -> TxReceipt {
    let transaction_id = canister.approve(spender, value)?;
    let state = canister.state.borrow_mut();
    let tx = state
        .ledger
        .get(&transaction_id)
        .ok_or(TxError::TransactionDoesNotExist)?;
    match send_notification(&tx) {
        Ok(()) => Ok(tx.index),
        Err((_, _)) => Err(TxError::NotificationFailed),
    }
}

#[derive(CandidType, Deserialize, Debug, PartialEq)]
pub struct ApproveNotification {
    /// Transaction id.
    pub tx_id: Nat,

    /// Id of the principal (user, canister) that approve the tokens being transferred.
    pub from: Principal,

    /// Id of the token canister.
    pub token_id: Principal,

    /// Approved amount of tokens being transferred.
    pub amount: Nat,
}

#[allow(unused_variables)]
fn send_notification(tx: &TxRecord) -> CallResult<()> {
    let notification = ApproveNotification {
        tx_id: tx.index.clone(),
        from: tx.from,
        token_id: ic_kit::ic::id(),
        amount: tx.amount.clone(),
    };

    virtual_canister_call_oneway!(tx.to, "approve_notification", (notification,), ()).map_err(|e| (e, String::from("Rejected before send.")))
}



#[cfg(test)]
mod tests {
    use super::*;
    use common::types::Metadata;
    use ic_canister::{register_virtual_responder, Canister};
    use ic_kit::mock_principals::{alice, bob};
    use ic_kit::MockContext;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicBool, Ordering};

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
        register_virtual_responder(
            bob(),
            "approve_notification",
            move |(notification,): (ApproveNotification,)| {
                is_notified.swap(true, Ordering::Relaxed);
                assert_eq!(notification.amount, AMOUNT);
            },
        );

        let canister = test_canister();

        canister.approveAndNotify(bob(), Nat::from(AMOUNT)).unwrap();
        assert!(!is_notified_clone.load(Ordering::Relaxed));
    }
}
