//! Deterministic synthetic execution stress; no fitted alpha or live readiness claim.
use crate::{
    Result,
    book::{Level, Side},
    engine::{Config, Engine, Event},
    polygon::CHAIN_ID,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
struct Scenario {
    name: &'static str,
    spread: u32,
    fee_bps: u32,
    latency_ms: u64,
    depth: u32,
    gap_every: u32,
    bias: i64,
    burst: bool,
    shock: bool,
}
#[derive(Clone, Copy)]
struct Policy {
    name: &'static str,
    edge: u32,
    position: u32,
    order: u64,
}
#[derive(Debug, Serialize, PartialEq)]
pub struct PathResult {
    pub scenario: String,
    pub policy: String,
    pub seed: u64,
    pub events: u64,
    pub fills: u64,
    pub fees_micro: u64,
    pub traded_notional_micro: u64,
    pub pnl_micro: i128,
    pub realized_pnl_micro: i128,
    pub max_drawdown_micro: u64,
    pub ending_position: u32,
    pub rejects: BTreeMap<String, u64>,
    pub invariant_checks: u64,
}
#[derive(Serialize)]
pub struct Summary {
    pub scenario: String,
    pub policy: String,
    pub paths: usize,
    pub median_pnl_micro: i128,
    pub worst_pnl_micro: i128,
    pub best_pnl_micro: i128,
    pub profitable_paths: usize,
    pub max_drawdown_micro: u64,
}
#[derive(Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub package_version: &'static str,
    pub chain_id: u64,
    pub synthetic: bool,
    pub live_ready: bool,
    pub profitability_established: bool,
    pub simulation_gate_passed: bool,
    pub steps_per_path: u32,
    pub seeds_per_scenario: u32,
    pub invariant_checks: u64,
    pub paths: Vec<PathResult>,
    pub summaries: Vec<Summary>,
    pub assumptions: Vec<&'static str>,
}
fn random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state
}
fn path(s: Scenario, p: Policy, seed: u64, steps: u32) -> Result<PathResult> {
    let config = Config {
        max_position: p.position,
        max_order_micro: p.order,
        min_edge_micro: p.edge,
        max_loss_micro: 5_000_000,
        ..Config::default()
    };
    let mut engine = Engine::new(config.clone())?;
    engine.apply(Event::Market {
        token: "synthetic".into(),
        city: "synthetic".into(),
        tick: 1000,
        chain_id: CHAIN_ID,
        fee_bps: s.fee_bps,
    })?;
    let mut out = PathResult {
        scenario: s.name.into(),
        policy: p.name.into(),
        seed,
        events: 1,
        fills: 0,
        fees_micro: 0,
        traded_notional_micro: 0,
        pnl_micro: 0,
        realized_pnl_micro: 0,
        max_drawdown_micro: 0,
        ending_position: 0,
        rejects: BTreeMap::new(),
        invariant_checks: 0,
    };
    let mut state = seed;
    let mut latent = 500_000i64;
    let mut clock = 1000;
    let mut peak = config.cash_micro;
    let mut last_bid = 400_000;
    for i in 0..steps {
        latent = (latent + (random(&mut state) % 11) as i64 * 1000 - 5000).clamp(300_000, 700_000);
        let shock = if s.shock && i >= steps / 2 + 3 {
            150_000
        } else {
            0
        };
        let noise = (random(&mut state) % 41) as i64 * 1000 - 20_000;
        let mid = (latent + noise - shock).clamp(200_000, 800_000) as u32;
        let bid = mid - s.spread / 2;
        let ask = mid + s.spread / 2;
        last_bid = bid;
        let sequence = u64::from(i) * 3 + 1;
        engine.apply(Event::Snapshot {
            token: "synthetic".into(),
            bids: vec![Level {
                price: bid,
                shares: s.depth,
            }],
            asks: vec![Level {
                price: ask,
                shares: s.depth,
            }],
            sequence,
            time_ms: clock,
        })?;
        out.events += 1;
        if s.gap_every != 0 && i % s.gap_every == 0 {
            let rejected = engine.apply(Event::Update {
                token: "synthetic".into(),
                side: Side::Buy,
                level: Level {
                    price: bid,
                    shares: s.depth,
                },
                sequence: sequence + 2,
                time_ms: clock,
            });
            if rejected.is_ok() {
                return Err("simulation_sequence_gap_accepted".into());
            }
            out.events += 1;
        }
        // Periodic inventory exit is part of both policies, not selected after observing returns.
        let side = if i % 8 == 7 { Side::Sell } else { Side::Buy };
        let fair = if side == Side::Sell {
            0
        } else {
            (latent + s.bias - shock).clamp(0, 1_000_000) as u32
        };
        let before_cash = engine.cash_micro();
        let before_position = engine.position("synthetic");
        let now = clock + s.latency_ms;
        match engine.apply(Event::Trade {
            token: "synthetic".into(),
            side,
            fair_micro: fair,
            shares: p.position,
            time_ms: now,
        }) {
            Ok(Some(fill)) => {
                if !fill.paper
                    || fill.chain_id != CHAIN_ID
                    || fill.shares > s.depth
                    || fill.notional_micro > config.max_order_micro
                {
                    return Err("simulation_fill_limit_failure".into());
                }
                if side == Side::Buy {
                    if fill.notional_micro + fill.fee_micro > config.max_order_micro
                        || before_cash.checked_sub(fill.notional_micro + fill.fee_micro)
                            != Some(fill.cash_micro)
                        || engine.position("synthetic") != before_position + fill.shares
                    {
                        return Err("simulation_buy_accounting_failure".into());
                    }
                } else if before_cash.checked_add(fill.notional_micro - fill.fee_micro)
                    != Some(fill.cash_micro)
                    || before_position.checked_sub(fill.shares)
                        != Some(engine.position("synthetic"))
                {
                    return Err("simulation_sell_accounting_failure".into());
                }
                out.fills += 1;
                out.fees_micro += fill.fee_micro;
                out.traded_notional_micro += fill.notional_micro;
            }
            Ok(None) => return Err("simulation_missing_trade_result".into()),
            Err(reason) => {
                if engine.cash_micro() != before_cash
                    || engine.position("synthetic") != before_position
                {
                    return Err("simulation_rejection_changed_holdings".into());
                }
                *out.rejects.entry(reason).or_default() += 1;
            }
        }
        if engine.position("synthetic") > config.max_position {
            return Err("simulation_inventory_limit_failure".into());
        }
        let equity = engine.liquidation_equity_micro(now);
        peak = peak.max(equity);
        out.max_drawdown_micro = out.max_drawdown_micro.max(peak.saturating_sub(equity));
        out.events += 1;
        out.invariant_checks += 1;
        clock = now + if s.burst { 1 } else { 250 };
    }
    // End with a fresh mark and report remaining inventory rather than inventing settlement.
    engine.apply(Event::Snapshot {
        token: "synthetic".into(),
        bids: vec![Level {
            price: last_bid,
            shares: s.depth,
        }],
        asks: vec![Level {
            price: last_bid + s.spread,
            shares: s.depth,
        }],
        sequence: u64::from(steps) * 3 + 1,
        time_ms: clock,
    })?;
    out.events += 1;
    out.pnl_micro =
        i128::from(engine.liquidation_equity_micro(clock)) - i128::from(config.cash_micro);
    out.realized_pnl_micro = engine.realized_pnl_micro();
    out.ending_position = engine.position("synthetic");
    Ok(out)
}
pub fn run(steps: u32, seeds: u32) -> Result<Report> {
    if !(8..=10_000).contains(&steps) || !(1..=64).contains(&seeds) {
        return Err("invalid_simulation_bounds".into());
    }
    let base = Scenario {
        name: "normal",
        spread: 10_000,
        fee_bps: 20,
        latency_ms: 5,
        depth: 100,
        gap_every: 0,
        bias: 0,
        burst: false,
        shock: false,
    };
    let scenarios = [
        base,
        Scenario {
            name: "delayed",
            latency_ms: 3500,
            ..base
        },
        Scenario {
            name: "fee_spike",
            fee_bps: 1000,
            spread: 40_000,
            ..base
        },
        Scenario {
            name: "thin",
            depth: 1,
            ..base
        },
        Scenario {
            name: "sequence_gaps",
            gap_every: 7,
            ..base
        },
        Scenario {
            name: "bursts",
            latency_ms: 0,
            burst: true,
            ..base
        },
        Scenario {
            name: "biased_crash",
            bias: 120_000,
            shock: true,
            ..base
        },
    ];
    let policies = [
        Policy {
            name: "aggressive",
            edge: 0,
            position: 100,
            order: 25_000_000,
        },
        Policy {
            name: "conservative",
            edge: 20_000,
            position: 25,
            order: 10_000_000,
        },
        Policy {
            name: "strict_edge",
            edge: 40_000,
            position: 25,
            order: 10_000_000,
        },
    ];
    let mut paths = Vec::new();
    let mut summaries = Vec::new();
    for s in scenarios {
        for p in policies {
            let start = paths.len();
            // Fixed held-out seed schedule; no tuning or parameter selection on these outcomes.
            for seed in 0..seeds {
                paths.push(path(s, p, 10_000 + u64::from(seed), steps)?);
            }
            let slice = &paths[start..];
            let mut pnls: Vec<_> = slice.iter().map(|v| v.pnl_micro).collect();
            pnls.sort_unstable();
            summaries.push(Summary {
                scenario: s.name.into(),
                policy: p.name.into(),
                paths: slice.len(),
                median_pnl_micro: pnls[pnls.len() / 2],
                worst_pnl_micro: pnls[0],
                best_pnl_micro: *pnls.last().unwrap(),
                profitable_paths: pnls.iter().filter(|&&v| v > 0).count(),
                max_drawdown_micro: slice.iter().map(|v| v.max_drawdown_micro).max().unwrap(),
            });
        }
    }
    Ok(Report {
        schema_version: 1,
        package_version: env!("CARGO_PKG_VERSION"),
        chain_id: CHAIN_ID,
        synthetic: true,
        live_ready: false,
        profitability_established: false,
        simulation_gate_passed: true,
        steps_per_path: steps,
        seeds_per_scenario: seeds,
        invariant_checks: paths.iter().map(|v| v.invariant_checks).sum(),
        paths,
        summaries,
        assumptions: vec![
            "Toy bounded random-walk latent price and noisy quote; forecast observes synthetic latent state, never future outcomes",
            "Fixed policies and separate seed schedule; not historical out-of-sample evidence or calibrated alpha",
            "Periodic taker exits; no maker queue, market impact, binary resolution, gas, borrow or real network feed",
            "Flat replay fees; a delayed scenario exceeds book freshness, but does not model a calibrated price-latency relationship",
            "Remaining inventory is marked conservatively at fresh bid depth; P&L includes charged fees",
            "Passing means engine accounting and configured limits held; losses are reported and do not fail the correctness gate",
        ],
    })
}
