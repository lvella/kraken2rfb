use chrono::{Local, NaiveDate};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::error::Error;
use serde_json;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;

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
    NaiveDate::parse_from_str(&s, "%d/%m/%Y")
        .map_err(serde::de::Error::custom)
}

fn deserialize_decimal<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Decimal::from_str(&s).map_err(serde::de::Error::custom)
}

fn get_currency_series_code(code: &str) -> Option<&'static str> {
    match code {
        "USD" => Some("1"),           // Dólar Comercial (venda)
        "EUR" => Some("21619"),       // Euro (venda)
        "JPY" => Some("21621"),       // Iene (venda)
        "GBP" => Some("21623"),       // Libra esterlina (venda)
        "CHF" => Some("21625"),       // Franco Suíço (venda)
        "DKK" => Some("21627"),       // Coroa Dinamarquesa (venda)
        "NOK" => Some("21629"),       // Coroa Norueguesa (venda)
        "SEK" => Some("21631"),       // Coroa Sueca (venda)
        "AUD" => Some("21633"),       // Dólar Australiano (venda)
        "CAD" => Some("21635"),       // Dólar Canadense (venda)
        _ => None,
    }
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
pub fn get_exchange_rate(date: NaiveDate, currency_code: &str) -> Result<(NaiveDate, Decimal), Box<dyn Error>> {
    let today = Local::now().date_naive();
    
    if date > today {
        return Err("Cannot fetch exchange rate for future dates".into());
    }

    let series_code = get_currency_series_code(currency_code)
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
        return Err(format!("No exchange rate data available for {} within the last 7 days of {}", currency_code, date).into());
    }

    // Sort by date in descending order to get the most recent rate
    response.sort_by(|a, b| b.data.cmp(&a.data));

    let rate_data = &response[0];
    Ok((rate_data.data, rate_data.valor))
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
            match get_exchange_rate(date, code) {
                Ok((_date, rate)) => println!("  Rate: {}", rate),
                Err(e) => println!("  Error: {}", e),
            }
            println!("----------------------------------------");
        }
    }

    #[test]
    fn test_invalid_currency() {
        let date = NaiveDate::from_ymd_opt(2024, 3, 1).unwrap();
        let result = get_exchange_rate(date, "INVALID");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported currency code"));
    }

    #[test]
    fn test_future_date() {
        let future_date = Local::now().date_naive() + chrono::Duration::days(1);
        let result = get_exchange_rate(future_date, "USD");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot fetch exchange rate for future dates"));
    }

    #[test]
    fn test_non_bank_day() {
        // March 2, 2024 is a Saturday
        let weekend_date = NaiveDate::from_ymd_opt(2024, 3, 2).unwrap();
        let result = get_exchange_rate(weekend_date, "USD").unwrap();
        
        // The rate should be from the previous business day (March 1, 2024)
        assert_eq!(result.0, NaiveDate::from_ymd_opt(2024, 3, 1).unwrap());
        
        // The rate should be positive and reasonable
        assert!(result.1 > Decimal::ZERO);
        assert!(result.1 < dec!(10));
    }
} 