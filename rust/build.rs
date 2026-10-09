use std::{collections::BTreeMap, env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=../schema/polygon.conf");
    let text = fs::read_to_string("../schema/polygon.conf").expect("Polygon schema");
    let mut values = BTreeMap::new();
    for line in text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
    {
        let (key, value) = line.split_once('=').expect("key=value schema");
        assert!(values.insert(key, value).is_none(), "duplicate schema key");
    }
    assert_eq!(values.len(), 5, "unknown/missing schema keys");
    assert_eq!(values["chain_id"], "137", "Polygon mainnet only");
    assert_eq!(values["chain_name"], "Polygon");
    assert_eq!(values["rpc"], "https://polygon-rpc.com");
    let price: u32 = values["max_price"].parse().unwrap();
    let levels: usize = values["max_book_levels"].parse().unwrap();
    assert_eq!(price, 1_000_000);
    assert_eq!(levels, 1001);
    let output = format!(
        "// Generated from schema/polygon.conf; do not edit.\n\
         pub const CHAIN_ID: u64 = 137;\n\
         pub const CHAIN_NAME: &str = \"Polygon\";\n\
         pub const RPC: &str = \"https://polygon-rpc.com\";\n\
         pub const PRICE_SCALE: u32 = {price};\n\
         pub const BOOK_LEVELS: usize = {levels};\n"
    );
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("polygon.rs"),
        output,
    )
    .unwrap();
}
