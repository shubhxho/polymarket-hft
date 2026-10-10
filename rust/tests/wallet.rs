use polymarket_hft::wallet::*;
use serde_json::json;
const OWNER: &str = "0x1111111111111111111111111111111111111111";
const TOKEN: &str = "0x2222222222222222222222222222222222222222";
#[test]
fn addresses_and_rpc_urls_reject_credentials_zero_and_wrong_scheme() {
    assert_eq!(
        address("0xABCDEF1234567890123456789012345678901234").unwrap(),
        "0xabcdef1234567890123456789012345678901234"
    );
    for value in [
        "",
        "0x0",
        "0x0000000000000000000000000000000000000000",
        "0xgggggggggggggggggggggggggggggggggggggggg",
        "1111111111111111111111111111111111111111",
    ] {
        assert!(address(value).is_err());
    }
    for url in [
        "http://localhost:8545",
        "file:///tmp/rpc",
        "https://key:secret@example.com",
        "https://example.com/#secret",
        "bad",
    ] {
        assert!(WalletClient::new(url).is_err());
    }
    assert!(WalletClient::new("https://example.com/rpc").is_ok());
}
#[test]
fn rpc_envelopes_require_version_exact_id_string_result_and_no_error() {
    assert_eq!(
        rpc_result(&json!({"jsonrpc":"2.0","id":3,"result":"0x0"}), 3).unwrap(),
        "0x0"
    );
    for value in [
        json!({"id":3,"result":"0x0"}),
        json!({"jsonrpc":"2.0","id":"3","result":"0x0"}),
        json!({"jsonrpc":"2.0","id":4,"result":"0x0"}),
        json!({"jsonrpc":"2.0","id":3,"result":0}),
        json!({"jsonrpc":"2.0","id":3,"result":"0x0","error":null}),
        json!([]),
    ] {
        assert!(rpc_result(&value, 3).is_err());
    }
}
#[test]
fn polygon_quantities_and_uint256_token_words_are_exact() {
    check_chain("0x89").unwrap();
    for chain in [
        "0x1",
        "0x2105",
        "0x089",
        "137",
        "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    ] {
        assert!(check_chain(chain).is_err());
    }
    assert_eq!(quantity("0xAB").unwrap(), "0xab");
    assert!(quantity(&format!("0x{}", "f".repeat(64))).is_ok());
    for value in ["0x", "0x00", "0xg", "0X1", "-1"] {
        assert!(quantity(value).is_err());
    }
    assert!(quantity(&format!("0x{}", "f".repeat(65))).is_err());
    for digit in ["0", "f"] {
        assert!(token_balance(&format!("0x{}", digit.repeat(64))).is_ok());
    }
    for value in ["0x0", "0x", "0xffff"] {
        assert!(token_balance(value).is_err());
    }
}
#[test]
fn balance_of_is_pinned_read_only_and_cannot_encode_arbitrary_calls() {
    let params = balance_call(OWNER, TOKEN, "0x123").unwrap();
    assert_eq!(params[0]["to"], TOKEN);
    assert_eq!(
        params[0]["data"],
        format!("0x70a08231{}{}", "0".repeat(24), &OWNER[2..])
    );
    assert_eq!(params[1], "0x123");
    assert!(balance_call("bad", TOKEN, "0x123").is_err());
    assert!(balance_call(OWNER, "bad", "0x123").is_err());
    assert!(balance_call(OWNER, TOKEN, "latest").is_err());
}

#[test]
fn inspection_checks_chain_twice_and_uses_one_block_for_both_balances() {
    let mut calls = Vec::new();
    let snapshot = inspect_wallet(OWNER, Some(TOKEN), |id, method, params| {
        calls.push((id, method.to_string(), params));
        Ok(match id {
            1 | 5 => "0x89".into(),
            2 => "0x123".into(),
            3 => "0x123456789abcdef".into(),
            4 => format!("0x{}", "f".repeat(64)),
            _ => panic!(),
        })
    })
    .unwrap();
    assert_eq!(
        calls.iter().map(|x| x.1.as_str()).collect::<Vec<_>>(),
        [
            "eth_chainId",
            "eth_blockNumber",
            "eth_getBalance",
            "eth_call",
            "eth_chainId"
        ]
    );
    assert_eq!(calls[2].2, json!([OWNER, "0x123"]));
    assert_eq!(calls[3].2[1], "0x123");
    assert_eq!(snapshot.chain_id, 137);
    assert_eq!(snapshot.native_balance_wei_hex, "0x123456789abcdef");
    assert!(snapshot.read_only);
    assert!(!snapshot.ownership_verified);
}
#[test]
fn inspection_stops_on_wrong_chain_bad_block_malformed_balance_or_transport_error() {
    for fail_at in [1, 2, 3, 4, 5] {
        let mut calls = 0;
        let result = inspect_wallet(OWNER, Some(TOKEN), |id, _, _| {
            calls += 1;
            if id == fail_at {
                return Ok(match id {
                    1 | 5 => "0x2105",
                    2 => "0x10000000000000000",
                    3 => "0x00",
                    4 => "0x1",
                    _ => panic!(),
                }
                .into());
            }
            Ok(match id {
                1 | 5 => "0x89".into(),
                2 => "0x123".into(),
                3 => "0x0".into(),
                4 => format!("0x{}", "0".repeat(64)),
                _ => panic!(),
            })
        });
        assert!(result.is_err());
        assert_eq!(calls, fail_at);
    }
    assert!(inspect_wallet(OWNER, None, |_, _, _| Err("offline".into())).is_err());
    let mut methods = Vec::new();
    let result = inspect_wallet(OWNER, None, |id, method, _| {
        methods.push(method.to_owned());
        Ok(match id {
            1 | 5 => "0x89",
            2 => "0x123",
            3 => "0x0",
            _ => panic!(),
        }
        .into())
    })
    .unwrap();
    assert!(result.token_balance_raw_hex.is_none());
    assert!(!methods.iter().any(|m| m == "eth_call"));
}
