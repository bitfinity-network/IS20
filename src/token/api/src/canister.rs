use std::cell::RefCell;
use std::rc::Rc;

use canister_sdk::{
    ic_auction::state::Timestamp,
    ic_canister::{
        generate_exports, generate_idl, query, state_getter, update, Canister, Idl, PreUpdate,
    },
    ic_cdk::export::candid::Principal,
    ic_helpers::tokens::Tokens128,
    ic_kit::ic,
    ic_storage,
};
#[cfg(feature = "auction")]
use canister_sdk::{
    ic_auction::{
        api::Auction,
        error::AuctionError,
        state::{AuctionInfo, AuctionState},
    },
    ic_storage::IcStorage,
};

pub use inspect::AcceptReason;

use crate::{
    account::{Account, AccountInternal, CheckedAccount, Subaccount},
    canister::icrc1_transfer::icrc1_transfer,
    error::{TransferError, TxError},
    principal::{CheckedPrincipal, Owner},
    state::{
        stats::{StandardRecord, StatsData, TokenInfo, Value},
        CanisterState,
    },
    tx_record::{TxId, TxRecord},
    types::{BatchTransferArgs, PaginatedResult, TransferArgs, TxReceipt},
};

use self::is20_transactions::{
    batch_transfer, burn_as_owner, burn_own_tokens, is20_transfer, mint_as_owner, mint_test_token,
};

#[cfg(feature = "claim")]
use self::is20_transactions::{claim, get_claim_subaccount};

mod inspect;

pub mod icrc1_transfer;

#[cfg(feature = "auction")]
pub mod is20_auction;
pub mod is20_transactions;

pub(crate) const MAX_TRANSACTION_QUERY_LEN: usize = 1000;
// 1 day in seconds.
pub const DEFAULT_AUCTION_PERIOD_SECONDS: Timestamp = 60 * 60 * 24;

pub enum CanisterUpdate {
    Name(String),
    Symbol(String),
    Logo(String),
    Fee(Tokens128),
    FeeTo(Principal),
    Owner(Principal),
    MinCycles(u64),
}

#[cfg(not(feature = "auction"))]
pub trait AuctionCanister {}

#[cfg(feature = "auction")]
pub trait AuctionCanister: Auction {}

impl<T: TokenCanisterAPI> AuctionCanister for T {}

pub trait TokenCanisterAPI: Canister + Sized + AuctionCanister {
    #[state_getter]
    fn state(&self) -> Rc<RefCell<CanisterState>>;

    /// The `inspect_message()` call is not exported by default. Add your custom #[inspect_message]
    /// function and use this method there to export the `inspect_message()` call.
    fn inspect_message(method: &str, caller: Principal) -> Result<AcceptReason, &'static str> {
        inspect::inspect_message(method, caller)
    }

    /********************** METADATA ***********************/

    #[query(trait = true)]
    fn is_test_token(&self) -> bool {
        StatsData::get_stable().is_test_token
    }

    #[query(trait = true)]
    fn logo(&self) -> String {
        StatsData::get_stable().logo
    }

    #[query(trait = true)]
    fn icrc1_total_supply(&self) -> Tokens128 {
        self.state().borrow().balances.total_supply()
    }

    #[query(trait = true)]
    fn owner(&self) -> Principal {
        StatsData::get_stable().owner
    }

    #[query(trait = true)]
    fn get_token_info(&self) -> TokenInfo {
        let StatsData {
            fee_to,
            deploy_time,
            ..
        } = StatsData::get_stable();
        TokenInfo {
            metadata: self.state().borrow().get_metadata(),
            fee_to,
            history_size: self.state().borrow().ledger.len(),
            deployTime: deploy_time,
            holderNumber: self.state().borrow().balances.0.len(),
            cycles: canister_sdk::ic_kit::ic::balance(),
        }
    }

    #[update(trait = true)]
    fn set_name(&self, name: String) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&StatsData::get_stable())?;
        self.update_stats(caller, CanisterUpdate::Name(name));
        Ok(())
    }

    #[update(trait = true)]
    fn set_symbol(&self, symbol: String) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&StatsData::get_stable())?;
        self.update_stats(caller, CanisterUpdate::Symbol(symbol));
        Ok(())
    }

    #[update(trait = true)]
    fn set_logo(&self, logo: String) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&StatsData::get_stable())?;
        self.update_stats(caller, CanisterUpdate::Logo(logo));
        Ok(())
    }

    #[update(trait = true)]
    fn set_fee(&self, fee: Tokens128) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&StatsData::get_stable())?;
        self.update_stats(caller, CanisterUpdate::Fee(fee));
        Ok(())
    }

    #[update(trait = true)]
    fn set_fee_to(&self, fee_to: Principal) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&StatsData::get_stable())?;
        self.update_stats(caller, CanisterUpdate::FeeTo(fee_to));
        Ok(())
    }

    #[update(trait = true)]
    fn set_owner(&self, owner: Principal) -> Result<(), TxError> {
        let caller = CheckedPrincipal::owner(&StatsData::get_stable())?;
        self.update_stats(caller, CanisterUpdate::Owner(owner));
        Ok(())
    }

    /********************** BALANCES INFO ***********************/

    /// This method retreieves holders of `Account` and their amounts.
    #[query(trait = true)]
    fn get_holders(&self, start: usize, limit: usize) -> Vec<(Account, Tokens128)> {
        self.state()
            .borrow()
            .balances
            .get_holders(start, limit)
            .into_iter()
            .map(|(acc, amount)| (acc.into(), amount))
            .collect()
    }

    /// Returns the list of the caller's subaccounts with balances. If the caller account does not exist, will
    /// return an empty list.
    ///
    /// It is intentional that the method does not accept the principal to list the subaccounts
    /// for, because in some cases the token holder want to keep some of his subaccounts a secret.
    /// So only own subaccounts can be listed safely.
    #[query(trait = true)]
    fn list_subaccounts(&self) -> std::collections::HashMap<Subaccount, Tokens128> {
        self.state()
            .borrow()
            .balances
            .list_subaccounts(ic::caller())
    }

    /********************** CLAIMS ***********************/

    #[cfg(feature = "claim")]
    #[query(trait = true)]
    fn get_claimable_amount(&self, holder: Principal, subaccount: Option<Subaccount>) -> Tokens128 {
        self.state()
            .borrow()
            .get_claimable_amount(holder, subaccount)
    }

    #[cfg(feature = "claim")]
    #[query(trait = true)]
    fn get_claim_subaccount(
        &self,
        claimer: Principal,
        claimer_subaccount: Option<Subaccount>,
    ) -> Subaccount {
        get_claim_subaccount(claimer, claimer_subaccount)
    }

    #[cfg(feature = "claim")]
    #[update(trait = true)]
    fn claim(&self, holder: Principal, subaccount: Option<Subaccount>) -> TxReceipt {
        claim(&mut self.state().borrow_mut(), holder, subaccount)
    }

    /********************** TRANSACTION HISTORY ***********************/

    #[query(trait = true)]
    fn history_size(&self) -> u64 {
        self.state().borrow().ledger.len()
    }

    #[query(trait = true)]
    fn get_transaction(&self, id: TxId) -> TxRecord {
        self.state().borrow().ledger.get(id).unwrap_or_else(|| {
            canister_sdk::ic_kit::ic::trap(&format!("Transaction {} does not exist", id))
        })
    }

    /// Returns a list of transactions in paginated form. The `who` is optional, if given, only transactions of the `who` are
    /// returned. `count` is the number of transactions to return, `transaction_id` is the transaction index which is used as
    /// the offset of the first transaction to return, any
    ///
    /// It returns `PaginatedResult` a struct, which contains `result` which is a list of transactions `Vec<TxRecord>` that meet the requirements of the query,
    /// and `next_id` which is the index of the next transaction to return.
    #[query(trait = true)]
    fn get_transactions(
        &self,
        who: Option<Principal>,
        count: usize,
        transaction_id: Option<TxId>,
    ) -> PaginatedResult {
        self.state().borrow().ledger.get_transactions(
            who,
            count.min(MAX_TRANSACTION_QUERY_LEN),
            transaction_id,
        )
    }

    /// Returns the total number of transactions related to the user `who`.
    #[query(trait = true)]
    fn get_user_transaction_count(&self, who: Principal) -> usize {
        self.state().borrow().ledger.get_len_user_history(who)
    }

    /********************** IS20 TRANSACTIONS ***********************/

    #[cfg_attr(feature = "transfer", update(trait = true))]
    fn transfer(&self, transfer: TransferArgs) -> Result<u128, TxError> {
        let account = CheckedAccount::with_recipient(transfer.to.into(), transfer.from_subaccount)?;
        is20_transfer(
            &mut self.state().borrow_mut(),
            account,
            &transfer,
            self.fee_ratio(),
        )
    }

    /// Takes a list of transfers, each of which is a pair of `to` and `value` fields, it returns a `TxReceipt` which contains
    /// a vec of transaction index or an error message. The list of transfers is processed in the order they are given. if the `fee`
    /// is set, the `fee` amount is applied to each transfer.
    /// The balance of the caller is reduced by sum of `value + fee` amount for each transfer. If the total sum of `value + fee` for all transfers,
    /// is less than the `balance` of the caller, the transaction will fail with `TxError::InsufficientBalance` error.
    #[cfg_attr(feature = "transfer", update(trait = true))]
    fn batch_transfer(
        &self,
        from_subaccount: Option<Subaccount>,
        transfers: Vec<BatchTransferArgs>,
    ) -> Result<Vec<TxId>, TxError> {
        for x in &transfers {
            let recipient = x.receiver;
            CheckedAccount::with_recipient(recipient.into(), from_subaccount)?;
        }
        batch_transfer(
            &mut self.state().borrow_mut(),
            from_subaccount,
            transfers,
            self.fee_ratio(),
        )
    }

    #[cfg_attr(feature = "mint_burn", update(trait = true))]
    fn mint(
        &self,
        to: Principal,
        to_subaccount: Option<Subaccount>,
        amount: Tokens128,
    ) -> TxReceipt {
        if self.is_test_token() {
            let test_user = CheckedPrincipal::test_user(&StatsData::get_stable())?;
            mint_test_token(
                &mut self.state().borrow_mut(),
                test_user,
                to,
                to_subaccount,
                amount,
            )
        } else {
            let owner = CheckedPrincipal::owner(&StatsData::get_stable())?;
            mint_as_owner(
                &mut self.state().borrow_mut(),
                owner,
                to,
                to_subaccount,
                amount,
            )
        }
    }

    /// Burn `amount` of tokens from `from` principal.
    /// If `from` is None, then caller's tokens will be burned.
    /// If `from` is Some(_) but method called not by owner, `TxError::Unauthorized` will be returned.
    /// If owner calls this method and `from` is Some(who), then who's tokens will be burned.
    #[cfg_attr(feature = "mint_burn", update(trait = true))]
    fn burn(
        &self,
        from: Option<Principal>,
        from_subaccount: Option<Subaccount>,
        amount: Tokens128,
    ) -> TxReceipt {
        match from {
            None => burn_own_tokens(&mut self.state().borrow_mut(), from_subaccount, amount),
            Some(from) if from == canister_sdk::ic_kit::ic::caller() => {
                burn_own_tokens(&mut self.state().borrow_mut(), from_subaccount, amount)
            }
            Some(from) => {
                let caller = CheckedPrincipal::owner(&StatsData::get_stable())?;
                burn_as_owner(
                    &mut self.state().borrow_mut(),
                    caller,
                    from,
                    from_subaccount,
                    amount,
                )
            }
        }
    }

    /********************** ICRC-1 METHODS ***********************/

    #[query(trait = true)]
    fn icrc1_balance_of(&self, account: Account) -> Tokens128 {
        self.state().borrow().balances.balance_of(account.into())
    }

    #[cfg_attr(feature = "transfer", update(trait = true))]
    fn icrc1_transfer(&self, transfer: TransferArgs) -> Result<u128, TransferError> {
        let account = CheckedAccount::with_recipient(transfer.to.into(), transfer.from_subaccount)?;

        Ok(icrc1_transfer(
            &mut self.state().borrow_mut(),
            account,
            &transfer,
            self.fee_ratio(),
        )?)
    }

    #[query(trait = true)]
    fn icrc1_name(&self) -> String {
        StatsData::get_stable().name
    }

    #[query(trait = true)]
    fn icrc1_symbol(&self) -> String {
        StatsData::get_stable().symbol
    }

    #[query(trait = true)]
    fn icrc1_decimals(&self) -> u8 {
        StatsData::get_stable().decimals
    }

    /// Returns the default transfer fee.
    #[query(trait = true)]
    fn icrc1_fee(&self) -> Tokens128 {
        StatsData::get_stable().fee
    }
    #[query(trait = true)]
    fn icrc1_metadata(&self) -> Vec<(String, Value)> {
        self.state().borrow().icrc1_metadata()
    }

    #[query(trait = true)]
    fn icrc1_supported_standards(&self) -> Vec<StandardRecord> {
        StatsData::get_stable().supported_standards()
    }

    #[query(trait = true)]
    fn icrc1_minting_account(&self) -> Option<Account> {
        Some(StatsData::get_stable().owner.into())
    }

    /********************** INTERNAL METHODS ***********************/

    // Important: This function *must* be defined to be the
    // last one in the trait because it depends on the order
    // of expansion of update/query(trait = true) methods.
    fn get_idl() -> Idl {
        generate_idl!()
    }

    fn update_stats(&self, _caller: CheckedPrincipal<Owner>, update: CanisterUpdate) {
        use CanisterUpdate::*;
        let mut stats = StatsData::get_stable();
        match update {
            Name(name) => stats.name = name,
            Symbol(symbol) => stats.symbol = symbol,
            Logo(logo) => stats.logo = logo,
            Fee(fee) => stats.fee = fee,
            FeeTo(fee_to) => stats.fee_to = fee_to,
            Owner(owner) => stats.owner = owner,
            MinCycles(min_cycles) => stats.min_cycles = min_cycles,
        }
        StatsData::set_stable(stats)
    }

    fn fee_ratio(&self) -> f64 {
        #[cfg(feature = "auction")]
        return self.bidding_info().fee_ratio;

        #[cfg(not(feature = "auction"))]
        0.0
    }
}

generate_exports!(TokenCanisterAPI, TokenCanisterExports);

#[cfg(feature = "auction")]
impl Auction for TokenCanisterExports {
    fn auction_state(&self) -> Rc<RefCell<AuctionState>> {
        AuctionState::get()
    }

    fn disburse_rewards(&self) -> Result<AuctionInfo, AuctionError> {
        is20_auction::disburse_rewards(
            &mut self.state().borrow_mut(),
            &self.auction_state().borrow(),
        )
    }
}

pub fn auction_account() -> AccountInternal {
    // There are no sub accounts for the auction principal
    AccountInternal::new(Principal::management_canister(), None)
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "claim")]
    use canister_sdk::ledger_canister::{AccountIdentifier, Subaccount as SubaccountIdentifier};
    use canister_sdk::{
        ic_canister::canister_call,
        ic_kit::{
            mock_principals::{alice, bob, john},
            MockContext,
        },
    };
    use rand::{thread_rng, Rng};

    use crate::mock::TokenCanisterMock;
    use crate::{account::DEFAULT_SUBACCOUNT, state::stats::Metadata};

    use super::*;

    // Method for generating random Subaccount.
    #[cfg(feature = "claim")]
    #[cfg_attr(coverage_nightly, no_coverage)]
    fn gen_subaccount() -> Subaccount {
        let mut subaccount = [0u8; 32];
        thread_rng().fill(&mut subaccount);
        subaccount
    }

    #[cfg_attr(coverage_nightly, no_coverage)]
    fn test_context() -> (&'static MockContext, TokenCanisterMock) {
        let context = MockContext::new().with_caller(john()).inject();

        let canister = TokenCanisterMock::init_instance();
        canister.init(
            Metadata {
                logo: "".to_string(),
                name: "".to_string(),
                symbol: "".to_string(),
                decimals: 8,

                owner: john(),
                fee: Tokens128::from(0),
                fee_to: john(),
                is_test_token: None,
            },
            Tokens128::from(1000),
        );

        // This is to make tests that don't rely on auction state
        // pass, because since we are running auction state on each
        // endpoint call, it affects `BiddingInfo.fee_ratio` that is
        // used for charging fees in `approve` endpoint.
        let mut stats = StatsData::get_stable();
        stats.min_cycles = 0;
        StatsData::set_stable(stats);

        canister.mint(alice(), None, 1000.into()).unwrap();
        context.update_caller(alice());

        (context, canister)
    }

    fn test_canister() -> TokenCanisterMock {
        MockContext::new().with_caller(alice()).inject();

        let canister = TokenCanisterMock::init_instance();
        canister.init(
            Metadata {
                logo: "".to_string(),
                name: "".to_string(),
                symbol: "".to_string(),
                decimals: 8,
                owner: alice(),
                fee: Tokens128::from(0),
                fee_to: alice(),
                is_test_token: None,
            },
            Tokens128::from(1000),
        );

        let mut stats = StatsData::get_stable();
        stats.min_cycles = 0;
        StatsData::set_stable(stats);

        canister
    }

    #[test]
    fn transfer_to_same_account() {
        let canister = test_canister();
        let transfer = TransferArgs {
            from_subaccount: None,
            to: alice().into(),
            amount: 100.into(),
            fee: None,
            memo: None,
            created_at_time: None,
        };

        let res = canister.icrc1_transfer(transfer);
        assert_eq!(
            res,
            Err(TransferError::GenericError {
                error_code: 500,
                message: "self transfer".into()
            })
        )
    }

    #[test]
    fn transfer_to_same_default_subaccount() {
        let canister = test_canister();
        let transfer = TransferArgs {
            from_subaccount: Some(crate::account::DEFAULT_SUBACCOUNT),
            to: alice().into(),
            amount: 100.into(),
            fee: None,
            memo: None,
            created_at_time: None,
        };

        let res = canister.icrc1_transfer(transfer);
        assert_eq!(
            res,
            Err(TransferError::GenericError {
                error_code: 500,
                message: "self transfer".into()
            })
        );

        let transfer = TransferArgs {
            from_subaccount: None,
            to: Account::new(alice(), Some(DEFAULT_SUBACCOUNT)),
            amount: 100.into(),
            fee: None,
            memo: None,
            created_at_time: None,
        };

        let res = canister.icrc1_transfer(transfer);
        assert_eq!(
            res,
            Err(TransferError::GenericError {
                error_code: 500,
                message: "self transfer".into()
            })
        );
    }

    #[cfg(feature = "claim")]
    #[test]
    fn test_claim() {
        let bob_sub = gen_subaccount();
        let alice_sub = gen_subaccount();

        let alice_aid =
            AccountIdentifier::new(alice().into(), Some(SubaccountIdentifier(alice_sub)));
        let bob_aid = AccountIdentifier::new(bob().into(), Some(SubaccountIdentifier(bob_sub)));

        let (ctx, canister) = test_context();
        ctx.update_caller(john());

        assert!(canister
            .mint(
                canister.owner(),
                Some(alice_aid.to_address()),
                Tokens128::from(1000)
            )
            .is_ok());
        assert!(canister
            .mint(
                canister.owner(),
                Some(bob_aid.to_address()),
                Tokens128::from(2000)
            )
            .is_ok());

        ctx.update_caller(alice());
        assert_eq!(
            canister.get_claimable_amount(canister.owner(), Some(alice_sub)),
            Tokens128::from(1000)
        );

        let balance_before = canister.icrc1_balance_of(alice().into());
        canister.claim(canister.owner(), Some(alice_sub)).unwrap();
        assert_eq!(
            canister.icrc1_balance_of(alice().into()),
            (Tokens128::from(1000) + balance_before).unwrap()
        );
        assert_eq!(
            canister.get_claimable_amount(canister.owner(), Some(alice_sub)),
            0.into()
        );

        ctx.update_caller(bob());
        assert_eq!(
            canister.get_claimable_amount(canister.owner(), Some(bob_sub)),
            Tokens128::from(2000)
        );
    }

    // **** APIs tests ****

    #[tokio::test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    async fn set_name() {
        let (ctx, canister) = test_context();
        ctx.update_id(john());
        canister_call!(canister.set_name("War and Piece".to_string()), Result<(), TxError>)
            .await
            .unwrap()
            .unwrap();
        let info = canister_call!(canister.get_token_info(), TokenInfo)
            .await
            .unwrap();

        assert_eq!(info.metadata.name, "War and Piece".to_string());

        ctx.update_id(bob());
        let res = canister_call!(canister.set_name("Crime and Punishment".to_string()), Result<(), TxError>)
            .await
            .unwrap();

        assert_eq!(res, Err(TxError::Unauthorized));
        let info = canister_call!(canister.get_token_info(), TokenInfo)
            .await
            .unwrap();

        assert_eq!(info.metadata.name, "War and Piece".to_string());
        let name = canister_call!(canister.icrc1_name(), String).await.unwrap();
        assert_eq!(name, "War and Piece".to_string());
    }

    #[tokio::test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    async fn set_symbol() {
        let (ctx, canister) = test_context();
        ctx.update_id(john());
        canister_call!(canister.set_symbol("MAX".to_string()), Result<(), TxError>)
            .await
            .unwrap()
            .unwrap();
        let info = canister_call!(canister.get_token_info(), TokenInfo)
            .await
            .unwrap();

        assert_eq!(info.metadata.symbol, "MAX".to_string());

        ctx.update_id(bob());
        let res = canister_call!(canister.set_symbol("BOB".to_string()), Result<(), TxError>)
            .await
            .unwrap();

        assert_eq!(res, Err(TxError::Unauthorized));
        let info = canister_call!(canister.get_token_info(), TokenInfo)
            .await
            .unwrap();

        assert_eq!(info.metadata.symbol, "MAX".to_string());
        let symbol = canister_call!(canister.icrc1_symbol(), String)
            .await
            .unwrap();
        assert_eq!(symbol, "MAX".to_string());
    }

    #[tokio::test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    async fn set_logo() {
        let (ctx, canister) = test_context();
        ctx.update_id(john());
        canister_call!(canister.set_logo("1".to_string()), Result<(), TxError>)
            .await
            .unwrap()
            .unwrap();
        let info = canister_call!(canister.get_token_info(), TokenInfo)
            .await
            .unwrap();

        assert_eq!(info.metadata.logo, "1".to_string());

        ctx.update_id(bob());
        let res = canister_call!(canister.set_logo("2".to_string()), Result<(), TxError>)
            .await
            .unwrap();

        assert_eq!(res, Err(TxError::Unauthorized));
        let info = canister_call!(canister.get_token_info(), TokenInfo)
            .await
            .unwrap();

        assert_eq!(info.metadata.logo, "1".to_string());

        let logo = canister_call!(canister.logo(), String).await.unwrap();
        assert_eq!(logo, "1".to_string());
    }

    #[tokio::test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    async fn set_fee() {
        let (ctx, canister) = test_context();
        ctx.update_id(john());
        canister_call!(canister.set_fee(100500.into()), Result<(), TxError>)
            .await
            .unwrap()
            .unwrap();
        let info = canister_call!(canister.get_token_info(), TokenInfo)
            .await
            .unwrap();

        assert_eq!(info.metadata.fee, 100500.into());

        ctx.update_id(bob());
        let res = canister_call!(canister.set_fee(0.into()), Result<(), TxError>)
            .await
            .unwrap();

        assert_eq!(res, Err(TxError::Unauthorized));
        let info = canister_call!(canister.get_token_info(), TokenInfo)
            .await
            .unwrap();

        assert_eq!(info.metadata.fee, 100500.into());
        let fee = canister_call!(canister.icrc1_fee(), Tokens128)
            .await
            .unwrap();
        assert_eq!(fee, 100500.into());
    }

    #[tokio::test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    async fn set_fee_to() {
        let (ctx, canister) = test_context();
        ctx.update_id(john());
        canister_call!(canister.set_fee_to(alice()), Result<(), TxError>)
            .await
            .unwrap()
            .unwrap();
        let info = canister_call!(canister.get_token_info(), TokenInfo)
            .await
            .unwrap();

        assert_eq!(info.metadata.fee_to, alice());

        ctx.update_id(bob());
        let res = canister_call!(canister.set_fee_to(bob()), Result<(), TxError>)
            .await
            .unwrap();

        assert_eq!(res, Err(TxError::Unauthorized));
        let info = canister_call!(canister.get_token_info(), TokenInfo)
            .await
            .unwrap();

        assert_eq!(info.metadata.fee_to, alice());
    }

    #[tokio::test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    async fn set_owner() {
        let (ctx, canister) = test_context();
        ctx.update_id(john());
        canister_call!(canister.set_owner(alice()), Result<(), TxError>)
            .await
            .unwrap()
            .unwrap();
        let info = canister_call!(canister.get_token_info(), TokenInfo)
            .await
            .unwrap();

        assert_eq!(info.metadata.owner, alice());

        ctx.update_id(bob());
        let res = canister_call!(canister.set_owner(bob()), Result<(), TxError>)
            .await
            .unwrap();

        assert_eq!(res, Err(TxError::Unauthorized));
        let info = canister_call!(canister.get_token_info(), TokenInfo)
            .await
            .unwrap();

        assert_eq!(info.metadata.owner, alice());
        let owner = canister_call!(canister.owner(), Principal).await.unwrap();
        assert_eq!(owner, alice());

        let minting_account = canister_call!(canister.icrc1_minting_account(), Principal)
            .await
            .unwrap();
        assert_eq!(minting_account, Some(alice().into()));
    }

    #[tokio::test]
    #[cfg_attr(coverage_nightly, no_coverage)]
    async fn list_subaccounts() {
        let canister = test_canister();
        let subaccount: Subaccount = [1; 32];
        canister
            .transfer(TransferArgs {
                from_subaccount: None,
                to: Account::new(alice(), Some(subaccount)),
                amount: 100.into(),
                fee: None,
                memo: None,
                created_at_time: None,
            })
            .unwrap();

        let list = canister_call!(canister.list_subaccounts(), std::collections::HashMap<Subaccount, Tokens128>).await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[&DEFAULT_SUBACCOUNT], 900.into());
        assert_eq!(list[&subaccount], 100.into());
    }
}
