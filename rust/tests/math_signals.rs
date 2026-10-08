use polymarket_hft::{
    evaluation::{PairedInput, paired_scores},
    math::{binary_quote, fraction_budget},
    model::{Bucket, Metric, probabilities},
    signals::{SignalAction, SignalRequest, generate},
};
fn request() -> SignalRequest {
    serde_json::from_str(
        include_str!("../../fixtures/signals.jsonl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap()
}
fn paired() -> PairedInput {
    serde_json::from_str(include_str!("../../fixtures/paired-forecasts.json")).unwrap()
}
#[test]
fn truncated_normal_matches_half_normal_reference() {
    let bins = [
        Bucket { lo: -100, hi: -1 },
        Bucket { lo: 0, hi: 0 },
        Bucket { lo: 1, hi: 100 },
    ];
    let high = probabilities(&bins, Metric::High, 0.0, 0.0, 1.0, 14.0).unwrap();
    let low = probabilities(&bins, Metric::Low, 0.0, 0.0, 1.0, 14.0).unwrap();
    // 2 * (Phi(0.5) - 0.5), reference normal CDF Phi(0.5)=0.6914624612740131.
    assert_eq!(high[0], 0.0);
    assert_eq!(low[2], 0.0);
    assert!((high[1] - 0.3829249225480262).abs() < 2e-7);
    for i in 0..3 {
        assert!((high[i] - low[2 - i]).abs() < 2e-7);
    }
    assert!((high.iter().sum::<f64>() - 1.0).abs() < 1e-12);
}
#[test]
fn weather_reference_grid_preserves_support_and_mass() {
    let bins = [
        Bucket { lo: -1000, hi: -1 },
        Bucket { lo: 0, hi: 0 },
        Bucket { lo: 1, hi: 1000 },
    ];
    for mean in [-3.0, -0.5, 0.0, 0.5, 3.0] {
        for sigma in [0.1, 1.0, 10.0] {
            for hours in [0.0, 0.01, 6.0, 14.0, 24.0] {
                for metric in [Metric::High, Metric::Low] {
                    let ps = probabilities(&bins, metric, 0.0, mean, sigma, hours).unwrap();
                    assert!(ps.iter().all(|p| p.is_finite() && (0.0..=1.0).contains(p)));
                    assert!((ps.iter().sum::<f64>() - 1.0).abs() < 1e-12);
                    match metric {
                        Metric::High => assert_eq!(ps[0], 0.0),
                        Metric::Low => assert_eq!(ps[2], 0.0),
                    }
                }
            }
        }
    }
}
#[test]
fn binary_math_exact_and_extreme_budget_checked() {
    let q = binary_quote(600000, 450000, 20).unwrap();
    assert_eq!(q.fee_micro, 900);
    assert_eq!(q.effective_cost_micro, 450900);
    assert_eq!(q.expected_edge_micro, 149100);
    assert_eq!(q.full_kelly_fraction_micro, 271535);
    assert_eq!(fraction_budget(u64::MAX, 1_000_000).unwrap(), u64::MAX);
    assert_eq!(fraction_budget(19, 500_000).unwrap(), 9);
    assert!(fraction_budget(10, 1_000_001).is_err());
    assert_eq!(
        binary_quote(1_000_000, 999999, 10_000)
            .unwrap()
            .full_kelly_fraction_micro,
        0
    );
    for p in [0, 1, 250000, 500000, 999999, 1000000] {
        for price in [1, 250000, 500000, 999999] {
            let q = binary_quote(p, price, 20).unwrap();
            assert!(q.full_kelly_fraction_micro <= 1_000_000);
            assert_eq!(q.full_kelly_fraction_micro > 0, q.expected_edge_micro > 0);
        }
    }
}
#[test]
fn signal_budget_candidate_and_engine_contract() {
    let r = request();
    let s = generate(&r).unwrap();
    assert_eq!(s.action, SignalAction::BuyCandidate);
    assert_eq!(s.conservative_probability_micro, 600000);
    assert_eq!(s.shares, 20);
    assert_eq!(s.total_cost_micro, 9018000);
    assert!(s.total_cost_micro <= s.budget_micro && s.budget_micro <= r.max_order_micro);
    assert_eq!(s.expires_ms, 2900);
    let mut engine = polymarket_hft::engine::Engine::new(Default::default()).unwrap();
    for line in include_str!("../../fixtures/weather.jsonl").lines().take(2) {
        engine.apply(serde_json::from_str(line).unwrap()).unwrap();
    }
    let fill = engine.apply(s.suggested_event.unwrap()).unwrap().unwrap();
    assert_eq!(fill.shares, s.shares);
    assert_eq!(fill.notional_micro + fill.fee_micro, s.total_cost_micro);
}
#[test]
fn signals_fail_closed_for_chain_time_and_missing_evidence() {
    let mut r = request();
    r.chain_id = 137;
    assert!(generate(&r).is_err());
    r = request();
    r.generated_ms = 2901;
    assert!(generate(&r).is_err());
    r = request();
    r.forecast_ms = 1101;
    assert!(generate(&r).is_err());
    r = request();
    r.evidence_ref.clear();
    assert!(generate(&r).is_err());
    r = request();
    r.probability_micro = 1_000_001;
    assert!(generate(&r).is_err());
    r = request();
    r.quoted_ms = u64::MAX;
    r.forecast_ms = u64::MAX;
    r.generated_ms = u64::MAX;
    assert!(generate(&r).is_err());
}
#[test]
fn uncertain_signals_and_budget_cannot_create_orders() {
    let mut r = request();
    r.uncertainty_margin_micro = 600000;
    let s = generate(&r).unwrap();
    assert_eq!(s.action, SignalAction::NoEdge);
    assert!(s.suggested_event.is_none());
    r = request();
    r.capital_micro = 1;
    let s = generate(&r).unwrap();
    assert_eq!(s.action, SignalAction::InsufficientBudget);
    assert_eq!(s.shares, 0);
}
#[test]
fn paired_scores_use_identical_records_and_report_provenance_limits() {
    let r = paired_scores(&paired()).unwrap();
    assert!((r.candidate.brier - 0.065).abs() < 1e-12);
    assert_eq!(r.reference.brier, 0.25);
    assert!((r.candidate_minus_reference_brier + 0.185).abs() < 1e-12);
    assert!(r.candidate_lower_on_both_scores);
    assert!(!r.model_inference_measured && !r.provenance_verified && r.synthetic);
    assert!(r.mean_brier_difference_standard_error.unwrap() > 0.0);
}
#[test]
fn paired_evaluation_rejects_leakage_duplicates_and_invalid_probabilities() {
    let mut r = paired();
    r.records[0].predicted_ms = 3;
    assert!(paired_scores(&r).is_err());
    r = paired();
    r.records[1].id = r.records[0].id.clone();
    assert!(paired_scores(&r).is_err());
    r = paired();
    r.records[0].candidate_probability = f64::NAN;
    assert!(paired_scores(&r).is_err());
    r = paired();
    r.split = "train".into();
    assert!(paired_scores(&r).is_err());
    r = paired();
    r.reference_model = r.candidate_model.clone();
    assert!(paired_scores(&r).is_err());
}
