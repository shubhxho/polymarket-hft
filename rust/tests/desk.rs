use polymarket_hft::{
    desk,
    signals::{SignalRequest, generate},
};
fn candidate() -> SignalRequest {
    let mut r: SignalRequest = serde_json::from_str(
        include_str!("../../fixtures/signals.jsonl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    r.fee_bps = 0;
    r.fee_rate_micro = Some(50_000);
    r
}
#[test]
fn ranking_rechecks_wall_clock_and_market_fees() {
    let r = candidate();
    let valid = desk::rank(vec![r.clone()], 1100).unwrap();
    assert_eq!(valid["best_candidate_id"], r.id);
    assert_eq!(valid["signals"][0]["math"]["effective_cost_micro"], 462375);
    assert_eq!(
        valid["signals"][0]["fee_model"],
        "polymarket_price_curve_cash_bound"
    );
    let stale = desk::rank(vec![r.clone()], 4000).unwrap();
    assert!(stale["best_candidate_id"].is_null());
    assert_eq!(stale["rejected"][0]["reason"], "signal_clock_or_age");
    let mut missing = r.clone();
    missing.fee_rate_micro = None;
    assert_eq!(
        desk::rank(vec![missing], 1100).unwrap()["rejected"][0]["reason"],
        "actual_market_fee_rate_required"
    );
    let mut ambiguous = r;
    ambiguous.fee_bps = 20;
    assert!(generate(&ambiguous).is_err());
}
#[test]
fn ranking_order_duplicates_and_bounds() {
    let r = candidate();
    let mut worse = r.clone();
    worse.id = "worse".into();
    worse.token = "second-token".into();
    worse.probability_micro = 550000;
    let out = desk::rank(vec![worse, r.clone(), r.clone()], 1100).unwrap();
    assert_eq!(out["signals"][0]["id"], r.id);
    assert_eq!(out["rejected"][0]["reason"], "duplicate_token");
    assert!(desk::rank(Vec::new(), 1100).is_err());
    assert!(desk::rank(vec![r; 65], 1100).is_err());
}
#[test]
fn http_rejects_rebinding_cross_origin_smuggling_and_large_bodies() {
    for s in [
        "GET / HTTP/1.1\r\nHost: evil.example\r\n\r\n",
        "GET / HTTP/1.1\r\nHost: localhost:8787\r\nOrigin: https://evil.example\r\n\r\n",
        "POST / HTTP/1.1\r\nHost: localhost:8787\r\nContent-Length: 1\r\nContent-Length: 1\r\n\r\nx",
        "POST / HTTP/1.1\r\nHost: localhost:8787\r\nTransfer-Encoding: chunked\r\n\r\n",
        "POST / HTTP/1.1\r\nHost: localhost:8787\r\nContent-Length: 65537\r\n\r\n",
    ] {
        assert!(desk::read_request(&mut s.as_bytes(), 8787).is_err());
    }
    let header = "A".repeat(8193);
    assert!(desk::read_request(&mut header.as_bytes(), 8787).is_err());
    let valid = "POST /api/signals HTTP/1.1\r\nHost: 127.0.0.1:8787\r\nContent-Type: application/json\r\nX-HFT-Desk: 1\r\nContent-Length: 2\r\n\r\n[]";
    assert_eq!(
        desk::read_request(&mut valid.as_bytes(), 8787)
            .unwrap()
            .body,
        b"[]"
    );
}
