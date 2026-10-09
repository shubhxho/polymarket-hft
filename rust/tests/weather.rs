use polymarket_hft::weather;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "weather-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn all_weather_contracts_share_ten_thousand_and_settle_without_future_data() {
    let temp = Temp::new();
    let manifest = weather::generate(&temp.0, 3, 2).unwrap();
    assert_eq!(manifest.records, 6 * 2 * 3 * 16);
    let report = weather::simulate(&temp.0).unwrap();
    assert_eq!(report.portfolios.len(), 12);
    assert_eq!(report.evaluated_buckets, manifest.records * 5);
    assert!(report.synthetic && !report.live_ready && !report.profitability_established);
    for p in &report.portfolios {
        assert_eq!(p.starting_cash_micro, 10_000_000_000);
        assert_eq!(p.daily_equity_micro.len(), 3);
        assert_eq!(
            p.pnl_micro,
            i128::from(p.ending_cash_micro) - 10_000_000_000
        );
    }
    assert!(
        report
            .portfolios
            .iter()
            .any(|p| p.orders > 0 && p.fees_micro > 0)
    );
    assert!(
        report
            .portfolios
            .iter()
            .filter(|p| p.scenario == "delayed" || p.scenario == "thin")
            .all(|p| p.orders == 0 && p.pnl_micro == 0)
    );
    assert!(weather::generate(&temp.0, 3, 2).is_err());
    let other = Temp::new();
    weather::generate(&other.0, 3, 2).unwrap();
    assert_eq!(
        fs::read(temp.0.join("forecasts.jsonl")).unwrap(),
        fs::read(other.0.join("forecasts.jsonl")).unwrap()
    );
}
#[test]
fn weather_bounds_and_missing_outcomes_fail() {
    let temp = Temp::new();
    assert!(weather::generate(&temp.0, 0, 2).is_err());
    weather::generate(&temp.0, 1, 1).unwrap();
    fs::write(temp.0.join("resolutions.jsonl"), b"").unwrap();
    assert!(weather::simulate(&temp.0).is_err());
}
