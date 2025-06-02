mod exchange_rate;
mod kraken;
mod report;

use chrono::NaiveDate;
use kraken::fetch_kraken_activity;

fn main() {
    let (deposits, withdrawals, trades) = fetch_kraken_activity(
        NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
        NaiveDate::from_ymd_opt(2025, 5, 31).unwrap(),
        "kraken_keys.json",
    );
    println!("Deposits: {:#?}", deposits);
    println!("Withdrawals: {:#?}", withdrawals);
    println!("Trades: {:#?}", trades);
}
