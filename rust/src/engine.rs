use crate::{
    Result,
    base::{CHAIN_ID, PRICE_SCALE},
    book::{Book, Level, Side},
    model::{Bucket, Metric, probabilities},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub chain_id: u64,
    pub cash_micro: u64,
    pub max_order_micro: u64,
    pub max_city_micro: u64,
    pub max_position: u32,
    pub min_shares: u32,
    pub min_edge_micro: u32,
    pub max_book_age_ms: u64,
    pub max_orders_per_second: usize,
    pub max_loss_micro: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            chain_id: CHAIN_ID,
            cash_micro: 1_000_000_000,
            max_order_micro: 25_000_000,
            max_city_micro: 150_000_000,
            max_position: 500,
            min_shares: 5,
            min_edge_micro: 40_000,
            max_book_age_ms: 2000,
            max_orders_per_second: 8,
            max_loss_micro: 75_000_000,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Weather {
        tokens: Vec<String>,
        buckets: Vec<Bucket>,
        metric: Metric,
        observed: f64,
        mu: f64,
        sigma: f64,
        hours: f64,
        shares: u32,
        time_ms: u64,
    },
    Market {
        token: String,
        city: String,
        tick: u32,
        chain_id: u64,
        fee_bps: u32,
    },
    Snapshot {
        token: String,
        bids: Vec<Level>,
        asks: Vec<Level>,
        sequence: u64,
        time_ms: u64,
    },
    Update {
        token: String,
        side: Side,
        level: Level,
        sequence: u64,
        time_ms: u64,
    },
    Trade {
        token: String,
        side: Side,
        fair_micro: u32,
        shares: u32,
        time_ms: u64,
    },
    Halt {
        reason: String,
    },
}

struct Market {
    city: String,
    fee_bps: u32,
    book: Book,
    position: u32,
    cost: u64,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Fill {
    pub token: String,
    pub side: Side,
    pub shares: u32,
    pub notional_micro: u64,
    pub fee_micro: u64,
    pub vwap_micro: u32,
    pub worst_price_micro: u32,
    pub cash_micro: u64,
    pub time_ms: u64,
    pub paper: bool,
    pub chain_id: u64,
}

pub struct Engine {
    pub config: Config,
    markets: BTreeMap<String, Market>,
    pub cash: u64,
    pub realized_pnl: i128,
    halted: Option<String>,
    attempts: VecDeque<u64>,
    last_time: u64,
}

// Conservative flat bps of notional for replay; use verified per-market schedules for live.
pub fn fee(notional: u64, bps: u32) -> u64 {
    (notional * u64::from(bps)).div_ceil(10_000)
}

impl Engine {
    pub fn new(config: Config) -> Result<Self> {
        if config.chain_id != CHAIN_ID {
            return Err("Base_only: chain_id_must_be_8453".into());
        }
        if config.max_order_micro == 0
            || config.max_city_micro == 0
            || config.max_position == 0
            || config.min_shares == 0
            || config.max_orders_per_second == 0
            || config.max_orders_per_second > 10_000
            || config.max_loss_micro == 0
            || config.min_edge_micro >= PRICE_SCALE
            || config.cash_micro > i64::MAX as u64
        {
            return Err("invalid_risk_config".into());
        }
        Ok(Self {
            cash: config.cash_micro,
            config,
            markets: BTreeMap::new(),
            realized_pnl: 0,
            halted: None,
            attempts: VecDeque::new(),
            last_time: 0,
        })
    }
    pub fn position(&self, token: &str) -> u32 {
        self.markets.get(token).map_or(0, |m| m.position)
    }
    pub fn apply(&mut self, event: Event) -> Result<Option<Fill>> {
        match event {
            Event::Weather {
                tokens,
                buckets,
                metric,
                observed,
                mu,
                sigma,
                hours,
                shares,
                time_ms,
            } => {
                if tokens.len() != buckets.len() || tokens.is_empty() {
                    return Err("weather_token_bucket_mismatch".into());
                }
                let mut unique = std::collections::BTreeSet::new();
                for token in &tokens {
                    if !unique.insert(token) || !self.markets.contains_key(token) {
                        return Err("unknown_or_duplicate_weather_token".into());
                    }
                }
                let city = &self.markets[&tokens[0]].city;
                if tokens.iter().any(|t| &self.markets[t].city != city) {
                    return Err("weather_city_mismatch".into());
                }
                let p = probabilities(&buckets, metric, observed, mu, sigma, hours)?;
                let best = tokens
                    .iter()
                    .zip(p)
                    .filter_map(|(token, probability)| {
                        let m = &self.markets[token];
                        if !m.book.ready(time_ms, self.config.max_book_age_ms) {
                            return None;
                        }
                        let ask = m.book.best_ask()?;
                        let fair =
                            (probability.clamp(0.0, 1.0) * f64::from(PRICE_SCALE)).floor() as u32;
                        let edge = i64::from(fair)
                            - i64::from(ask)
                            - fee(u64::from(ask), m.fee_bps) as i64;
                        (edge >= i64::from(self.config.min_edge_micro)
                            && m.position < self.config.max_position)
                            .then_some((token.clone(), fair, edge))
                    })
                    .max_by_key(|(_, _, edge)| *edge);
                let (token, fair, _) = best.ok_or("no_eligible_weather_edge")?;
                return self
                    .trade(&token, Side::Buy, fair, shares, time_ms)
                    .map(Some);
            }
            Event::Market {
                token,
                city,
                tick,
                chain_id,
                fee_bps,
            } => {
                if chain_id != CHAIN_ID {
                    return Err("Base_only: market_chain_mismatch".into());
                }
                if token.is_empty()
                    || city.is_empty()
                    || fee_bps > 10_000
                    || self.markets.contains_key(&token)
                {
                    return Err("invalid_or_duplicate_market".into());
                }
                self.markets.insert(
                    token,
                    Market {
                        city,
                        fee_bps,
                        book: Book::new(tick)?,
                        position: 0,
                        cost: 0,
                    },
                );
            }
            Event::Snapshot {
                token,
                bids,
                asks,
                sequence,
                time_ms,
            } => {
                self.market(&token)?
                    .book
                    .snapshot(&bids, &asks, sequence, time_ms)?;
            }
            Event::Update {
                token,
                side,
                level,
                sequence,
                time_ms,
            } => {
                self.market(&token)?
                    .book
                    .update(side, level, sequence, time_ms)?;
            }
            Event::Trade {
                token,
                side,
                fair_micro,
                shares,
                time_ms,
            } => {
                return self
                    .trade(&token, side, fair_micro, shares, time_ms)
                    .map(Some);
            }
            Event::Halt { reason } => self.halted = Some(reason),
        }
        Ok(None)
    }
    fn market(&mut self, token: &str) -> Result<&mut Market> {
        self.markets
            .get_mut(token)
            .ok_or_else(|| "unknown_market".into())
    }
    fn trade(
        &mut self,
        token: &str,
        side: Side,
        fair: u32,
        requested: u32,
        now: u64,
    ) -> Result<Fill> {
        if let Some(reason) = &self.halted {
            return Err(format!("halted:{reason}"));
        }
        if fair > PRICE_SCALE || requested == 0 || now < self.last_time {
            return Err("invalid_trade_or_time".into());
        }
        self.last_time = now;
        if self.realized_pnl <= -i128::from(self.config.max_loss_micro) {
            self.halted = Some("daily_loss".into());
            return Err("daily_loss".into());
        }
        while self.attempts.front().is_some_and(|&t| now - t >= 1000) {
            self.attempts.pop_front();
        }
        if self.attempts.len() >= self.config.max_orders_per_second {
            return Err("rate_limit".into());
        }
        let market = self.markets.get(token).ok_or("unknown_market")?;
        if !market.book.ready(now, self.config.max_book_age_ms) {
            return Err("stale_or_unsynchronized_book".into());
        }
        let city_cost: u64 = self
            .markets
            .values()
            .filter(|m| m.city == market.city)
            .map(|m| m.cost)
            .sum();
        let budget = if side == Side::Buy {
            self.config
                .max_order_micro
                .min(self.cash)
                .min(self.config.max_city_micro.saturating_sub(city_cost))
        } else {
            self.config.max_order_micro
        };
        let wanted = requested.min(if side == Side::Buy {
            self.config.max_position.saturating_sub(market.position)
        } else {
            market.position
        });
        let mut fills = Vec::new();
        let (mut total, mut fees, mut quantity) = (0u64, 0u64, 0u32);
        for level in market.book.levels(side) {
            let unit_fee = fee(u64::from(level.price), market.fee_bps);
            let edge = if side == Side::Buy {
                i64::from(fair) - i64::from(level.price) - unit_fee as i64
            } else {
                i64::from(level.price) - i64::from(fair) - unit_fee as i64
            };
            if edge < i64::from(self.config.min_edge_micro) {
                break;
            }
            let unit = u64::from(level.price) + if side == Side::Buy { unit_fee } else { 0 };
            let spent = total + if side == Side::Buy { fees } else { 0 };
            let n = level
                .shares
                .min(wanted - quantity)
                .min(((budget - spent) / unit).min(u64::from(u32::MAX)) as u32);
            if n == 0 {
                break;
            }
            total += u64::from(n) * u64::from(level.price);
            fees += u64::from(n) * unit_fee;
            quantity += n;
            fills.push(Level {
                price: level.price,
                shares: n,
            });
            if quantity == wanted {
                break;
            }
        }
        // Every attempted eligible order uses a rate slot, even if liquidity is insufficient.
        self.attempts.push_back(now);
        if quantity < self.config.min_shares {
            return Err("insufficient_edge_liquidity_or_budget".into());
        }
        let market = self.markets.get_mut(token).unwrap();
        if side == Side::Buy {
            self.cash -= total + fees;
            market.cost += total + fees;
            market.position += quantity;
        } else {
            let basis = ((u128::from(market.cost) * u128::from(quantity))
                / u128::from(market.position)) as u64;
            market.cost -= basis;
            market.position -= quantity;
            self.cash = self.cash.checked_add(total - fees).ok_or("cash_overflow")?;
            self.realized_pnl += i128::from(total) - i128::from(fees) - i128::from(basis);
        }
        market.book.consume(side, &fills);
        Ok(Fill {
            token: token.into(),
            side,
            shares: quantity,
            notional_micro: total,
            fee_micro: fees,
            vwap_micro: (total / u64::from(quantity)) as u32,
            worst_price_micro: fills.last().unwrap().price,
            cash_micro: self.cash,
            time_ms: now,
            paper: true,
            chain_id: CHAIN_ID,
        })
    }
}
