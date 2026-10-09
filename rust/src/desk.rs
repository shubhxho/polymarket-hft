//! Local research desk: supplied-data ranking and bounded HTTP framing.
use crate::{
    Result,
    signals::{Signal, SignalAction, SignalRequest, generate},
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, io::Read};
pub const BODY_LIMIT: usize = 65_536;
pub struct Request {
    pub method: String,
    pub path: String,
    pub body: Vec<u8>,
}
pub fn read_request(reader: &mut impl Read, port: u16) -> Result<Request> {
    let mut header = Vec::with_capacity(1024);
    loop {
        if header.len() == 8192 {
            return Err("header_too_large".into());
        }
        let mut byte = [0];
        reader.read_exact(&mut byte).map_err(|e| e.to_string())?;
        header.push(byte[0]);
        if header.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let text = std::str::from_utf8(&header).map_err(|_| "header_not_utf8")?;
    let mut lines = text.split("\r\n");
    let parts: Vec<_> = lines.next().ok_or("missing_request")?.split(' ').collect();
    if parts.len() != 3 || parts[2] != "HTTP/1.1" || !["GET", "POST"].contains(&parts[0]) {
        return Err("unsupported_request".into());
    }
    let allowed = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    let mut seen = BTreeSet::new();
    let mut length = None;
    let mut host = false;
    let mut json_type = false;
    let mut desk = false;
    for line in lines.filter(|s| !s.is_empty()) {
        let (name, value) = line.split_once(':').ok_or("invalid_header")?;
        let name = name.to_ascii_lowercase();
        let value = value.trim();
        if !seen.insert(name.clone()) {
            return Err("duplicate_header".into());
        }
        match name.as_str() {
            "host" => {
                if !allowed.iter().any(|s| s == value) {
                    return Err("host_rejected".into());
                }
                host = true;
            }
            "origin" => {
                if !allowed.iter().any(|s| format!("http://{s}") == value) {
                    return Err("origin_rejected".into());
                }
            }
            "transfer-encoding" => return Err("chunked_requests_unsupported".into()),
            "content-length" => {
                length = Some(value.parse::<usize>().map_err(|_| "invalid_length")?)
            }
            "content-type" => json_type = value == "application/json",
            "x-hft-desk" => desk = value == "1",
            _ => (),
        }
    }
    if !host {
        return Err("host_required".into());
    }
    let length = length.unwrap_or(0);
    if length > BODY_LIMIT {
        return Err("body_too_large".into());
    }
    if parts[0] == "POST" && (!json_type || !desk || length == 0) {
        return Err("desk_json_required".into());
    }
    if parts[0] == "GET" && length != 0 {
        return Err("get_body_rejected".into());
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).map_err(|e| e.to_string())?;
    Ok(Request {
        method: parts[0].into(),
        path: parts[1].into(),
        body,
    })
}
/// Rank only the supplied candidates. Probability/evidence claims are not independently verified.
pub fn rank(mut requests: Vec<SignalRequest>, now: u64) -> Result<Value> {
    if requests.is_empty() || requests.len() > 64 {
        return Err("supply_1_to_64_candidates".into());
    }
    let mut signals: Vec<Signal> = Vec::new();
    let mut rejected = Vec::new();
    let mut tokens = BTreeSet::new();
    for r in &mut requests {
        // Wall-clock validation cannot be bypassed with a caller-chosen generated timestamp.
        r.generated_ms = now;
        let result = if !tokens.insert(r.token.clone()) {
            Err("duplicate_token".into())
        } else if r.fee_rate_micro.is_none() {
            Err("actual_market_fee_rate_required".into())
        } else {
            generate(r)
        };
        match result {
            Ok(signal) => signals.push(signal),
            Err(reason) => rejected.push(json!({"id":r.id,"reason":reason})),
        }
    }
    signals.sort_by(|a, b| {
        b.math
            .expected_edge_micro
            .cmp(&a.math.expected_edge_micro)
            .then_with(|| a.token.cmp(&b.token))
    });
    let best = signals
        .iter()
        .find(|s| s.action == SignalAction::BuyCandidate)
        .map(|s| s.id.clone());
    Ok(
        json!({"signals":signals,"rejected":rejected,"best_candidate_id":best,"ranked_at_ms":now,"scope":"Supplied candidates ranked by conservative fee-inclusive edge per share; no global best or profitability claim","submission_enabled":false,"declared_evidence_only":true}),
    )
}
