//! Stateless research candidates with declared provenance. This module submits no orders.
use crate::{
    Result,
    base::{CHAIN_ID, PRICE_SCALE},
    book::Side,
    engine::Event,
    math::{BinaryQuote, binary_quote, fraction_budget, fractional_kelly},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SignalRequest {
    pub id: String,
    pub token: String,
    pub chain_id: u64,
    pub model_id: String,
    pub evidence_ref: String,
    pub probability_micro: u32,
    pub uncertainty_margin_micro: u32,
    pub ask_price_micro: u32,
    pub fee_bps: u32,
    pub capital_micro: u64,
    #[serde(default = "full_kelly")]
    pub kelly_multiplier_micro: u32,
    pub max_fraction_micro: u32,
    pub max_order_micro: u64,
    pub max_shares: u32,
    pub min_edge_micro: u32,
    pub quoted_ms: u64,
    pub forecast_ms: u64,
    pub generated_ms: u64,
    pub max_age_ms: u64,
}
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SignalAction {
    BuyCandidate,
    NoEdge,
    InsufficientBudget,
}
#[derive(Serialize)]
pub struct Signal {
    pub schema_version: u32,
    pub id: String,
    pub token: String,
    pub chain_id: u64,
    pub model_id: String,
    pub evidence_ref: String,
    pub generated_ms: u64,
    pub expires_ms: u64,
    pub action: SignalAction,
    pub conservative_probability_micro: u32,
    pub math: BinaryQuote,
    pub capped_fraction_micro: u32,
    pub budget_micro: u64,
    pub shares: u32,
    pub total_cost_micro: u64,
    pub suggested_event: Option<Event>,
    pub research_only: bool,
    pub declared_evidence_only: bool,
}

fn full_kelly() -> u32 {
    PRICE_SCALE
}

pub fn generate(r: &SignalRequest) -> Result<Signal> {
    if r.chain_id != CHAIN_ID {
        return Err("Base_only: signal_chain_mismatch".into());
    }
    if [&r.id, &r.token, &r.model_id, &r.evidence_ref]
        .iter()
        .any(|s| s.trim().is_empty() || s.len() > 512)
        || r.probability_micro > PRICE_SCALE
        || r.uncertainty_margin_micro > PRICE_SCALE
        || r.max_fraction_micro > PRICE_SCALE
        || r.max_shares == 0
        || r.max_order_micro == 0
        || r.min_edge_micro >= PRICE_SCALE
        || r.max_age_ms == 0
    {
        return Err("invalid_signal_request".into());
    }
    let expires_ms = r
        .quoted_ms
        .min(r.forecast_ms)
        .checked_add(r.max_age_ms)
        .ok_or("signal_expiry_overflow")?;
    if r.generated_ms < r.quoted_ms || r.generated_ms < r.forecast_ms || r.generated_ms > expires_ms
    {
        return Err("signal_clock_or_age".into());
    }
    let probability = r
        .probability_micro
        .saturating_sub(r.uncertainty_margin_micro);
    let math = binary_quote(probability, r.ask_price_micro, r.fee_bps)?;
    let fraction = fractional_kelly(
        math.full_kelly_fraction_micro,
        r.kelly_multiplier_micro,
        r.max_fraction_micro,
    )?;
    let budget = fraction_budget(r.capital_micro, fraction)?.min(r.max_order_micro);
    let shares = (budget / math.effective_cost_micro).min(u64::from(r.max_shares)) as u32;
    let action = if math.expected_edge_micro <= 0
        || math.expected_edge_micro < i64::from(r.min_edge_micro)
    {
        SignalAction::NoEdge
    } else if shares == 0 {
        SignalAction::InsufficientBudget
    } else {
        SignalAction::BuyCandidate
    };
    let shares = if action == SignalAction::BuyCandidate {
        shares
    } else {
        0
    };
    let suggested_event = if shares > 0 {
        Some(Event::Trade {
            token: r.token.clone(),
            side: Side::Buy,
            fair_micro: probability,
            shares,
            time_ms: r.generated_ms,
        })
    } else {
        None
    };
    Ok(Signal {
        schema_version: 1,
        id: r.id.clone(),
        token: r.token.clone(),
        chain_id: CHAIN_ID,
        model_id: r.model_id.clone(),
        evidence_ref: r.evidence_ref.clone(),
        generated_ms: r.generated_ms,
        expires_ms,
        action,
        conservative_probability_micro: probability,
        math: math.clone(),
        capped_fraction_micro: fraction,
        budget_micro: budget,
        shares,
        total_cost_micro: u64::from(shares) * math.effective_cost_micro,
        suggested_event,
        research_only: true,
        declared_evidence_only: true,
    })
}
