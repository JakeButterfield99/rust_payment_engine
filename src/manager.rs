use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::HashMap;

use crate::TransactionRecord;
use crate::utils;

#[derive(Debug, Serialize)]
pub struct UserAccount {
    available: Decimal,
    held: Decimal,
    locked: bool,
}

pub struct UserExport {
    pub client: u16,
    pub available: Decimal,
    pub held: Decimal,
    pub total: Decimal,
    pub locked: bool,
}

pub struct DepositRecord {
    amount: Decimal,
    is_disputed: bool,
}

pub struct AccountsManager {
    accounts: HashMap<u16, UserAccount>,
    transactions: HashMap<u32, DepositRecord>,
    tx_keys: Vec<String>,
}

impl AccountsManager {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            transactions: HashMap::new(),
            tx_keys: Vec::new(),
        }
    }

    /* --- Account Management --- */

    fn get_account(&mut self, user_id: u16) -> &mut UserAccount {
        self.accounts.entry(user_id).or_insert(UserAccount {
            available: Decimal::new(0, 0),
            held: Decimal::new(0, 0),
            locked: false,
        })
    }

    fn set_available_funds(&mut self, user_id: u16, amount: Decimal) {
        let account = self.get_account(user_id);
        account.available = amount;
    }

    fn set_held_funds(&mut self, user_id: u16, amount: Decimal) {
        let account = self.get_account(user_id);
        account.held = amount;
    }

    fn set_locked(&mut self, user_id: u16, locked: bool) {
        let account = self.get_account(user_id);
        account.locked = locked;
    }

    // Convert accounts into export format and return
    pub fn export_accounts(&self) -> Vec<UserExport> {
        let mut exported = Vec::new();
        for (client, account) in &self.accounts {
            let total = account.available + account.held;
            exported.push(UserExport {
                client: *client,
                available: account.available,
                held: account.held,
                total,
                locked: account.locked,
            });
        }
        exported
    }

    #[cfg(test)]
    pub fn get_available_funds(&self, user_id: u16) -> Decimal {
        self.accounts
            .get(&user_id)
            .map_or(Decimal::new(0, 0), |account| account.available)
    }

    #[cfg(test)]
    pub fn get_held_funds(&self, user_id: u16) -> Decimal {
        self.accounts
            .get(&user_id)
            .map_or(Decimal::new(0, 0), |account| account.held)
    }

    #[cfg(test)]
    pub fn get_locked_status(&self, user_id: u16) -> bool {
        self.accounts
            .get(&user_id)
            .map_or(false, |account| account.locked)
    }

    /* --- Transaction Tracking --- */

    // Encode transaction and ensure we haven't seen it before
    fn is_duplicate(&self, record: &TransactionRecord) -> bool {
        let tx_key = utils::get_transaction_key(&record.typ, record.client, record.tx);
        self.tx_keys.contains(&tx_key)
    }

    // Encode transaction and keep a record of it
    fn log_transaction(&mut self, record: &TransactionRecord) {
        let tx_key = utils::get_transaction_key(&record.typ, record.client, record.tx);
        self.tx_keys.push(tx_key);
    }

    // Keep a log of a deposit transaction
    fn log_deposit(&mut self, tx: u32, amount: Decimal) {
        self.transactions.insert(
            tx,
            DepositRecord {
                amount,
                is_disputed: false,
            },
        );
    }

    fn get_deposit(&self, tx: u32) -> Option<&DepositRecord> {
        self.transactions.get(&tx)
    }

    fn is_disputed(&self, record: &DepositRecord) -> bool {
        record.is_disputed
    }

    fn mark_disputed(&mut self, tx: u32) {
        if let Some(deposit) = self.transactions.get_mut(&tx) {
            deposit.is_disputed = true;
        }
    }

    fn mark_resolved(&mut self, tx: u32) {
        if let Some(deposit) = self.transactions.get_mut(&tx) {
            deposit.is_disputed = false;
        }
    }

    /* --- Transaction Handlers --- */

    fn handle_deposit(&mut self, record: TransactionRecord) {
        let account = self.get_account(record.client);
        let amount = utils::parse_amount(record.amount);
        let new_amount = account.available + amount;

        self.set_available_funds(record.client, new_amount);
        self.log_deposit(record.tx, amount);
    }

    fn handle_withdrawal(&mut self, record: TransactionRecord) {
        let account = self.get_account(record.client);
        let amount = utils::parse_amount(record.amount);

        // Ensure user can afford this withdrawal
        if !utils::can_afford(account.available, amount) {
            return;
        }

        let new_amount = account.available - amount;
        self.set_available_funds(record.client, new_amount);
    }

    fn handle_dispute(&mut self, record: TransactionRecord) {
        // Ensure transaction exists
        let Some(deposit) = self.get_deposit(record.tx) else {
            return;
        };

        // Ensure this is not already disputed
        if self.is_disputed(deposit) {
            return;
        }

        let amount = deposit.amount;
        let account = self.get_account(record.client);

        // Ensure user has enough available funds
        if !utils::can_afford(account.available, amount) {
            return;
        }

        let new_available = account.available - amount;
        let new_held = account.held + amount;
        self.set_available_funds(record.client, new_available);
        self.set_held_funds(record.client, new_held);

        self.mark_disputed(record.tx);
    }

    fn handle_resolve(&mut self, record: TransactionRecord) {
        // Ensure transaction exists
        let Some(deposit) = self.get_deposit(record.tx) else {
            return;
        };

        // Ensure this is a disputed transaction
        if !self.is_disputed(deposit) {
            return;
        }

        let amount = deposit.amount;
        let account = self.get_account(record.client);

        // Ensure user has enough held funds
        if !utils::can_afford(account.held, amount) {
            return;
        }

        let new_held = account.held - amount;
        let new_available = account.available + amount;
        self.set_held_funds(record.client, new_held);
        self.set_available_funds(record.client, new_available);

        self.mark_resolved(record.tx);
    }

    fn handle_chargeback(&mut self, record: TransactionRecord) {
        // Ensure transaction exists
        let Some(deposit) = self.get_deposit(record.tx) else {
            return;
        };

        // Ensure this is a disputed transaction
        if !self.is_disputed(deposit) {
            return;
        }

        let amount = deposit.amount;
        let account = self.get_account(record.client);

        // Ensure user has enough held funds
        if !utils::can_afford(account.held, amount) {
            return;
        }

        let new_held = account.held - amount;
        self.set_held_funds(record.client, new_held);
        self.set_locked(record.client, true);

        self.mark_resolved(record.tx);
    }

    pub fn handle_transaction(&mut self, record: TransactionRecord) {
        // Ensure this is not a duplicate transaction
        if self.is_duplicate(&record) {
            return;
        }

        // Keep a log of this transaction
        self.log_transaction(&record);

        // Ensure account is not locked
        let is_locked = self.get_account(record.client).locked;
        if is_locked {
            return;
        }

        // Parse transaction type and handle accordingly
        match record.typ.as_str() {
            "deposit" => self.handle_deposit(record),
            "withdrawal" => self.handle_withdrawal(record),
            "dispute" => self.handle_dispute(record),
            "resolve" => self.handle_resolve(record),
            "chargeback" => self.handle_chargeback(record),
            _ => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use rust_decimal::prelude::FromPrimitive;

    fn create_transaction(
        typ: &str,
        client: u16,
        tx: u32,
        amount: Option<f32>,
    ) -> TransactionRecord {
        TransactionRecord {
            typ: typ.to_string(),
            client,
            tx,
            amount,
        }
    }

    // Helper to convert f32 to rounded Decimal (mimics parse_amount)
    fn d(val: f32) -> Decimal {
        let decimal = Decimal::from_f32(val).unwrap_or(Decimal::new(0, 0));
        decimal.round_dp(4)
    }

    // Test 1: Simple deposit & withdrawal with no expected errors
    #[test]
    fn test_simple_deposit_and_withdrawal() {
        let mut manager = AccountsManager::new();
        let client_id = 1;

        // Deposit $100.5678
        manager.handle_transaction(create_transaction("deposit", client_id, 1, Some(100.5678)));
        assert_eq!(manager.get_available_funds(client_id), d(100.5678));

        // Deposit another $50.1234
        manager.handle_transaction(create_transaction("deposit", client_id, 2, Some(50.1234)));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(100.5678) + d(50.1234)
        );

        // Withdraw $30.2567
        manager.handle_transaction(create_transaction(
            "withdrawal",
            client_id,
            3,
            Some(30.2567),
        ));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(100.5678) + d(50.1234) - d(30.2567)
        );

        // Withdraw another $20.4321
        manager.handle_transaction(create_transaction(
            "withdrawal",
            client_id,
            4,
            Some(20.4321),
        ));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(100.5678) + d(50.1234) - d(30.2567) - d(20.4321)
        );
        assert_eq!(manager.get_held_funds(client_id), Decimal::ZERO);
        assert_eq!(manager.get_locked_status(client_id), false);
    }

    // Test 2: Withdrawals that exceed available funds
    #[test]
    fn test_withdrawal_exceeds_available_funds() {
        let mut manager = AccountsManager::new();
        let client_id = 2;

        // Deposit $50.7890
        manager.handle_transaction(create_transaction("deposit", client_id, 5, Some(50.7890)));
        assert_eq!(manager.get_available_funds(client_id), d(50.7890));

        // Attempt withdrawal of $75.4321 (should fail silently, no error)
        manager.handle_transaction(create_transaction(
            "withdrawal",
            client_id,
            6,
            Some(75.4321),
        ));
        assert_eq!(manager.get_available_funds(client_id), d(50.7890));
        assert_eq!(manager.get_held_funds(client_id), Decimal::ZERO);

        // Withdrawal of exact amount should succeed
        manager.handle_transaction(create_transaction(
            "withdrawal",
            client_id,
            7,
            Some(50.7890),
        ));
        assert_eq!(manager.get_available_funds(client_id), Decimal::ZERO);

        // Withdrawal from zero should fail
        manager.handle_transaction(create_transaction(
            "withdrawal",
            client_id,
            8,
            Some(10.1111),
        ));
        assert_eq!(manager.get_available_funds(client_id), Decimal::ZERO);
    }

    // Test 3: Simple dispute/resolve handling
    #[test]
    fn test_simple_dispute_and_resolve() {
        let mut manager = AccountsManager::new();
        let client_id = 3;

        // Seed several deposits and withdrawals
        manager.handle_transaction(create_transaction("deposit", client_id, 10, Some(100.2345)));
        manager.handle_transaction(create_transaction("deposit", client_id, 11, Some(75.6789)));
        manager.handle_transaction(create_transaction(
            "withdrawal",
            client_id,
            12,
            Some(25.4567),
        ));

        assert_eq!(
            manager.get_available_funds(client_id),
            d(100.2345) + d(75.6789) - d(25.4567)
        );
        assert_eq!(manager.get_held_funds(client_id), Decimal::ZERO);

        // Dispute the first deposit ($100.2345)
        manager.handle_transaction(create_transaction("dispute", client_id, 10, None));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(75.6789) - d(25.4567)
        );
        assert_eq!(manager.get_held_funds(client_id), d(100.2345));
        assert_eq!(manager.get_locked_status(client_id), false);

        // Resolve the dispute
        manager.handle_transaction(create_transaction("resolve", client_id, 10, None));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(100.2345) + d(75.6789) - d(25.4567)
        );
        assert_eq!(manager.get_held_funds(client_id), Decimal::ZERO);
        assert_eq!(manager.get_locked_status(client_id), false);
    }

    // Test 4: Simple dispute/chargeback handling
    #[test]
    fn test_simple_dispute_and_chargeback() {
        let mut manager = AccountsManager::new();
        let client_id = 4;

        // Seed several deposits and withdrawals
        manager.handle_transaction(create_transaction("deposit", client_id, 20, Some(200.3456)));
        manager.handle_transaction(create_transaction("deposit", client_id, 21, Some(100.7654)));
        manager.handle_transaction(create_transaction(
            "withdrawal",
            client_id,
            22,
            Some(50.5678),
        ));

        assert_eq!(
            manager.get_available_funds(client_id),
            d(200.3456) + d(100.7654) - d(50.5678)
        );
        assert_eq!(manager.get_held_funds(client_id), Decimal::ZERO);
        assert_eq!(manager.get_locked_status(client_id), false);

        // Dispute the first deposit ($200.3456)
        manager.handle_transaction(create_transaction("dispute", client_id, 20, None));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(100.7654) - d(50.5678)
        );
        assert_eq!(manager.get_held_funds(client_id), d(200.3456));
        assert_eq!(manager.get_locked_status(client_id), false);

        // Chargeback the disputed transaction
        manager.handle_transaction(create_transaction("chargeback", client_id, 20, None));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(100.7654) - d(50.5678)
        );
        assert_eq!(manager.get_held_funds(client_id), Decimal::ZERO);
        assert_eq!(manager.get_locked_status(client_id), true);
    }

    // Test 5: Resolve transaction with invalid tx (non-existent transaction)
    #[test]
    fn test_resolve_with_invalid_transaction() {
        let mut manager = AccountsManager::new();
        let client_id = 5;

        // Deposit some funds
        manager.handle_transaction(create_transaction("deposit", client_id, 30, Some(100.9876)));
        assert_eq!(manager.get_available_funds(client_id), d(100.9876));
        assert_eq!(manager.get_held_funds(client_id), Decimal::ZERO);

        // Attempt to resolve a transaction that doesn't exist (tx 999)
        manager.handle_transaction(create_transaction("resolve", client_id, 999, None));

        // Funds should remain unchanged
        assert_eq!(manager.get_available_funds(client_id), d(100.9876));
        assert_eq!(manager.get_held_funds(client_id), Decimal::ZERO);
        assert_eq!(manager.get_locked_status(client_id), false);
    }

    // Test 6: Resolve transaction with no associated dispute
    #[test]
    fn test_resolve_with_no_associated_dispute() {
        let mut manager = AccountsManager::new();
        let client_id = 6;

        // Deposit funds
        manager.handle_transaction(create_transaction("deposit", client_id, 40, Some(100.1111)));
        assert_eq!(manager.get_available_funds(client_id), d(100.1111));

        // Attempt to resolve without first disputing
        manager.handle_transaction(create_transaction("resolve", client_id, 40, None));

        // Funds should remain unchanged (resolve should fail silently)
        assert_eq!(manager.get_available_funds(client_id), d(100.1111));
        assert_eq!(manager.get_held_funds(client_id), Decimal::ZERO);
        assert_eq!(manager.get_locked_status(client_id), false);
    }

    // Test 7: Chargeback transaction with invalid tx (non-existent transaction)
    #[test]
    fn test_chargeback_with_invalid_transaction() {
        let mut manager = AccountsManager::new();
        let client_id = 7;

        // Deposit some funds
        manager.handle_transaction(create_transaction("deposit", client_id, 50, Some(150.2222)));
        assert_eq!(manager.get_available_funds(client_id), d(150.2222));

        // Attempt chargeback on non-existent transaction (tx 888)
        manager.handle_transaction(create_transaction("chargeback", client_id, 888, None));

        // Funds should remain unchanged and account should not be locked
        assert_eq!(manager.get_available_funds(client_id), d(150.2222));
        assert_eq!(manager.get_held_funds(client_id), Decimal::ZERO);
        assert_eq!(manager.get_locked_status(client_id), false);
    }

    // Test 8: Chargeback transaction with no associated dispute
    #[test]
    fn test_chargeback_with_no_associated_dispute() {
        let mut manager = AccountsManager::new();
        let client_id = 8;

        // Deposit funds
        manager.handle_transaction(create_transaction("deposit", client_id, 60, Some(100.3333)));
        assert_eq!(manager.get_available_funds(client_id), d(100.3333));

        // Attempt chargeback without first disputing
        manager.handle_transaction(create_transaction("chargeback", client_id, 60, None));

        // Funds should remain unchanged and account should not be locked
        assert_eq!(manager.get_available_funds(client_id), d(100.3333));
        assert_eq!(manager.get_held_funds(client_id), Decimal::ZERO);
        assert_eq!(manager.get_locked_status(client_id), false);
    }

    // Test 9: Rounding verification with 5-6 decimal inputs
    #[test]
    fn test_rounding_more_than_4_decimals() {
        let mut manager = AccountsManager::new();
        let client_id = 10;

        // Deposit with 5 decimals: 100.123456 should round to 100.1235
        manager.handle_transaction(create_transaction(
            "deposit",
            client_id,
            80,
            Some(100.123456),
        ));
        assert_eq!(manager.get_available_funds(client_id), d(100.123456));

        // Deposit with 6 decimals: 50.567899 should round to 50.5679
        manager.handle_transaction(create_transaction(
            "deposit",
            client_id,
            81,
            Some(50.567899),
        ));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(100.123456) + d(50.567899)
        );

        // Withdrawal with 5 decimals: 25.987654 should round to 25.9877
        manager.handle_transaction(create_transaction(
            "withdrawal",
            client_id,
            82,
            Some(25.987654),
        ));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(100.123456) + d(50.567899) - d(25.987654)
        );

        // Deposit a value that rounds down: 10.123444 should round to 10.1234
        manager.handle_transaction(create_transaction(
            "deposit",
            client_id,
            83,
            Some(10.123444),
        ));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(100.123456) + d(50.567899) - d(25.987654) + d(10.123444)
        );

        // Deposit a value with many decimals: 5.555555 should round to 5.5556
        manager.handle_transaction(create_transaction("deposit", client_id, 84, Some(5.555555)));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(100.123456) + d(50.567899) - d(25.987654) + d(10.123444) + d(5.555555)
        );
    }

    // Test 10: Simple chargeback without other disputes
    #[test]
    fn test_simple_chargeback_only() {
        let mut manager = AccountsManager::new();
        let client_id = 11;

        // Single deposit
        manager.handle_transaction(create_transaction("deposit", client_id, 90, Some(100.1234)));
        assert_eq!(manager.get_available_funds(client_id), d(100.1234));
        assert_eq!(manager.get_held_funds(client_id), Decimal::ZERO);
        assert_eq!(manager.get_locked_status(client_id), false);

        // Dispute it
        manager.handle_transaction(create_transaction("dispute", client_id, 90, None));
        assert_eq!(manager.get_available_funds(client_id), Decimal::ZERO);
        assert_eq!(manager.get_held_funds(client_id), d(100.1234));
        assert_eq!(manager.get_locked_status(client_id), false);

        // Chargeback it
        manager.handle_transaction(create_transaction("chargeback", client_id, 90, None));
        assert_eq!(manager.get_available_funds(client_id), Decimal::ZERO);
        assert_eq!(manager.get_held_funds(client_id), Decimal::ZERO);
        assert_eq!(manager.get_locked_status(client_id), true);
    }

    // Test 11: Multiple deposits with different decimal values
    #[test]
    fn test_multiple_deposits_and_withdrawals() {
        let mut manager = AccountsManager::new();
        let client_id = 9;

        // Seed multiple deposits with varying 4+ decimal places
        manager.handle_transaction(create_transaction("deposit", client_id, 70, Some(100.4321)));
        manager.handle_transaction(create_transaction("deposit", client_id, 71, Some(100.8765)));
        manager.handle_transaction(create_transaction("deposit", client_id, 72, Some(100.2468)));

        let total_deposits = d(100.4321) + d(100.8765) + d(100.2468);
        assert_eq!(manager.get_available_funds(client_id), total_deposits);
        assert_eq!(manager.get_held_funds(client_id), Decimal::ZERO);

        // Withdraw from total with 4 decimals
        manager.handle_transaction(create_transaction(
            "withdrawal",
            client_id,
            73,
            Some(50.1234),
        ));
        let after_withdrawal = total_deposits - d(50.1234);
        assert_eq!(manager.get_available_funds(client_id), after_withdrawal);

        // Dispute one transaction (tx 70)
        manager.handle_transaction(create_transaction("dispute", client_id, 70, None));
        let after_dispute = after_withdrawal - d(100.4321);
        assert_eq!(manager.get_available_funds(client_id), after_dispute);
        assert_eq!(manager.get_held_funds(client_id), d(100.4321));

        // Resolve it
        manager.handle_transaction(create_transaction("resolve", client_id, 70, None));
        assert_eq!(manager.get_available_funds(client_id), after_withdrawal);
        assert_eq!(manager.get_held_funds(client_id), Decimal::ZERO);
    }

    // Test 12: Duplicate deposits should only be processed once
    #[test]
    fn test_duplicate_deposits_only_process_once() {
        let mut manager = AccountsManager::new();
        let client_id = 12;

        // Deposit $100.5678 with tx ID 100
        manager.handle_transaction(create_transaction(
            "deposit",
            client_id,
            100,
            Some(100.5678),
        ));
        assert_eq!(manager.get_available_funds(client_id), d(100.5678));

        // Attempt to deposit the same transaction again (same type, client, tx ID)
        manager.handle_transaction(create_transaction(
            "deposit",
            client_id,
            100,
            Some(100.5678),
        ));

        // Balance should still be $100.5678, not $201.1356 (duplicate should be rejected)
        assert_eq!(
            manager.get_available_funds(client_id),
            d(100.5678),
            "Duplicate deposit should not be processed; balance should remain at single deposit amount"
        );

        // Attempt duplicate again with a different amount (should still be rejected)
        manager.handle_transaction(create_transaction("deposit", client_id, 100, Some(50.0000)));

        // Balance should still be $100.5678 (duplicate rejected regardless of amount)
        assert_eq!(
            manager.get_available_funds(client_id),
            d(100.5678),
            "Duplicate deposit with different amount should still be rejected"
        );

        // A different transaction (different tx ID) should succeed
        manager.handle_transaction(create_transaction("deposit", client_id, 101, Some(50.1234)));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(100.5678) + d(50.1234),
            "Non-duplicate deposit with different tx ID should be processed"
        );

        // Duplicate of tx 101 should be rejected
        manager.handle_transaction(create_transaction("deposit", client_id, 101, Some(50.1234)));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(100.5678) + d(50.1234),
            "Duplicate deposit of tx 101 should be rejected"
        );
    }

    // Test 13: Locked account should reject all subsequent transactions
    #[test]
    fn test_locked_account_rejects_transactions() {
        let mut manager = AccountsManager::new();
        let client_id = 13;

        // Initial deposits
        manager.handle_transaction(create_transaction(
            "deposit",
            client_id,
            200,
            Some(500.0000),
        ));
        manager.handle_transaction(create_transaction(
            "deposit",
            client_id,
            201,
            Some(250.5678),
        ));
        let initial_balance = d(500.0000) + d(250.5678);
        assert_eq!(manager.get_available_funds(client_id), initial_balance);
        assert_eq!(manager.get_held_funds(client_id), Decimal::ZERO);
        assert_eq!(manager.get_locked_status(client_id), false);

        // Dispute and chargeback to lock the account
        manager.handle_transaction(create_transaction("dispute", client_id, 200, None));
        assert_eq!(manager.get_held_funds(client_id), d(500.0000));
        assert_eq!(manager.get_locked_status(client_id), false);

        manager.handle_transaction(create_transaction("chargeback", client_id, 200, None));
        assert_eq!(manager.get_available_funds(client_id), d(250.5678));
        assert_eq!(manager.get_held_funds(client_id), Decimal::ZERO);
        assert_eq!(
            manager.get_locked_status(client_id),
            true,
            "Account should be locked after chargeback"
        );

        // Try to deposit - should be ignored
        manager.handle_transaction(create_transaction(
            "deposit",
            client_id,
            202,
            Some(100.1111),
        ));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(250.5678),
            "Deposit on locked account should be ignored"
        );

        // Try to withdraw - should be ignored
        manager.handle_transaction(create_transaction(
            "withdrawal",
            client_id,
            203,
            Some(50.0000),
        ));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(250.5678),
            "Withdrawal on locked account should be ignored"
        );

        // Try to dispute an existing transaction - should be ignored
        manager.handle_transaction(create_transaction("dispute", client_id, 201, None));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(250.5678),
            "Available funds should remain unchanged after dispute on locked account"
        );
        assert_eq!(
            manager.get_held_funds(client_id),
            Decimal::ZERO,
            "Held funds should remain unchanged after dispute on locked account"
        );

        // Try to resolve a dispute - should be ignored
        manager.handle_transaction(create_transaction("resolve", client_id, 201, None));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(250.5678),
            "Resolve on locked account should be ignored"
        );

        // Try another chargeback - should be ignored
        manager.handle_transaction(create_transaction("chargeback", client_id, 201, None));
        assert_eq!(
            manager.get_available_funds(client_id),
            d(250.5678),
            "Chargeback on locked account should be ignored"
        );

        // Verify account is still locked
        assert_eq!(
            manager.get_locked_status(client_id),
            true,
            "Account should remain locked"
        );
        assert_eq!(
            manager.get_held_funds(client_id),
            Decimal::ZERO,
            "Held funds should remain zero"
        );
    }
}
