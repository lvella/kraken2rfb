pub mod encoding;
pub mod transactions;

use std::fs::File;
use std::io::BufWriter;

use crate::kraken::is_fiat;
use crate::{exchange_rate::get_exchange_rate, kraken_pairs, to_decimal};
use chrono::DateTime;
use rust_decimal::Decimal;
use serde_json::Value;
use transactions::{
    ExchangeInfo, PurchaseTransaction, SaleTransaction, SwapTransaction, Transaction,
    TransactionBase, TransferToExchangeTransaction, WithdrawalFromExchangeTransaction,
};

/// Get the standard Kraken exchange information
fn kraken_exchange_info() -> ExchangeInfo {
    ExchangeInfo {
        name: "Kraken".to_string(),
        url: "https://www.kraken.com".to_string(),
        country: "US".to_string(),
    }
}

/// Process Kraken data into BCB report transactions
pub fn process_kraken_data(
    deposits: Vec<Value>,
    withdrawals: Vec<Value>,
    trades: Vec<Value>,
) -> Vec<Transaction> {
    let mut transactions = Vec::new();

    // Process deposits (only non-fiat)
    for deposit in deposits {
        let asset = deposit["asset"].as_str().unwrap();
        if !is_fiat(asset) {
            let amount = deposit["amount"]
                .as_str()
                .unwrap()
                .parse::<Decimal>()
                .unwrap();
            let fee = deposit["fee"].as_str().unwrap().parse::<Decimal>().unwrap();
            let time = DateTime::from_timestamp(deposit["time"].as_u64().unwrap() as i64, 0)
                .unwrap()
                .date_naive();

            let transfer = Transaction::TransferToExchange(TransferToExchangeTransaction {
                base: TransactionBase {
                    operation_date: time,
                    operation_fees: Some(fee),
                    crypto_symbol: asset.to_string(),
                    crypto_amount: amount,
                },
                origin_wallet: None,
                origin_exchange_name: None,
            });

            transactions.push(transfer);
        }
    }

    // Process withdrawals (only non-fiat)
    for withdrawal in withdrawals {
        let asset = withdrawal["asset"].as_str().unwrap();
        if !is_fiat(asset) {
            let amount = withdrawal["amount"]
                .as_str()
                .unwrap()
                .parse::<Decimal>()
                .unwrap();
            let fee = withdrawal["fee"]
                .as_str()
                .unwrap()
                .parse::<Decimal>()
                .unwrap();
            let time = DateTime::from_timestamp(withdrawal["time"].as_u64().unwrap() as i64, 0)
                .unwrap()
                .date_naive();

            // Convert fee from crypto to BRL
            let (_rate_date, brl_rate) = get_exchange_rate(time, asset).unwrap_or_else(|e| {
                panic!(
                    "Failed to get exchange rate for {} on {}: {}",
                    asset, time, e
                )
            });

            println!("### Withdrawal asset: {asset}");
            println!(
                "### Original fee: {fee} {asset}, converted fee: {} BRL",
                fee * brl_rate
            );

            let withdrawal =
                Transaction::WithdrawalFromExchange(WithdrawalFromExchangeTransaction {
                    base: TransactionBase {
                        operation_date: time,
                        operation_fees: Some(fee * brl_rate),
                        crypto_symbol: asset.to_string(),
                        crypto_amount: amount,
                    },
                    origin_exchange: kraken_exchange_info(),
                });

            transactions.push(withdrawal);
        }
    }

    // Process trades
    for trade in trades {
        let pair = trade["pair"].as_str().unwrap();
        let (base, quote) = kraken_pairs::parse_pair(pair).unwrap();
        let vol = trade["vol"].as_str().unwrap().parse::<Decimal>().unwrap(); // BASE amount
        let cost = trade["cost"].as_str().unwrap().parse::<Decimal>().unwrap(); // QUOTE amount
        let fee = trade["fee"].as_str().unwrap().parse::<Decimal>().unwrap(); // QUOTE amount
        let price = trade["price"].as_str().unwrap().parse::<Decimal>().unwrap(); // QUOTE / BASE
        let time = DateTime::from_timestamp(int_part(to_decimal(&trade["time"])), 0)
            .unwrap()
            .date_naive();
        let trade_type = trade["type"].as_str().unwrap();

        println!("### Trade pair: {pair}");

        match (is_fiat(base), is_fiat(quote)) {
            // Crypto-Fiat trade
            (false, true) => {
                // Calculate net amounts (after fees)
                let operation_value = cost - fee; // QUOTE amount
                let crypto_amount = vol - (fee / price); // BASE amount
                let (_rate_date, brl_rate /* BRL / QUOTE */) = get_exchange_rate(time, quote)
                    .unwrap_or_else(|e| {
                        panic!(
                            "Failed to get exchange rate for {} on {}: {}",
                            quote, time, e
                        )
                    });

                println!(
                    "### Original fee: {fee} {quote}, converted fee: {} BRL",
                    fee * brl_rate
                );
                println!(
                    "### Operation value: {operation_value} {quote}, converted value: {} BRL",
                    operation_value * brl_rate
                );

                match trade_type {
                    "buy" => {
                        let purchase = Transaction::Purchase(PurchaseTransaction {
                            base: TransactionBase {
                                operation_date: time,
                                operation_fees: Some(fee * brl_rate),
                                crypto_symbol: base.to_string(),
                                crypto_amount,
                            },
                            operation_value: operation_value * brl_rate,
                            buyer_exchange: kraken_exchange_info(),
                        });
                        transactions.push(purchase);
                    }
                    "sell" => {
                        let sale = Transaction::Sale(SaleTransaction {
                            base: TransactionBase {
                                operation_date: time,
                                operation_fees: Some(fee * brl_rate),
                                crypto_symbol: base.to_string(),
                                crypto_amount,
                            },
                            operation_value: operation_value * brl_rate,
                            seller_exchange: kraken_exchange_info(),
                        });
                        transactions.push(sale);
                    }
                    _ => panic!("Unknown trade type: {}", trade_type),
                }
            }
            // Crypto-Crypto trade
            (false, false) => {
                // Convert fee to BRL using the base currency rate
                let (_rate_date, base_brl_rate) =
                    get_exchange_rate(time, base).unwrap_or_else(|e| {
                        panic!(
                            "Failed to get exchange rate for {} on {}: {}",
                            base, time, e
                        )
                    });

                let operation_fees = Some(fee * base_brl_rate);
                println!("### Original fee: {fee} {quote}, converted fee: {operation_fees:?} BRL");
                let exchange = kraken_exchange_info();

                let swap = Transaction::Swap(if trade_type == "buy" {
                    SwapTransaction {
                        operation_date: time,
                        operation_fees,
                        received_crypto_symbol: base.to_string(),
                        received_crypto_amount: vol,
                        given_crypto_symbol: quote.to_string(),
                        given_crypto_amount: cost,
                        exchange,
                    }
                } else if trade_type == "sell" {
                    SwapTransaction {
                        operation_date: time,
                        operation_fees,
                        received_crypto_symbol: quote.to_string(),
                        received_crypto_amount: cost,
                        given_crypto_symbol: base.to_string(),
                        given_crypto_amount: vol,
                        exchange,
                    }
                } else {
                    panic!("Unknown trade type: {}", trade_type);
                });
                transactions.push(swap);
            }
            // Fiat-Crypto trade (should be handled by the other case)
            (true, false) => {
                // This case should not happen as Kraken always puts the base currency first
                panic!("Unexpected Fiat-Crypto trade pair: {}", pair);
            }
            // Fiat-Fiat trade (should be ignored)
            (true, true) => continue,
        }
    }

    transactions.sort_unstable_by_key(|t| t.record_type().0);
    transactions
}

pub fn generate_report(transactions: Vec<Transaction>, out_file: &str) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(out_file)?);

    for transaction in transactions {
        transaction.write_transaction(&mut file)?;
    }

    Ok(())
}

/// Get the integer part of a Decimal
fn int_part(d: Decimal) -> i64 {
    let d = d.trunc();
    assert_eq!(d.scale(), 0, "Decimal must be an integer");
    d.mantissa() as i64
}
