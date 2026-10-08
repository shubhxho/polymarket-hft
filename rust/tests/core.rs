use polymarket_hft::{
    book::{Book, Level, Side},
    engine::{Config, Engine, Event},
    model::{Bucket, Metric, probabilities},
    quant::Quantized,
};

fn event(line: &str) -> Event {
    serde_json::from_str(line).unwrap()
}
fn engine(config: Config) -> Engine {
    let mut e = Engine::new(config).unwrap();
    e.apply(event(
        r#"{"type":"market","token":"t","city":"nyc","tick":10000,"chain_id":8453,"fee_bps":20}"#,
    ))
    .unwrap();
    e.apply(event(r#"{"type":"snapshot","token":"t","bids":[{"price":400000,"shares":100}],"asks":[{"price":450000,"shares":10},{"price":460000,"shares":20},{"price":600000,"shares":100}],"sequence":1,"time_ms":1000}"#)).unwrap();
    e
}
fn trade(side: Side, shares: u32, fair: u32, time: u64) -> Event {
    Event::Trade {
        token: "t".into(),
        side,
        fair_micro: fair,
        shares,
        time_ms: time,
    }
}

#[test]
fn base_only_on_config_and_market() {
    assert!(
        Engine::new(Config {
            chain_id: 137,
            ..Config::default()
        })
        .is_err()
    );
    let mut e = Engine::new(Config::default()).unwrap();
    assert!(
        e.apply(event(
            r#"{"type":"market","token":"t","city":"nyc","tick":10000,"chain_id":137,"fee_bps":0}"#
        ))
        .is_err()
    );
}
#[test]
fn sweep_uses_actual_depth_and_fee_then_depletes() {
    let mut e = engine(Config::default());
    let fill = e
        .apply(trade(Side::Buy, 40, 550000, 1001))
        .unwrap()
        .unwrap();
    assert_eq!(fill.shares, 30);
    assert_eq!(fill.notional_micro, 13_700_000);
    assert_eq!(fill.fee_micro, 27_400);
    assert_eq!(fill.worst_price_micro, 460000);
    assert_eq!(e.position("t"), 30);
    assert_eq!(e.cash_micro(), 986_272_600);
    assert!(e.apply(trade(Side::Buy, 40, 550000, 1002)).is_err());
    assert_eq!(e.position("t"), 30);
}
#[test]
fn cash_order_city_and_inventory_caps_include_fees() {
    for config in [
        Config {
            cash_micro: 3_000_000,
            ..Config::default()
        },
        Config {
            max_order_micro: 3_000_000,
            ..Config::default()
        },
        Config {
            max_city_micro: 3_000_000,
            ..Config::default()
        },
        Config {
            max_position: 6,
            ..Config::default()
        },
    ] {
        let mut e = engine(config);
        let f = e
            .apply(trade(Side::Buy, 100, 550000, 1001))
            .unwrap()
            .unwrap();
        assert_eq!(f.shares, 6);
    }
}
#[test]
fn sell_cannot_short_and_releases_city_budget() {
    let mut e = engine(Config {
        max_city_micro: 3_000_000,
        ..Config::default()
    });
    assert!(e.apply(trade(Side::Sell, 50, 200000, 1001)).is_err());
    e.apply(trade(Side::Buy, 50, 550000, 1002)).unwrap();
    let fill = e
        .apply(trade(Side::Sell, 50, 200000, 1003))
        .unwrap()
        .unwrap();
    assert_eq!(fill.shares, 6);
    assert_eq!(e.position("t"), 0);
    assert!(e.realized_pnl_micro() < 0);
    assert!(e.apply(trade(Side::Buy, 50, 550000, 1004)).is_ok());
}
#[test]
fn loss_kill_switch_and_rate_limit() {
    let mut e = engine(Config {
        max_loss_micro: 100000,
        ..Config::default()
    });
    e.apply(trade(Side::Buy, 10, 550000, 1001)).unwrap();
    e.apply(trade(Side::Sell, 10, 200000, 1002)).unwrap();
    assert_eq!(
        e.apply(trade(Side::Buy, 10, 550000, 1003)).unwrap_err(),
        "daily_loss"
    );
    let mut e = engine(Config {
        max_orders_per_second: 1,
        ..Config::default()
    });
    e.apply(trade(Side::Buy, 5, 550000, 1001)).unwrap();
    assert_eq!(
        e.apply(trade(Side::Buy, 5, 550000, 1002)).unwrap_err(),
        "rate_limit"
    );
    assert!(e.apply(trade(Side::Buy, 5, 550000, 2001)).is_ok());
}
#[test]
fn stale_future_and_reversed_clocks_reject() {
    let mut e = engine(Config::default());
    assert!(e.apply(trade(Side::Buy, 5, 550000, 999)).is_err());
    assert!(e.apply(trade(Side::Buy, 5, 550000, 4000)).is_err());
    assert_eq!(
        e.apply(trade(Side::Buy, 5, 550000, 1001)).unwrap_err(),
        "invalid_trade_or_time"
    );
}
#[test]
fn sequence_gap_requires_snapshot_and_crossed_update_is_atomic() {
    let mut b = Book::new(1000).unwrap();
    let bids = [Level {
        price: 400000,
        shares: 10,
    }];
    let asks = [Level {
        price: 450000,
        shares: 10,
    }];
    b.snapshot(&bids, &asks, 1, 1000).unwrap();
    assert!(b.update(Side::Buy, bids[0], 3, 1001).is_err());
    assert!(!b.ready(1001, 2000));
    assert!(b.update(Side::Buy, bids[0], 2, 1002).is_err());
    b.snapshot(&bids, &asks, 4, 1003).unwrap();
    assert!(
        b.update(
            Side::Buy,
            Level {
                price: 460000,
                shares: 10
            },
            5,
            1004
        )
        .is_err()
    );
    assert_eq!(b.best_bid(), Some(400000));
    assert!(!b.ready(1004, 2000));
}
#[test]
fn no_silent_tick_clamping_or_duplicate_snapshot_levels() {
    for tick in [0, 1, 3000, 1_000_001] {
        assert!(Book::new(tick).is_err());
    }
    let mut b = Book::new(1000).unwrap();
    assert!(
        b.snapshot(
            &[Level {
                price: 400001,
                shares: 10
            }],
            &[],
            1,
            1000
        )
        .is_err()
    );
    assert!(
        b.snapshot(
            &[Level {
                price: 400000,
                shares: 10
            }; 2],
            &[],
            1,
            1000
        )
        .is_err()
    );
}
#[test]
fn weather_high_low_final_observation_and_mass() {
    let bins = [
        Bucket { lo: -100, hi: 69 },
        Bucket { lo: 70, hi: 71 },
        Bucket { lo: 72, hi: 200 },
    ];
    let high = probabilities(&bins, Metric::High, 72.0, 73.0, 1.0, 6.0).unwrap();
    assert_eq!(high[0], 0.0);
    assert_eq!(high[1], 0.0);
    let low = probabilities(&bins, Metric::Low, 69.0, 68.0, 1.0, 6.0).unwrap();
    assert_eq!(low[1], 0.0);
    assert_eq!(low[2], 0.0);
    assert_eq!(
        probabilities(&bins, Metric::High, 70.0, 90.0, 3.0, 0.0).unwrap(),
        vec![0.0, 1.0, 0.0]
    );
    assert!(probabilities(&bins, Metric::High, f64::NAN, 70.0, 1.0, 3.0).is_err());
    assert!(
        probabilities(
            &[Bucket { lo: 70, hi: 71 }],
            Metric::High,
            70.0,
            75.0,
            2.0,
            6.0
        )
        .is_err()
    );
}
#[test]
fn quantization_memory_and_error_bound() {
    let weights: Vec<f32> = (0..128).map(|i| i as f32 / 128.0).collect();
    let input = vec![1.0; 128];
    let reference: f32 = weights.iter().sum();
    for bits in [2, 4, 8] {
        let q = Quantized::new(&weights, bits, 64).unwrap();
        assert!(q.storage_bytes() < weights.len() * 4);
        let bound = 128.0 * (63.0 / 128.0) / (2.0 * ((1u32 << bits) - 1) as f32);
        assert!((q.dot(&input).unwrap() - reference).abs() <= bound + 1e-4);
        assert!(q.dot(&[1.0]).is_err());
    }
    assert!(Quantized::new(&weights, 3, 64).is_err());
    assert!(Quantized::new(&[f32::NAN; 64], 4, 64).is_err());
}
#[test]
fn json_schema_strict_and_replay_deterministic() {
    assert!(serde_json::from_str::<Event>(r#"{"type":"halt","reason":"test","typo":1}"#).is_err());
    let replay = || {
        let mut e = Engine::new(Config::default()).unwrap();
        include_str!("../../fixtures/smoke.jsonl")
            .lines()
            .map(|l| match e.apply(event(l)) {
                Ok(Some(f)) => serde_json::to_string(&f).unwrap(),
                Ok(None) => "ok".into(),
                Err(e) => e,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(replay(), replay());
}

#[cfg(any(feature = "mlx", feature = "mlx-cpu"))]
#[test]
fn native_mlx_quantization_matches_constant_reference() {
    for bits in [2, 4, 8] {
        let q = polymarket_hft::mlx::MlxLinear::new(&[0.5; 64], bits).unwrap();
        assert!((q.dot(&[1.0; 64]).unwrap() - 32.0).abs() < 1e-4);
        assert!(
            q.dot_batch(&[1.0; 128])
                .unwrap()
                .iter()
                .all(|v| (*v - 32.0).abs() < 1e-4)
        );
    }
}

#[test]
fn randomized_book_matches_ordered_reference() {
    use std::collections::BTreeMap;
    let mut b = Book::new(1000).unwrap();
    b.snapshot(&[], &[], 1, 1).unwrap();
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    let mut seed = 123456789u64;
    for seq in 2..10002 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let side = if seed & 1 == 0 { Side::Buy } else { Side::Sell };
        let price = if side == Side::Buy {
            1 + ((seed >> 16) % 400) as u32
        } else {
            600 + ((seed >> 16) % 399) as u32
        } * 1000;
        let shares = ((seed >> 32) % 20) as u32;
        b.update(side, Level { price, shares }, seq, seq).unwrap();
        let reference = if side == Side::Buy {
            &mut bids
        } else {
            &mut asks
        };
        if shares == 0 {
            reference.remove(&price);
        } else {
            reference.insert(price, shares);
        }
        assert_eq!(b.best_bid(), bids.last_key_value().map(|(&p, _)| p));
        assert_eq!(b.best_ask(), asks.first_key_value().map(|(&p, _)| p));
        assert_eq!(
            b.levels(Side::Buy)
                .map(|l| (l.price, l.shares))
                .collect::<Vec<_>>(),
            asks.iter().map(|(&p, &s)| (p, s)).collect::<Vec<_>>()
        );
        assert_eq!(
            b.levels(Side::Sell)
                .map(|l| (l.price, l.shares))
                .collect::<Vec<_>>(),
            bids.iter().rev().map(|(&p, &s)| (p, s)).collect::<Vec<_>>()
        );
    }
}

#[test]
fn weather_signal_runs_through_execution_risk() {
    let mut e = engine(Config::default());
    let fill = e
        .apply(Event::Weather {
            tokens: vec!["t".into()],
            buckets: vec![Bucket { lo: -100, hi: 200 }],
            metric: Metric::High,
            observed: 72.0,
            mu: 72.0,
            sigma: 1.0,
            hours: 0.0,
            shares: 10,
            time_ms: 1001,
        })
        .unwrap()
        .unwrap();
    assert_eq!(fill.shares, 10);
    assert_eq!(fill.worst_price_micro, 450000);
}

#[test]
fn malformed_update_disables_execution_until_snapshot() {
    let mut b = Book::new(1000).unwrap();
    b.snapshot(
        &[],
        &[Level {
            price: 450000,
            shares: 10,
        }],
        1,
        1000,
    )
    .unwrap();
    assert!(
        b.update(
            Side::Sell,
            Level {
                price: 450001,
                shares: 10
            },
            2,
            1001
        )
        .is_err()
    );
    assert!(!b.ready(1001, 2000));
    b.snapshot(
        &[],
        &[Level {
            price: 450000,
            shares: 10,
        }],
        3,
        1002,
    )
    .unwrap();
    assert!(b.ready(1002, 2000));
}
