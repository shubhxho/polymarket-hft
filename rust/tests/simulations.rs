use polymarket_hft::simulation;
#[test]
fn stress_is_deterministic_and_reports_real_failures_and_loss() {
    let a = simulation::run(64, 2).unwrap();
    let b = simulation::run(64, 2).unwrap();
    assert_eq!(a.paths, b.paths);
    assert_eq!(a.invariant_checks, 64 * 2 * 7 * 3);
    assert!(
        a.simulation_gate_passed && a.synthetic && !a.live_ready && !a.profitability_established
    );
    for (scenario, reason) in [
        ("delayed", "stale_or_unsynchronized_book"),
        ("thin", "insufficient_edge_liquidity_or_budget"),
        ("sequence_gaps", "stale_or_unsynchronized_book"),
        ("bursts", "rate_limit"),
        ("biased_crash", "daily_loss"),
    ] {
        assert!(
            a.paths.iter().filter(|p| p.scenario == scenario).any(|p| p
                .rejects
                .get(reason)
                .copied()
                .unwrap_or(0)
                > 0),
            "missing {scenario} / {reason}"
        );
    }
    assert!(a.paths.iter().any(|p| p.fills > 0 && p.fees_micro > 0));
    assert!(a.paths.iter().any(|p| p.pnl_micro < 0));
    assert!(a.paths.iter().all(|p| p.ending_position <= 100));
}
#[test]
fn stress_work_is_bounded() {
    for (steps, seeds) in [(0, 1), (7, 1), (10001, 1), (8, 0), (8, 65)] {
        assert!(simulation::run(steps, seeds).is_err());
    }
}
