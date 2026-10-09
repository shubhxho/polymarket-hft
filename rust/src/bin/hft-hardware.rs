fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&polymarket_hft::hardware::detect()).unwrap()
    );
}
