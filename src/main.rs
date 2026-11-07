mod manager;
mod utils;

use std::env;

use csv::{Error, ReaderBuilder};
use serde::Deserialize;

use manager::AccountsManager;

#[derive(Debug, Deserialize)]
struct TransactionRecord {
    #[serde(rename = "type")]
    typ: String,
    client: u16,
    tx: u32,
    amount: Option<f32>,
}

// Parse transaction type and apply record to account manager
fn handle_transaction(accounts_mgr: &mut AccountsManager, record: TransactionRecord) {
    match record.typ.as_str() {
        "deposit" => accounts_mgr.handle_deposit(record),
        "withdrawal" => accounts_mgr.handle_withdrawal(record),
        "dispute" => accounts_mgr.handle_dispute(record),
        "resolve" => accounts_mgr.handle_resolve(record),
        "chargeback" => accounts_mgr.handle_chargeback(record),
        _ => (),
    }
}

// Parse CSV file line-by-line, pass down any recognised transactions to be processed
fn process_csv(accounts_mgr: &mut AccountsManager, csv_file: &String) -> Result<(), Error> {
    let mut csv_reader = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(csv_file)?;

    for result in csv_reader.deserialize() {
        let trans_record: Result<TransactionRecord, Error> = result;
        match trans_record {
            Ok(record) => handle_transaction(accounts_mgr, record),
            Err(err) => println!("Error reading record {:?}", err),
        }
    }

    Ok(())
}

// Output the accounts into stdout (CSV file if supplied)
fn output_accounts(accounts_mgr: &AccountsManager) {
    let accounts = accounts_mgr.export_accounts();

    println!("client,available,held,total,locked");
    for account in accounts {
        println!(
            "{},{:.4},{:.4},{:.4},{}",
            account.client, account.available, account.held, account.total, account.locked
        );
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let csv_file = &args[1];

    let mut accounts_mgr = AccountsManager::new();

    match process_csv(&mut accounts_mgr, csv_file) {
        Ok(()) => output_accounts(&accounts_mgr),
        Err(err) => println!("Error processing transactions: {:?}", err),
    }
}
