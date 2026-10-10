use polymarket_hft::wallet::WalletClient;
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(1..=2).contains(&args.len()) {
        return Err(
            "usage: HFT_POLYGON_RPC_URL=https://... hft-wallet <address> [token-contract]".into(),
        );
    }
    let endpoint = std::env::var("HFT_POLYGON_RPC_URL").map_err(|_| "missing_polygon_rpc_url")?;
    let snapshot =
        WalletClient::new(&endpoint)?.snapshot(&args[0], args.get(1).map(String::as_str))?;
    println!("{}", serde_json::to_string_pretty(&snapshot)?);
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
