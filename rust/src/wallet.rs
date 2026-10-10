//! Read-only Polygon wallet inspection. No keys, signatures, approvals or orders.
use crate::{Result, venue::POLYGON};
use serde::Serialize;
use serde_json::{Value, json};
use std::{io::Read, time::Duration};

pub fn address(value: &str) -> Result<String> {
    if value.len() != 42
        || !value.starts_with("0x")
        || !value[2..].bytes().all(|b| b.is_ascii_hexdigit())
        || value[2..].bytes().all(|b| b == b'0')
    {
        return Err("invalid_wallet_address".into());
    }
    // Normalization is not an EIP-55 checksum or proof of wallet ownership.
    Ok(value.to_ascii_lowercase())
}

pub fn rpc_result(response: &Value, id: u64) -> Result<&str> {
    if response.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || response.get("id").and_then(Value::as_u64) != Some(id)
        || response.get("error").is_some()
    {
        return Err("invalid_wallet_rpc_response".into());
    }
    response
        .get("result")
        .and_then(Value::as_str)
        .ok_or("invalid_wallet_rpc_result".into())
}

pub fn quantity(value: &str) -> Result<String> {
    let digits = value.strip_prefix("0x").ok_or("invalid_wallet_quantity")?;
    if digits.is_empty()
        || digits.len() > 64
        || !digits.bytes().all(|b| b.is_ascii_hexdigit())
        || (digits.len() > 1 && digits.starts_with('0'))
    {
        return Err("invalid_wallet_quantity".into());
    }
    Ok(value.to_ascii_lowercase())
}

pub fn token_balance(value: &str) -> Result<String> {
    if value.len() != 66
        || !value.starts_with("0x")
        || !value[2..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("invalid_wallet_token_balance".into());
    }
    Ok(value.to_ascii_lowercase())
}

pub fn check_chain(value: &str) -> Result<()> {
    let chain = quantity(value)?;
    if u64::from_str_radix(&chain[2..], 16).ok() != Some(POLYGON) {
        return Err("wallet_wrong_chain".into());
    }
    Ok(())
}

pub fn balance_call(owner: &str, token: &str, block: &str) -> Result<Value> {
    let owner = address(owner)?;
    let token = address(token)?;
    let block = quantity(block)?;
    Ok(json!([{"to": token, "data": format!("0x70a08231{:0>64}", &owner[2..])}, block]))
}

#[derive(Debug, Serialize)]
pub struct WalletSnapshot {
    pub chain_id: u64,
    pub address: String,
    pub block_number: String,
    pub native_balance_wei_hex: String,
    pub token_contract: Option<String>,
    pub token_balance_raw_hex: Option<String>,
    pub read_only: bool,
    pub ownership_verified: bool,
}

pub struct WalletClient {
    endpoint: reqwest::Url,
    client: reqwest::blocking::Client,
}
impl WalletClient {
    pub fn new(endpoint: &str) -> Result<Self> {
        let endpoint = reqwest::Url::parse(endpoint).map_err(|_| "invalid_wallet_rpc_url")?;
        if endpoint.scheme() != "https"
            || endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.fragment().is_some()
        {
            return Err("wallet_rpc_requires_https".into());
        }
        Ok(Self {
            endpoint,
            client: reqwest::blocking::Client::builder()
                .connect_timeout(Duration::from_secs(2))
                .timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| "wallet_client_failed")?,
        })
    }
    fn call(&self, id: u64, method: &str, params: Value) -> Result<String> {
        let response = self
            .client
            .post(self.endpoint.clone())
            .json(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .send()
            .map_err(|_| "wallet_rpc_network_error")?;
        if !response.status().is_success() {
            return Err("wallet_rpc_http_error".into());
        }
        let mut bytes = Vec::new();
        response
            .take(65_537)
            .read_to_end(&mut bytes)
            .map_err(|_| "wallet_rpc_read_error")?;
        if bytes.len() > 65_536 {
            return Err("wallet_rpc_too_large".into());
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| "wallet_rpc_invalid_json")?;
        Ok(rpc_result(&value, id)?.to_owned())
    }
    pub fn snapshot(&self, owner: &str, token: Option<&str>) -> Result<WalletSnapshot> {
        inspect_wallet(owner, token, |id, method, params| {
            self.call(id, method, params)
        })
    }
}

/// Transport-injected inspection keeps the complete RPC sequence testable offline.
pub fn inspect_wallet(
    owner: &str,
    token: Option<&str>,
    mut rpc: impl FnMut(u64, &str, Value) -> Result<String>,
) -> Result<WalletSnapshot> {
    let owner = address(owner)?;
    let token = token.map(address).transpose()?;
    check_chain(&rpc(1, "eth_chainId", json!([]))?)?;
    let block = quantity(&rpc(2, "eth_blockNumber", json!([]))?)?;
    u64::from_str_radix(&block[2..], 16).map_err(|_| "wallet_block_overflow")?;
    let native = quantity(&rpc(3, "eth_getBalance", json!([owner, block]))?)?;
    let balance = token
        .as_deref()
        .map(|contract| {
            token_balance(&rpc(
                4,
                "eth_call",
                balance_call(&owner, contract, &block)?,
            )?)
        })
        .transpose()?;
    check_chain(&rpc(5, "eth_chainId", json!([]))?)?;
    Ok(WalletSnapshot {
        chain_id: POLYGON,
        address: owner,
        block_number: block,
        native_balance_wei_hex: native,
        token_contract: token,
        token_balance_raw_hex: balance,
        read_only: true,
        ownership_verified: false,
    })
}
