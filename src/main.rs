mod exchange_rate;
mod kraken;
mod report;

use chrono::NaiveDate;
use kraken::fetch_kraken_activity;
use report::process_kraken_data;
use rust_decimal::Decimal;
use serde_json::Value;

fn to_decimal(value: &Value) -> Decimal {
    Decimal::try_from(value.as_number().unwrap().as_str()).unwrap()
}

fn main() {
    let (deposits, withdrawals, trades) = fetch_kraken_activity(
        NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
        NaiveDate::from_ymd_opt(2025, 5, 31).unwrap(),
        "kraken_keys.json",
    );
    println!("Deposits: {:#?}", deposits);
    println!("Withdrawals: {:#?}", withdrawals);
    println!("Trades: {:#?}", trades);

    let transactions = process_kraken_data(deposits, withdrawals, trades);
    println!("============\nTransactions: {:#?}", transactions);
}
