use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chrono::NaiveDate;
use hmac::{Hmac, Mac};
use phf::phf_set;
use reqwest::blocking::Client;
use reqwest::header::HeaderMap;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_urlencoded;
use sha2::{Digest, Sha256, Sha512};
use std::collections::BTreeMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha512 = Hmac<Sha512>;

#[derive(Serialize, Deserialize)]
struct ApiKeys {
    key: String,
    secret: String,
}

fn load_api_keys(path: &str) -> ApiKeys {
    let data = fs::read_to_string(path).expect("Failed to read key file");
    serde_json::from_str(&data).expect("Invalid JSON in key file")
}

fn get_timestamp(date: NaiveDate) -> u64 {
    date.and_hms_opt(0, 0, 0).unwrap().timestamp() as u64
}

// Kraken API signature
fn kraken_signature(uri_path: &str, data: &BTreeMap<&str, String>, secret: &str) -> String {
    // Get nonce from data
    let nonce = data.get("nonce").expect("nonce is required");

    // Create the encoded data string (nonce + urlencoded data)
    let encoded_data = format!("{}{}", nonce, serde_urlencoded::to_string(data).unwrap());

    // Create the message (uri_path + sha256(encoded_data))
    let mut hasher = Sha256::new();
    hasher.update(encoded_data.as_bytes());
    let hash = hasher.finalize();

    let mut message = uri_path.as_bytes().to_vec();
    message.extend_from_slice(&hash);

    // Create HMAC-SHA512
    let decoded_secret = BASE64.decode(secret).expect("Base64 decode failed");
    let mut mac =
        Hmac::<Sha512>::new_from_slice(&decoded_secret).expect("HMAC can take key of any size");
    mac.update(&message);

    // Return base64 encoded signature
    BASE64.encode(mac.finalize().into_bytes())
}

// Helper for authenticated requests
fn kraken_private_request(
    client: &Client,
    api_keys: &ApiKeys,
    uri_path: &str,
    params: &mut BTreeMap<&str, String>,
) -> Value {
    let url = format!("https://api.kraken.com{}", uri_path);
    let nonce = format!(
        "{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    params.insert("nonce", nonce);

    let signature = kraken_signature(uri_path, params, &api_keys.secret);

    let mut headers = HeaderMap::new();
    headers.insert("API-Key", api_keys.key.parse().unwrap());
    headers.insert("API-Sign", signature.parse().unwrap());

    let res = client
        .post(url)
        .headers(headers)
        .form(params)
        .send()
        .expect("API request failed");
    let json: Value = res.json().expect("Invalid JSON");
    if !json["error"].as_array().unwrap().is_empty() {
        panic!("Kraken error: {:?}", json["error"]);
    }
    json["result"].clone()
}

fn to_decimal(value: &Value) -> Decimal {
    Decimal::try_from(value.as_number().unwrap().as_str()).unwrap()
}

pub fn fetch_kraken_activity(
    initial: NaiveDate,
    final_: NaiveDate,
    keyfile: &str,
) -> (Vec<Value>, Vec<Value>, Vec<Value>) {
    let api_keys = load_api_keys(keyfile);
    let client = Client::new();

    let start_ts = get_timestamp(initial);
    let end_ts = get_timestamp(final_) + 24 * 60 * 60 - 1; // include whole final day

    // 1. Deposits
    let mut params = BTreeMap::new();
    params.insert("start", start_ts.to_string());
    params.insert("end", end_ts.to_string());
    let deposits_json =
        kraken_private_request(&client, &api_keys, "/0/private/DepositStatus", &mut params);
    let mut deposits: Vec<Value> = deposits_json
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| {
            entry["time"]
                .as_u64()
                .map(|ts| ts >= start_ts && ts <= end_ts)
                .unwrap()
        })
        .cloned()
        .collect();

    // 2. Withdrawals
    let mut params = BTreeMap::new();
    params.insert("start", start_ts.to_string());
    params.insert("end", end_ts.to_string());
    let withdrawals_json =
        kraken_private_request(&client, &api_keys, "/0/private/WithdrawStatus", &mut params);
    let mut withdrawals: Vec<Value> = withdrawals_json
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| {
            entry["time"]
                .as_u64()
                .map(|ts| ts >= start_ts && ts <= end_ts)
                .unwrap()
        })
        .cloned()
        .collect();

    // 3. Trades
    let mut params = BTreeMap::new();
    params.insert("start", start_ts.to_string());
    params.insert("end", end_ts.to_string());
    let trades_json =
        kraken_private_request(&client, &api_keys, "/0/private/TradesHistory", &mut params);
    let mut trades: Vec<Value> = trades_json["trades"]
        .as_object()
        .unwrap()
        .values()
        .filter(|entry| {
            let ts = to_decimal(&entry["time"]);
            ts >= Decimal::from(start_ts) && ts <= Decimal::from(end_ts)
        })
        .cloned()
        .collect();

    // Sort all by time ascending
    deposits.sort_by_key(|v| v["time"].as_u64().unwrap());
    withdrawals.sort_by_key(|v| v["time"].as_u64().unwrap());
    trades.sort_by_key(|v| to_decimal(&v["time"]));

    (deposits, withdrawals, trades)
}

pub fn is_fiat(ticker: &str) -> bool {
    static FIAT_CURRENCIES: phf::Set<&'static str> = phf_set! {
        "USD", "ZUSD",
        "EUR", "ZEUR",
        "GBP", "ZGBP",
        "JPY", "ZJPY",
        "CAD", "ZCAD",
        "AUD", "ZAUD",
        "MXN", "ZMXN",
        "CHF", "ZCHF",
        "BRL", "ZBRL",
        "ARS", "ZARS",
        "AED", "ZAED",
    };
    FIAT_CURRENCIES.contains(ticker.to_uppercase().as_str())
}
