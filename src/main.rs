mod exchange_rate;
mod kraken;
mod kraken_pairs;
mod kraken_symbols;
mod report;

use chrono::NaiveDate;
use kraken::fetch_kraken_activity;
use report::process_kraken_data;
use rust_decimal::Decimal;
use serde_json::Value;

use crate::report::{generate_report, transactions::Transaction};

fn to_decimal(value: &Value) -> Decimal {
    Decimal::try_from(value.as_number().unwrap().as_str()).unwrap()
}

fn main() {
    // Comman line is like:
    // ./generate_report <year> <month> <report_file>
    // where <year> and <month> are used to fetch data from Kraken API
    // and <report_file> is the output file for the report.
    let mut args = std::env::args();
    let command = args.next().unwrap();
    if std::env::args().len() < 4 {
        eprintln!("Usage: {command} <year> <month> <report_file>");
        return;
    }

    let year = args.next().unwrap();
    let month = args.next().unwrap();
    let report_file = args.next().unwrap();
    println!(
        "Generating report for year: {}, month: {}, report file: {}",
        year, month, report_file,
    );

    let year: i32 = year.parse().expect("Invalid year");
    let month: u32 = month.parse().expect("Invalid month");

    let first_day = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let last_day = {
        // Get the last day of the month by creating the first day of the next month and subtracting one day
        let next_month = if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
        } else {
            NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
        };
        next_month.pred_opt().unwrap()
    };

    println!(
        "Fetching Kraken activity from {} to {}",
        first_day, last_day
    );
    let (deposits, withdrawals, trades) =
        fetch_kraken_activity(first_day, last_day, "kraken_keys.json");
    println!("Deposits: {:#?}", deposits);
    println!("Withdrawals: {:#?}", withdrawals);
    println!("Trades: {:#?}", trades);

    let transactions = process_kraken_data(deposits, withdrawals, trades);

    let mut brl_spent_in_purchases = Decimal::ZERO;
    for t in &transactions {
        if let Transaction::Purchase(purchase) = t {
            brl_spent_in_purchases += purchase.operation_value;
            if let Some(fee) = purchase.base.operation_fees {
                brl_spent_in_purchases += fee;
            }
        }
    }
    println!("Total BRL spent in purchases: {}", brl_spent_in_purchases);

    //println!("============\nTransactions: {:#?}", transactions);

    // Get first command line argument as report file name
    generate_report(transactions, &report_file).expect("Failed to generate report");
}
