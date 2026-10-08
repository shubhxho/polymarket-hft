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
