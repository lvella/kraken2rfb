/// This module provides functionality to retrieve exchange rates from different sources.
///
/// For traditional currencies, it uses the BCB (Banco Central do Brasil) API.
/// For cryptocurrencies, it uses the CoinGecko public API.
///
/// NOTE on CoinGecko API Usage:
///
/// The CoinGecko API implementation has the following limitations for the free tier:
/// - Rate limits: 10-30 calls per minute
/// - Historical data: Limited to recent dates (roughly within the past year)
/// - May require API key for higher volumes
///
/// For production use with higher volumes or reliable historical data access,
/// consider subscribing to CoinGecko Pro API and modifying the get_crypto_rate_* functions
/// to include an API key in the headers:
///
/// ```
/// let client = Client::builder()
///     .default_headers({
///         let mut headers = reqwest::header::HeaderMap::new();
///         headers.insert("x-cg-pro-api-key",
///             reqwest::header::HeaderValue::from_str("YOUR_API_KEY").unwrap());
///         headers
///     })
///     .build()?;
/// ```
use chrono::{Local, NaiveDate};
use phf::phf_map;
use reqwest::blocking::Client;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use serde::Deserialize;
use serde_json;
use std::collections::HashMap;
use std::error::Error;

#[derive(Debug, Deserialize)]
struct BCBValue {
    #[serde(deserialize_with = "deserialize_date")]
    data: NaiveDate,
    #[serde(deserialize_with = "deserialize_decimal")]
    valor: Decimal,
}

fn deserialize_date<'de, D>(deserializer: D) -> Result<NaiveDate, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    NaiveDate::parse_from_str(&s, "%d/%m/%Y").map_err(serde::de::Error::custom)
}

fn deserialize_decimal<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Decimal::from_str(&s).map_err(serde::de::Error::custom)
}

/// PHF map to convert currency codes to BCB series codes
static CURRENCY_TO_BCB_SERIES: phf::Map<&'static str, &'static str> = phf_map! {
    "USD" => "1",     // Dólar Comercial (venda)
    "EUR" => "21619", // Euro (venda)
    "JPY" => "21621", // Iene (venda)
    "GBP" => "21623", // Libra esterlina (venda)
    "CHF" => "21625", // Franco Suíço (venda)
    "DKK" => "21627", // Coroa Dinamarquesa (venda)
    "NOK" => "21629", // Coroa Norueguesa (venda)
    "SEK" => "21631", // Coroa Sueca (venda)
    "AUD" => "21633", // Dólar Australiano (venda)
    "CAD" => "21635", // Dólar Canadense (venda)
};

/// CoinGecko API response for prices
#[derive(Debug, Deserialize)]
struct CoinGeckoMarketData {
    current_price: HashMap<String, f64>,
}

#[derive(Debug, Deserialize)]
struct CoinGeckoCoinData {
    market_data: CoinGeckoMarketData,
}

/// CoinGecko API response for historical prices
/// The structure is the same as the current price response
type CoinGeckoHistoricalData = CoinGeckoCoinData;

/// Fetches the historical exchange rate of a cryptocurrency against BRL from CoinGecko for a specific date.
///
/// # Arguments
/// * `crypto_id` - The CoinGecko ID of the cryptocurrency (e.g., "bitcoin", "ethereum", "litecoin")
/// * `date` - The date to fetch the exchange rate for
///
/// # Returns
/// A tuple containing:
/// * The actual date of the exchange rate (same as the input date if data is available)
/// * The exchange rate as a Decimal (BRL per unit of cryptocurrency)
///
/// # Errors
/// Returns an error if:
/// * The date is in the future
/// * The API request fails
/// * The response cannot be parsed
/// * The cryptocurrency ID is not supported by CoinGecko
/// * No exchange rate data is available for the specified date
fn get_crypto_rate_historical(
    crypto_id: &str,
    date: NaiveDate,
) -> Result<(NaiveDate, Decimal), Box<dyn Error>> {
    let today = Local::now().date_naive();

    if date > today {
        return Err("Cannot fetch exchange rate for future dates".into());
    }

    let formatted_date = date.format("%d-%m-%Y").to_string();
    let client = Client::new();
    let url = format!(
        "https://api.coingecko.com/api/v3/coins/{}/history?date={}&localization=false",
        crypto_id, formatted_date
    );

    let response = client.get(&url).send()?;

    if response.status() == 404 {
        return Err(format!("Cryptocurrency ID not found: {}", crypto_id).into());
    }

    if !response.status().is_success() {
        return Err(format!("CoinGecko API error: {}", response.status()).into());
    }

    let historical_data: CoinGeckoHistoricalData = response.json()?;

    let price_brl = historical_data
        .market_data
        .current_price
        .get("brl")
        .ok_or_else(|| format!("BRL price not available for {} on {}", crypto_id, date))?;

    let rate = Decimal::from_f64(*price_brl)
        .ok_or_else(|| "Failed to convert price to Decimal".to_string())?;

    Ok((date, rate))
}

/// Fetches the exchange rate from BCB for a given date and currency code.
/// If the requested date is not a bank day, returns the most recent available rate.
///
/// # Arguments
/// * `date` - The date to fetch the exchange rate for
/// * `currency_code` - The currency code (e.g., "USD", "EUR", "JPY")
///
/// # Returns
/// A tuple containing:
/// * The actual date of the exchange rate (which might be the first bank day before the supplied date)
/// * The exchange rate as a Decimal for maximum precision
///
/// # Errors
/// Returns an error if:
/// * The date is in the future
/// * The API request fails
/// * The response cannot be parsed
/// * The currency code is not supported
/// * No exchange rate data is available within the last 7 days
fn get_fiat_exchange_rate(
    date: NaiveDate,
    currency_code: &str,
) -> Result<(NaiveDate, Decimal), Box<dyn Error>> {
    let today = Local::now().date_naive();

    if date > today {
        return Err("Cannot fetch exchange rate for future dates".into());
    }

    let series_code = CURRENCY_TO_BCB_SERIES
        .get(currency_code)
        .ok_or_else(|| format!("Unsupported currency code: {}", currency_code))?;

    // Request data for the last 7 days to ensure we get a valid bank day
    let start_date = date - chrono::Duration::days(7);
    let client = Client::new();
    let url = format!(
        "https://api.bcb.gov.br/dados/serie/bcdata.sgs.{}/dados?formato=json&dataInicial={}&dataFinal={}",
        series_code,
        start_date.format("%d/%m/%Y"),
        date.format("%d/%m/%Y")
    );

    let resp = client.get(&url).send()?;
    let text = resp.text()?;
    let mut response: Vec<BCBValue> = serde_json::from_str(&text)?;

    if response.is_empty() {
        return Err(format!(
            "No exchange rate data available for {} within the last 7 days of {}",
            currency_code, date
        )
        .into());
    }

    // Sort by date in descending order to get the most recent rate
    response.sort_by(|a, b| b.data.cmp(&a.data));

    let rate_data = &response[0];
    Ok((rate_data.data, rate_data.valor))
}

// AssetType enum has been removed as it's no longer needed
// The get_exchange_rate function now automatically detects the asset type

/// Fetches the exchange rate for a given asset against BRL.
/// This function automatically detects the asset type and chooses the appropriate data source:
/// - BCB API for fiat currencies supported by get_currency_series_code()
/// - CoinGecko API for cryptocurrencies and any other assets
///
/// # Arguments
/// * `date` - The date to fetch the exchange rate for
/// * `asset_code` - The asset code (e.g., "USD", "EUR", "BTC", "ETH")
///   * For fiat currencies: Use standard ISO code (e.g., "USD", "EUR", "JPY")
///   * For cryptocurrencies: Use standard ticker (e.g., "BTC", "ETH", "LTC")
///
/// # Returns
/// A tuple containing:
/// * The actual date of the exchange rate
/// * The exchange rate as a Decimal (BRL per unit of asset)
///
/// # Errors
/// Returns an error if:
/// * The date is in the future
/// * The API request fails
/// * The asset code is not supported
/// * No exchange rate data is available
pub fn get_exchange_rate(
    date: NaiveDate,
    asset_code: &str,
) -> Result<(NaiveDate, Decimal), Box<dyn Error>> {
    // First try as fiat currency with BCB
    if let Some(_) = CURRENCY_TO_BCB_SERIES.get(asset_code) {
        return get_fiat_exchange_rate(date, asset_code);
    }

    // If not a supported fiat currency, try as cryptocurrency with CoinGecko
    // Look up the CoinGecko ID from the ticker
    let coingecko_id = if let Some(id) = CRYPTO_TICKER_TO_ID.get(asset_code) {
        *id
    } else {
        // If not found in the map, try using the code directly as a CoinGecko ID
        asset_code
    };

    get_crypto_rate_historical(coingecko_id, date)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_all_currencies() {
        let date = NaiveDate::from_ymd_opt(2024, 3, 1).unwrap();
        println!("\nTesting exchange rates for {}:", date);
        println!("----------------------------------------");

        for (code, series) in [
            ("USD", "1"),
            ("EUR", "21619"),
            ("JPY", "21621"),
            ("GBP", "21623"),
            ("CHF", "21625"),
            ("DKK", "21627"),
            ("NOK", "21629"),
            ("SEK", "21631"),
            ("AUD", "21633"),
            ("CAD", "21635"),
        ] {
            println!("Currency: {} (Series: {})", code, series);
            match get_fiat_exchange_rate(date, code) {
                Ok((_date, rate)) => println!("  Rate: {}", rate),
                Err(e) => println!("  Error: {}", e),
            }
            println!("----------------------------------------");
        }
    }

    #[test]
    fn test_invalid_currency() {
        let date = NaiveDate::from_ymd_opt(2024, 3, 1).unwrap();
        let result = get_fiat_exchange_rate(date, "INVALID");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unsupported currency code")
        );
    }

    #[test]
    fn test_future_date() {
        let future_date = Local::now().date_naive() + chrono::Duration::days(1);
        let result = get_fiat_exchange_rate(future_date, "USD");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Cannot fetch exchange rate for future dates")
        );
    }

    #[test]
    fn test_non_bank_day() {
        // March 2, 2024 is a Saturday
        let weekend_date = NaiveDate::from_ymd_opt(2024, 3, 2).unwrap();
        let result = get_fiat_exchange_rate(weekend_date, "USD").unwrap();

        // The rate should be from the previous business day (March 1, 2024)
        assert_eq!(result.0, NaiveDate::from_ymd_opt(2024, 3, 1).unwrap());

        // The rate should be positive and reasonable
        assert!(result.1 > Decimal::ZERO);
        assert!(result.1 < dec!(10));
    }

    #[test]
    fn test_crypto_rate_historical() {
        // Get a date from the previous week which should be well within CoinGecko's free API limitations
        let date = Local::now().date_naive() - chrono::Duration::days(7);
        println!("\nTesting historical cryptocurrency rates for {}:", date);
        println!("----------------------------------------");

        // Only test with Bitcoin to avoid hitting rate limits
        let crypto_id = "bitcoin";
        println!("Cryptocurrency: {}", crypto_id);
        let result = get_crypto_rate_historical(crypto_id, date);

        match &result {
            Ok((date, rate)) => println!("  Date: {}, Rate: {} BRL", date, rate),
            Err(e) => println!("  Error: {}", e),
        }
        println!("----------------------------------------");

        // Assert that we can retrieve the historical rate
        // If the test is failing consistently with API errors, consider:
        // 1. Using an even more recent date (yesterday)
        // 2. Obtaining a CoinGecko API key
        // 3. Marking the test as #[ignore] with a comment explaining why
        assert!(
            result.is_ok(),
            "Failed to get historical rate for Bitcoin: {:?}",
            result.err()
        );

        // Verify the rate is reasonable (non-zero, positive)
        let (_, rate) = result.unwrap();
        assert!(rate > Decimal::ZERO, "Bitcoin rate should be positive");
    }

    #[test]
    fn test_invalid_crypto_id() {
        let date = Local::now().date_naive() - chrono::Duration::days(3);
        let result = get_crypto_rate_historical("invalid-crypto-id-123456789", date);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Cryptocurrency ID not found")
        );
    }

    #[test]
    fn test_crypto_future_date() {
        let future_date = Local::now().date_naive() + chrono::Duration::days(1);
        let result = get_crypto_rate_historical("bitcoin", future_date);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Cannot fetch exchange rate for future dates")
        );
    }

    #[test]
    fn test_unified_api() {
        // Use a very recent date (3 days ago) to ensure CoinGecko will provide data
        let date = Local::now().date_naive() - chrono::Duration::days(3);
        println!("\nTesting unified API for {}:", date);
        println!("----------------------------------------");

        // Test traditional currency
        let currency_result = get_exchange_rate(date, "USD");
        match &currency_result {
            Ok((date, rate)) => println!("USD: Date: {}, Rate: {} BRL", date, rate),
            Err(e) => panic!("USD Error: {}", e),
        }
        assert!(
            currency_result.is_ok(),
            "Failed to get USD exchange rate: {:?}",
            currency_result.err()
        );

        // Test cryptocurrency with proper assertions
        let crypto_result = get_exchange_rate(date, "BTC");
        match &crypto_result {
            Ok((date, rate)) => println!("Bitcoin: Date: {}, Rate: {} BRL", date, rate),
            Err(e) => panic!("Bitcoin Error: {}", e),
        }

        // Verify the cryptocurrency rate is reasonable
        let (_, rate) = crypto_result.unwrap();
        assert!(rate > Decimal::ZERO, "Bitcoin rate should be positive");

        println!("----------------------------------------");
    }
}

/// Maps standard cryptocurrency tickers to CoinGecko IDs
/// This is necessary because CoinGecko uses IDs like "bitcoin" instead of tickers like "BTC"
static CRYPTO_TICKER_TO_ID: phf::Map<&'static str, &'static str> = phf_map! {
    // Major cryptocurrencies
    "BTC" => "bitcoin",
    "ETH" => "ethereum",
    "LTC" => "litecoin",
    "XRP" => "ripple",
    "BCH" => "bitcoin-cash",
    "BNB" => "binancecoin",
    "ADA" => "cardano",
    "DOT" => "polkadot",
    "DOGE" => "dogecoin",
    "SOL" => "solana",
    "USDT" => "tether",
    "USDC" => "usd-coin",
    "AVAX" => "avalanche-2",
    "LINK" => "chainlink",
    "MATIC" => "matic-network",
    "XLM" => "stellar",
    "UNI" => "uniswap",
    "ATOM" => "cosmos",
    "ALGO" => "algorand",
    "XTZ" => "tezos",
    // Add more mappings as needed
};
