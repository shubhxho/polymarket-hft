use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};
fn service(args: &[&str], input: &[Value]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hft-desk"))
        .arg("--stdio")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        for value in input {
            writeln!(stdin, "{}", value).unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
#[test]
fn desk_dispatch_replays_validates_and_retains_supplied_feed() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let mut r: Value = serde_json::from_str(
        include_str!("../../fixtures/signals.jsonl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    r["quoted_ms"] = json!(now);
    r["forecast_ms"] = json!(now);
    r["max_age_ms"] = json!(30000);
    r["fee_bps"] = json!(0);
    r["fee_rate_micro"] = json!(50000);
    let rows = service(
        &[],
        &[
            json!({"method":"GET","path":"/api/meta"}),
            json!({"method":"POST","path":"/api/replay","body":{"scenario":"normal","seed":0,"day":0}}),
            json!({"method":"POST","path":"/api/replay","body":{"scenario":"normal","seed":0,"day":20}}),
            json!({"method":"POST","path":"/api/signals","body":[r]}),
            json!({"method":"GET","path":"/api/latest"}),
            json!({"method":"POST","path":"/api/infer","body":[1]}),
        ],
    );
    assert_eq!(rows[0]["data"]["submission_enabled"], false);
    assert_eq!(rows[0]["data"]["weather"]["portfolios"], 96);
    let report: Value =
        serde_json::from_str(include_str!("../../reports/weather-10000.json")).unwrap();
    assert_eq!(
        rows[1]["data"]["cash_micro"],
        report["portfolios"][0]["daily_equity_micro"][0]
    );
    assert_eq!(rows[2]["error"], "day_out_of_range");
    assert_eq!(rows[3]["data"]["best_candidate_id"], "synthetic-signal-1");
    assert_eq!(
        rows[3]["data"]["signals"][0]["id"],
        rows[4]["data"]["signals"][0]["id"]
    );
    assert_eq!(rows[5]["ok"], false);
}
#[test]
fn desk_retains_packed_cpu_model_across_batches() {
    let path = std::env::temp_dir().join(format!("hft-desk-{}.hqt", std::process::id()));
    std::fs::write(
        &path,
        polymarket_hft::quant::Quantized::new(&[1.; 64], 4, 64)
            .unwrap()
            .encode(),
    )
    .unwrap();
    let rows = service(
        &["8787", path.to_str().unwrap(), "cpu"],
        &[
            json!({"method":"POST","path":"/api/infer","body":vec![1.;64]}),
            json!({"method":"POST","path":"/api/infer","body":vec![2.;64]}),
            json!({"method":"POST","path":"/api/infer","body":[1]}),
        ],
    );
    std::fs::remove_file(path).unwrap();
    assert_eq!(rows[0]["data"]["backend"], "cpu");
    assert_eq!(rows[0]["data"]["values"], json!([64.]));
    assert_eq!(rows[1]["data"]["values"], json!([128.]));
    assert_eq!(rows[2]["ok"], false);
}
