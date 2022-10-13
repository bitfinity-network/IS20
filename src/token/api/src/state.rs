pub mod balances;
pub mod config;
pub mod ledger;

/// Clear all canister stable memory state.
///
/// May be useful to refresh global state between tests, for example.
pub fn clear() {
    ledger::LedgerData::clear();
}
