use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=kraken_data/assets.json");
    println!("cargo:rerun-if-changed=kraken_data/kraken_pairs.json");

    // Read and parse assets.json
    let assets_content =
        std::fs::read_to_string("kraken_data/assets.json").expect("Failed to read assets.json");
    let assets_json: serde_json::Value =
        serde_json::from_str(&assets_content).expect("Failed to parse assets.json");

    // Create a map from asset ID to altname
    let mut asset_altnames = HashMap::new();
    if let Some(result) = assets_json["result"].as_object() {
        for (asset_id, asset_info) in result {
            if let Some(altname) = asset_info["altname"].as_str() {
                // BTC is the only exception where altname is "XBT"
                // but we want to use "BTC" in the generated map
                let altname = if altname == "XBT" { "BTC" } else { altname };
                asset_altnames.insert(asset_id.clone(), altname.to_string());
            }
        }
    }

    // Read and parse kraken_pairs.json
    let pairs_content = std::fs::read_to_string("kraken_data/kraken_pairs.json")
        .expect("Failed to read kraken_pairs.json");
    let pairs_json: serde_json::Value =
        serde_json::from_str(&pairs_content).expect("Failed to parse kraken_pairs.json");

    // Generate PHF map entries
    let mut phf_entries = Vec::new();
    if let Some(result) = pairs_json["result"].as_object() {
        for (pair_name, pair_info) in result {
            if let (Some(base), Some(quote)) =
                (pair_info["base"].as_str(), pair_info["quote"].as_str())
            {
                // Get altnames for base and quote
                let base_altname = asset_altnames.get(base).unwrap().clone();
                let quote_altname = asset_altnames.get(quote).unwrap().clone();

                phf_entries.push(format!(
                    "    \"{}\" => (\"{}\", \"{}\"),",
                    pair_name, base_altname, quote_altname
                ));
            }
        }
    }

    // Write the generated code to a file
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("kraken_pairs_map.rs");
    let mut f = File::create(&dest_path).unwrap();

    write!(
        f,
        r#"
use phf::Map;

pub static KRAKEN_PAIRS: Map<&'static str, (&'static str, &'static str)> = phf::phf_map! {{
{}
}};
"#,
        phf_entries.join("\n")
    )
    .unwrap();

    println!("Generated {} pair mappings", phf_entries.len());
}
