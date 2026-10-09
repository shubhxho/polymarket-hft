//! Polygon order planning shares the paper engine’s chain constants.
//! No wallet signing, approval, bridging or automatic submission occurs here.
use crate::{
    Result,
    math::{
        BinaryQuote, binary_quote_with_fee, fraction_budget, fractional_kelly, polymarket_fee_bound,
    },
    polygon::PRICE_SCALE,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, io::Read, time::Duration};
pub const POLYGON: u64 = crate::polygon::CHAIN_ID;
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    Ctf,
    CtfNegRisk,
    V2,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MarketContext {
    pub asset_id: String,
    pub protocol: Protocol,
    pub chain_id: u64,
    pub tick_micro: u32,
    pub min_shares: u32,
    pub ask_micro: u32,
    pub available_shares: u32,
    pub taker_fee_rate_micro: u32,
    pub observed_ms: u64,
    pub accepting_orders: bool,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub polygon_enabled: bool,
    pub allowed_assets: BTreeSet<String>,
    pub capital_micro: u64,
    pub max_order_micro: u64,
    pub max_fraction_micro: u32,
    pub kelly_multiplier_micro: u32,
    pub max_shares: u32,
    pub min_edge_micro: u32,
    pub max_age_ms: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Forecast {
    pub probability_micro: u32,
    pub uncertainty_micro: u32,
    pub model_id: String,
    pub evidence_ref: String,
    pub forecast_ms: u64,
}
#[derive(Serialize)]
pub struct OrderPlan {
    pub chain_id: u64,
    pub asset_id: String,
    pub protocol: Protocol,
    pub domain_name: &'static str,
    pub domain_version: &'static str,
    pub verifying_contract: &'static str,
    pub side: &'static str,
    pub order_type: &'static str,
    pub maker_amount: String,
    pub taker_amount: String,
    pub price_micro: u32,
    pub shares: u32,
    pub math: BinaryQuote,
    pub reserved_cost_micro: u64,
    pub expires_ms: u64,
    pub model_id: String,
    pub evidence_ref: String,
    pub submission_enabled: bool,
    pub metadata_authenticated: bool,
}
pub fn plan(m: &MarketContext, p: &Policy, f: &Forecast, now_ms: u64) -> Result<OrderPlan> {
    if !p.polygon_enabled || m.chain_id != POLYGON {
        return Err("polygon_not_enabled_or_wrong_chain".into());
    }
    if !decimal_asset(&m.asset_id) || !p.allowed_assets.contains(&m.asset_id) || !m.accepting_orders
    {
        return Err("asset_not_allowed_or_market_closed".into());
    }
    if ![100000, 10000, 5000, 2500, 1000, 100].contains(&m.tick_micro)
        || m.ask_micro == 0
        || m.ask_micro >= PRICE_SCALE
        || !m.ask_micro.is_multiple_of(m.tick_micro)
        || m.min_shares == 0
        || p.max_shares == 0
        || p.capital_micro == 0
        || p.max_order_micro == 0
        || p.max_age_ms == 0
        || p.min_edge_micro >= PRICE_SCALE
        || f.probability_micro > PRICE_SCALE
        || f.uncertainty_micro > PRICE_SCALE
        || [&f.model_id, &f.evidence_ref]
            .iter()
            .any(|s| s.trim().is_empty() || s.len() > 512)
    {
        return Err("invalid_order_plan_inputs".into());
    }
    let expiry = m
        .observed_ms
        .min(f.forecast_ms)
        .checked_add(p.max_age_ms)
        .ok_or("order_expiry_overflow")?;
    if now_ms < m.observed_ms || now_ms < f.forecast_ms || now_ms > expiry {
        return Err("stale_or_future_market_forecast".into());
    }
    let fee = polymarket_fee_bound(m.ask_micro, m.taker_fee_rate_micro, false)?;
    let quote = binary_quote_with_fee(
        f.probability_micro.saturating_sub(f.uncertainty_micro),
        m.ask_micro,
        fee,
    )?;
    if quote.expected_edge_micro <= 0 || quote.expected_edge_micro < i64::from(p.min_edge_micro) {
        return Err("no_conservative_edge".into());
    }
    let fraction = fractional_kelly(
        quote.full_kelly_fraction_micro,
        p.kelly_multiplier_micro,
        p.max_fraction_micro,
    )?;
    let budget = fraction_budget(p.capital_micro, fraction)?.min(p.max_order_micro);
    let shares = (budget / quote.effective_cost_micro)
        .min(u64::from(p.max_shares))
        .min(u64::from(m.available_shares)) as u32;
    if shares < m.min_shares {
        return Err("below_minimum_order_size".into());
    }
    let (version, exchange) = match m.protocol {
        Protocol::V2 => ("3", "0xe3333700cA9d93003F00f0F71f8515005F6c00Aa"),
        Protocol::Ctf => ("2", "0xE111180000d2663C0091e4f400237545B87B996B"),
        Protocol::CtfNegRisk => ("2", "0xe2222d279d744050d28e00520010520000310F59"),
    };
    let cost = u64::from(shares) * quote.effective_cost_micro;
    Ok(OrderPlan {
        chain_id: POLYGON,
        asset_id: m.asset_id.clone(),
        protocol: m.protocol,
        domain_name: "Polymarket CTF Exchange",
        domain_version: version,
        verifying_contract: exchange,
        side: "BUY",
        order_type: "FOK",
        maker_amount: (u64::from(shares) * u64::from(m.ask_micro)).to_string(),
        taker_amount: (u64::from(shares) * u64::from(PRICE_SCALE)).to_string(),
        price_micro: m.ask_micro,
        shares,
        math: quote,
        reserved_cost_micro: cost,
        expires_ms: expiry,
        model_id: f.model_id.clone(),
        evidence_ref: f.evidence_ref.clone(),
        submission_enabled: false,
        metadata_authenticated: false,
    })
}
pub fn decimal_asset(s: &str) -> bool {
    if s.is_empty()
        || s.len() > 78
        || !s.bytes().all(|b| b.is_ascii_digit())
        || s == "0"
        || (s.len() > 1 && s.starts_with('0'))
    {
        return false;
    }
    const MAX: &str =
        "115792089237316195423570985008687907853269984665640564039457584007913129639935";
    s.len() < MAX.len() || s <= MAX
}
/// A reusable pooled read-only HTTP client. No API credentials or signing keys are read.
pub struct MarketClient {
    client: reqwest::blocking::Client,
}
impl MarketClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::blocking::Client::builder()
                .connect_timeout(Duration::from_secs(2))
                .timeout(Duration::from_secs(5))
                .pool_idle_timeout(Duration::from_secs(60))
                .tcp_nodelay(true)
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| e.to_string())?,
        })
    }
    pub fn book(&self, asset: &str) -> Result<serde_json::Value> {
        if !decimal_asset(asset) {
            return Err("invalid_asset_id".into());
        }
        let response = self
            .client
            .get("https://clob.polymarket.com/book")
            .query(&[("token_id", asset)])
            .send()
            .map_err(|_| "market_data_network_error")?;
        if !response.status().is_success() {
            return Err(format!("market_data_http_{}", response.status().as_u16()));
        }
        let mut bytes = Vec::new();
        response
            .take(1_048_577)
            .read_to_end(&mut bytes)
            .map_err(|_| "market_data_read_error")?;
        if bytes.len() > 1_048_576 {
            return Err("market_data_too_large".into());
        }
        serde_json::from_slice(&bytes).map_err(|_| "market_data_invalid_json".into())
    }
}

/// Parse nonnegative decimal strings without floating point, to exact six-decimal units.
pub fn decimal_micro(s: &str) -> Result<u64> {
    if s.is_empty() || s.len() > 32 {
        return Err("invalid_decimal".into());
    }
    let mut parts = s.split('.');
    let whole = parts.next().unwrap();
    let frac = parts.next().unwrap_or("");
    if parts.next().is_some()
        || whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || frac.len() > 6
        || !frac.bytes().all(|b| b.is_ascii_digit())
        || (s.ends_with('.') && frac.is_empty())
    {
        return Err("invalid_decimal".into());
    }
    let whole: u64 = whole.parse().map_err(|_| "decimal_overflow")?;
    let fraction = if frac.is_empty() {
        0
    } else {
        frac.parse::<u64>().map_err(|_| "invalid_decimal")? * 10u64.pow(6 - frac.len() as u32)
    };
    whole
        .checked_mul(u64::from(PRICE_SCALE))
        .and_then(|n| n.checked_add(fraction))
        .ok_or("decimal_overflow".into())
}
/// Normalizes declared book metadata; acceptance and fee rate require separate market data.
pub fn normalize_book(
    book: &serde_json::Value,
    fee_rate_micro: u32,
    accepting_orders: bool,
) -> Result<MarketContext> {
    let text = |key| {
        book.get(key)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("missing_book_{key}"))
    };
    let asset = text("asset_id")?;
    if !decimal_asset(asset) {
        return Err("invalid_book_asset".into());
    }
    let protocol = match book.get("version") {
        Some(serde_json::Value::String(v)) if v == "v2" => Protocol::V2,
        None => match book.get("neg_risk").and_then(serde_json::Value::as_bool) {
            Some(false) => Protocol::Ctf,
            Some(true) => Protocol::CtfNegRisk,
            None => return Err("missing_book_neg_risk".into()),
        },
        _ => return Err("unsupported_book_protocol".into()),
    };
    let tick =
        u32::try_from(decimal_micro(text("tick_size")?)?).map_err(|_| "invalid_book_tick")?;
    if ![100000, 10000, 5000, 2500, 1000, 100].contains(&tick) || fee_rate_micro > PRICE_SCALE {
        return Err("invalid_book_tick_or_fee".into());
    }
    let minimum = decimal_micro(text("min_order_size")?)?.div_ceil(u64::from(PRICE_SCALE));
    let observed_ms = text("timestamp")?
        .parse::<u64>()
        .map_err(|_| "invalid_book_timestamp")?;
    let asks = book
        .get("asks")
        .and_then(serde_json::Value::as_array)
        .ok_or("missing_book_asks")?;
    if asks.len() > 10000 {
        return Err("too_many_book_levels".into());
    }
    let mut best = u32::MAX;
    let mut shares = 0u64;
    for level in asks {
        let price = u32::try_from(decimal_micro(
            level
                .get("price")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid_book_price")?,
        )?)
        .map_err(|_| "invalid_book_price")?;
        let quantity = decimal_micro(
            level
                .get("size")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid_book_size")?,
        )? / u64::from(PRICE_SCALE);
        if price == 0 || price >= PRICE_SCALE || !price.is_multiple_of(tick) {
            return Err("invalid_book_price_tick".into());
        }
        if quantity == 0 {
            continue;
        }
        if price < best {
            best = price;
            shares = quantity;
        } else if price == best {
            shares = shares.checked_add(quantity).ok_or("book_size_overflow")?;
        }
    }
    if best == u32::MAX || minimum == 0 {
        return Err("book_no_whole_share_ask_or_minimum".into());
    }
    Ok(MarketContext {
        asset_id: asset.into(),
        protocol,
        chain_id: POLYGON,
        tick_micro: tick,
        min_shares: u32::try_from(minimum).map_err(|_| "book_minimum_overflow")?,
        ask_micro: best,
        available_shares: shares.min(u64::from(u32::MAX)) as u32,
        taker_fee_rate_micro: fee_rate_micro,
        observed_ms,
        accepting_orders,
    })
}
