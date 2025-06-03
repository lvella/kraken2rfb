pub mod encoding;
pub mod transactions;

use crate::kraken::is_fiat;
use crate::{exchange_rate::get_exchange_rate, to_decimal};
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

            let withdrawal =
                Transaction::WithdrawalFromExchange(WithdrawalFromExchangeTransaction {
                    base: TransactionBase {
                        operation_date: time,
                        operation_fees: Some(fee),
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
        let (base, quote) = parse_trading_pair(pair);
        let vol = trade["vol"].as_str().unwrap().parse::<Decimal>().unwrap(); // QUOTE amount
        let cost = trade["cost"].as_str().unwrap().parse::<Decimal>().unwrap(); // BASE amount
        let fee = trade["fee"].as_str().unwrap().parse::<Decimal>().unwrap(); // BASE amount
        let price = trade["price"].as_str().unwrap().parse::<Decimal>().unwrap(); // BASE / QUOTE
        let time = DateTime::from_timestamp(int_part(to_decimal(&trade["time"])), 0)
            .unwrap()
            .date_naive();
        let trade_type = trade["type"].as_str().unwrap();

        match (is_fiat(base), is_fiat(quote)) {
            // Crypto-Fiat trade
            (false, true) => {
                // Calculate net amounts (after fees)
                let operation_value = cost - fee; // BASE amount
                let crypto_amount = vol - (fee / price); // QUOTE amount
                let (_rate_date, brl_rate /* BRL / BASE */) = get_exchange_rate(time, quote).unwrap_or_else(|e| {
                    panic!(
                        "Failed to get exchange rate for {} on {}: {}",
                        quote, time, e
                    )
                });

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
                panic!("We don't have the exchange rate to convert the fee to BRL");
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

    transactions
}

/// Parse a Kraken trading pair into base and quote currencies
fn parse_trading_pair(pair: &str) -> (&str, &str) {
    // Remove any prefix like "X" or "Z" from the pair
    let clean_pair = pair.trim_start_matches(|c| c == 'X' || c == 'Z');
    
    // Find the first position where we have a known fiat currency
    let fiat_positions = ["USD", "EUR", "GBP", "JPY", "CAD", "AUD", "CHF", "BRL", "ARS", "AED"];
    let mut split_pos = None;
    
    for fiat in fiat_positions {
        if let Some(pos) = clean_pair.find(fiat) {
            if pos > 0 {  // Only split if the fiat is not at the start
                split_pos = Some(pos);
                break;
            }
        }
    }
    
    // If we found a fiat currency, split there
    if let Some(pos) = split_pos {
        let (base, quote) = clean_pair.split_at(pos);
        (base, quote)
    } else {
        // If no fiat found, assume the first 3-4 characters are the base
        // This handles cases like USDT, USDC, etc.
        if clean_pair.starts_with("USDT") || clean_pair.starts_with("USDC") {
            clean_pair.split_at(4)
        } else {
            clean_pair.split_at(3)
        }
    }
}

/// Get the integer part of a Decimal
fn int_part(d: Decimal) -> i64 {
    let d = d.trunc();
    assert_eq!(d.scale(), 0, "Decimal must be an integer");
    d.mantissa() as i64
}
