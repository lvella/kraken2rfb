mod exchange_rate;
mod report;
mod kraken;

use chrono::NaiveDate;
use kraken::fetch_kraken_activity;

fn main() {
    let (deposits, withdrawals, trades) = fetch_kraken_activity(
        NaiveDate::from_ymd(2025, 3, 1),
        NaiveDate::from_ymd(2025, 5, 31),
        "kraken_keys.json"
    );
    println!("Deposits: {:#?}", deposits);
    println!("Withdrawals: {:#?}", withdrawals);
    println!("Trades: {:#?}", trades);
}
