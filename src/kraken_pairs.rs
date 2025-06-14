// Generated at build time by build.rs
include!(concat!(env!("OUT_DIR"), "/kraken_pairs_map.rs"));

/// Get the base and quote altnames for a given Kraken trading pair
pub fn parse_pair(pair: &str) -> Option<(&'static str, &'static str)> {
    KRAKEN_PAIRS.get(pair).copied()
}
