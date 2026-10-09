use polymarket_hft::{
    book::Side,
    engine::{Config, Event},
    news::{Answer, JevResponse, NewsItem, NewsPipeline, Route, request},
    storage::Journal,
};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn binary_scoring_is_finite_and_rejects_bad_data() {
    use polymarket_hft::model::binary_scores;
    let s = binary_scores(&[0.9, 0.1], &[true, false]).unwrap();
    assert!((s.brier - 0.01).abs() < 1e-12);
    assert!((s.log_loss + 0.9f64.ln()).abs() < 1e-12);
    assert!((s.calibration_error - 0.1).abs() < 1e-12);
    assert!(
        binary_scores(&[0.0, 1.0], &[true, false])
            .unwrap()
            .log_loss
            .is_finite()
    );
    assert!(binary_scores(&[f64::NAN], &[true]).is_err());
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "hft-pipeline-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn journal(&self) -> PathBuf {
        self.0.join("state.journal")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn events() -> Vec<Event> {
    include_str!("../../fixtures/smoke.jsonl")
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}
fn news() -> (NewsItem, JevResponse) {
    let value: serde_json::Value = serde_json::from_str(
        include_str!("../../fixtures/news.jsonl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    (
        serde_json::from_value(value["item"].clone()).unwrap(),
        serde_json::from_value(value["response"].clone()).unwrap(),
    )
}

#[test]
fn durable_state_recovers_without_double_fills() {
    let temp = Temp::new();
    let mut j = Journal::open(temp.journal(), None).unwrap();
    for e in events() {
        let _ = j.apply(e).unwrap();
    }
    let cash = j.engine().cash_micro();
    let pnl = j.engine().realized_pnl_micro();
    assert_eq!(j.event_count(), 8);
    drop(j);
    let j = Journal::open(temp.journal(), None).unwrap();
    assert_eq!(j.engine().cash_micro(), cash);
    assert_eq!(j.engine().realized_pnl_micro(), pnl);
    for (i, e) in events().iter().enumerate() {
        assert!(j.is_replayed(i, e).unwrap());
    }
    assert!(
        j.is_replayed(
            0,
            &Event::Halt {
                reason: "changed".into()
            }
        )
        .is_err()
    );
}
#[test]
fn writer_lock_and_config_drift_reject() {
    let temp = Temp::new();
    let j = Journal::open(temp.journal(), None).unwrap();
    assert!(Journal::open(temp.journal(), None).is_err());
    drop(j);
    assert!(
        Journal::open(
            temp.journal(),
            Some(Config {
                cash_micro: 42,
                ..Config::default()
            })
        )
        .is_err()
    );
}
#[test]
fn torn_final_record_recovers_but_complete_corruption_fails() {
    let temp = Temp::new();
    let mut j = Journal::open(temp.journal(), None).unwrap();
    j.apply(events().remove(0)).unwrap().unwrap();
    drop(j);
    OpenOptions::new()
        .append(true)
        .open(temp.journal())
        .unwrap()
        .write_all(b"{\"unfinished")
        .unwrap();
    let j = Journal::open(temp.journal(), None).unwrap();
    assert_eq!(j.recovered_tail_bytes, 12);
    assert_eq!(j.event_count(), 1);
    drop(j);
    let bytes = fs::read_to_string(temp.journal())
        .unwrap()
        .replace("1000000000", "1000000001");
    fs::write(temp.journal(), bytes).unwrap();
    assert!(Journal::open(temp.journal(), None).is_err());
}
#[test]
fn dedup_survives_restart_and_missing_model_routes_review() {
    let temp = Temp::new();
    let (item, r) = news();
    let mut j = Journal::open(temp.journal(), None).unwrap();
    assert_eq!(
        j.ingest_news(item.clone(), Some(r)).unwrap().route,
        Route::Watch
    );
    drop(j);
    let mut j = Journal::open(temp.journal(), None).unwrap();
    assert!(j.ingest_news(item.clone(), None).is_err());
    let mut other = item;
    other.id = "other".into();
    other.published_ms += 1;
    other.received_ms += 1;
    assert_eq!(j.ingest_news(other, None).unwrap().route, Route::Review);
}
#[test]
fn source_time_and_content_dedup_gates() {
    let (item, r) = news();
    let mut p = NewsPipeline::default();
    for url in [
        "http://weather.gov/",
        "https://weather.gov.evil.org/",
        "https://user:pw@weather.gov/",
        "https://127.0.0.1/",
    ] {
        let mut bad = item.clone();
        bad.source_url = url.into();
        assert!(p.validate(&bad).is_err());
    }
    let mut bad = item.clone();
    bad.published_ms = bad.received_ms + 1;
    assert!(p.validate(&bad).is_err());
    bad.published_ms = 0;
    bad.received_ms = 900001;
    assert!(p.validate(&bad).is_err());
    p.ingest(&item, Some(&r)).unwrap();
    let mut duplicate = item;
    duplicate.id = "another".into();
    duplicate.text = duplicate.text.replace(' ', "  ");
    assert!(p.validate(&duplicate).is_err());
}
#[test]
fn bad_model_outputs_fail_and_contradiction_overrides_watch() {
    let (item, mut r) = news();
    let mut p = NewsPipeline::default();
    r.answers.insert(
        "contradictory_or_retracted".into(),
        Answer::Noul { noul: 0.99 },
    );
    assert_eq!(p.ingest(&item, Some(&r)).unwrap().route, Route::Halt);
    let mut p = NewsPipeline::default();
    r.answers
        .insert("matches_station".into(), Answer::Noul { noul: f64::NAN });
    assert!(p.ingest(&item, Some(&r)).is_err());
    let (_, mut r) = news();
    if let Answer::Choice { probabilities, .. } = r.answers.get_mut("action").unwrap() {
        probabilities.insert("watch".into(), 0.5);
    }
    assert!(p.ingest(&item, Some(&r)).is_err());
    let payload = request(&item, "jev-latest");
    assert_eq!(payload["questions"].as_object().unwrap().len(), 6);
}
#[test]
fn journaled_news_halt_recovers_and_blocks_orders() {
    let temp = Temp::new();
    let (item, mut r) = news();
    r.answers.insert(
        "contradictory_or_retracted".into(),
        Answer::Noul { noul: 0.99 },
    );
    let mut j = Journal::open(temp.journal(), None).unwrap();
    j.ingest_news(item, Some(r)).unwrap();
    drop(j);
    let mut j = Journal::open(temp.journal(), None).unwrap();
    let result = j
        .apply(Event::Trade {
            token: "missing".into(),
            side: Side::Buy,
            fair_micro: 600000,
            shares: 10,
            time_ms: 3000,
        })
        .unwrap();
    assert!(result.unwrap_err().starts_with("halted:news:"));
}
#[test]
fn drawdown_halts_new_risk_and_allows_inventory_exit() {
    let temp = Temp::new();
    let mut j = Journal::open(
        temp.journal(),
        Some(Config {
            max_loss_micro: 100000,
            ..Config::default()
        }),
    )
    .unwrap();
    let mut stream = events();
    for e in stream.drain(..2) {
        j.apply(e).unwrap().unwrap();
    }
    j.apply(Event::Trade {
        token: "weather-yes".into(),
        side: Side::Buy,
        fair_micro: 600000,
        shares: 10,
        time_ms: 1001,
    })
    .unwrap()
    .unwrap();
    assert_eq!(
        j.apply(Event::Trade {
            token: "weather-yes".into(),
            side: Side::Buy,
            fair_micro: 600000,
            shares: 10,
            time_ms: 1002
        })
        .unwrap()
        .unwrap_err(),
        "daily_loss"
    );
    assert!(
        j.apply(Event::Rebalance {
            token: "weather-yes".into(),
            fair_micro: 200000,
            target_shares: 0,
            time_ms: 1003
        })
        .unwrap()
        .unwrap()
        .is_some()
    );
    assert_eq!(j.engine().position("weather-yes"), 0);
}

#[test]
fn legacy_base_journal_rejects_without_rewriting_history() {
    use sha2::{Digest, Sha256};
    let temp = Temp::new();
    let config = Config {
        chain_id: 8453,
        ..Config::default()
    };
    let payload = serde_json::json!({"record":"config", "version":1, "config":config}).to_string();
    let previous = "0".repeat(64);
    let mut h = Sha256::new();
    h.update(1u64.to_le_bytes());
    h.update(previous.as_bytes());
    h.update(payload.as_bytes());
    let frame = serde_json::json!({"number":1,"previous":previous,"payload":payload,"hash":format!("{:x}",h.finalize())});
    let original = format!("{frame}\n");
    fs::write(temp.journal(), &original).unwrap();
    let error = Journal::open(temp.journal(), None)
        .err()
        .expect("Base journal must reject");
    assert_eq!(error, "Polygon_only: chain_id_must_be_137");
    assert_eq!(fs::read_to_string(temp.journal()).unwrap(), original);
}
