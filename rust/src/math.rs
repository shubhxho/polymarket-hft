//! Checked fixed-point binary-contract research math.
use crate::{Result, base::PRICE_SCALE, engine::fee};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct BinaryQuote {
    pub probability_micro: u32,
    pub price_micro: u32,
    pub fee_micro: u64,
    pub effective_cost_micro: u64,
    pub expected_edge_micro: i64,
    pub full_kelly_fraction_micro: u32,
}

/// Assumes a $1 successful resolution, $0 unsuccessful resolution and upfront flat fees.
/// Integer Kelly is rounded down. It is a research quantity, not a trading recommendation.
pub fn binary_quote(probability_micro: u32, price_micro: u32, fee_bps: u32) -> Result<BinaryQuote> {
    if probability_micro > PRICE_SCALE
        || price_micro == 0
        || price_micro >= PRICE_SCALE
        || fee_bps > 10_000
    {
        return Err("invalid_binary_quote".into());
    }
    let fee_micro = fee(u64::from(price_micro), fee_bps);
    binary_quote_with_fee(probability_micro, price_micro, fee_micro)
}

/// Uses a supplied conservative per-share upfront fee in millionths.
pub fn binary_quote_with_fee(
    probability_micro: u32,
    price_micro: u32,
    fee_micro: u64,
) -> Result<BinaryQuote> {
    if probability_micro > PRICE_SCALE
        || price_micro == 0
        || price_micro >= PRICE_SCALE
        || fee_micro > u64::from(PRICE_SCALE)
    {
        return Err("invalid_binary_quote".into());
    }
    let cost = u64::from(price_micro) + fee_micro;
    let edge = i64::from(probability_micro) - cost as i64;
    let fraction = if edge > 0 && cost < u64::from(PRICE_SCALE) {
        (edge as u64 * u64::from(PRICE_SCALE) / (u64::from(PRICE_SCALE) - cost)) as u32
    } else {
        0
    };
    Ok(BinaryQuote {
        probability_micro,
        price_micro,
        fee_micro,
        effective_cost_micro: cost,
        expected_edge_micro: edge,
        full_kelly_fraction_micro: fraction,
    })
}

/// Floors a fractional cash budget; u128 prevents intermediate overflow.
pub fn fraction_budget(capital_micro: u64, fraction_micro: u32) -> Result<u64> {
    if fraction_micro > PRICE_SCALE {
        return Err("invalid_budget_fraction".into());
    }
    Ok((u128::from(capital_micro) * u128::from(fraction_micro) / u128::from(PRICE_SCALE)) as u64)
}

/// Fractional Kelly is applied before the portfolio/order cap, always rounding down.
pub fn fractional_kelly(full_micro: u32, multiplier_micro: u32, cap_micro: u32) -> Result<u32> {
    if [full_micro, multiplier_micro, cap_micro]
        .iter()
        .any(|&x| x > PRICE_SCALE)
    {
        return Err("invalid_kelly_fraction".into());
    }
    Ok(
        ((u64::from(full_micro) * u64::from(multiplier_micro) / u64::from(PRICE_SCALE)) as u32)
            .min(cap_micro),
    )
}
/// Conservative per-share bound for C * rate * price * (1-price).
/// Ceil to one millionth per share; actual venue match rounding can differ.
pub fn polymarket_fee_bound(price_micro: u32, rate_micro: u32, maker: bool) -> Result<u64> {
    if price_micro == 0 || price_micro >= PRICE_SCALE || rate_micro > PRICE_SCALE {
        return Err("invalid_venue_fee".into());
    }
    if maker {
        return Ok(0);
    }
    let numerator =
        u128::from(rate_micro) * u128::from(price_micro) * u128::from(PRICE_SCALE - price_micro);
    Ok(numerator.div_ceil(u128::from(PRICE_SCALE).pow(2)) as u64)
}
