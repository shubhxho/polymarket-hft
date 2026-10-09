use polymarket_hft::{
    book::Side,
    buffer::{JsonLines, Lines},
    encoding::{FRAME_BYTES, LevelFrame},
    hardware::{Backend, select},
    math::{binary_quote_with_fee, fractional_kelly, polymarket_fee_bound},
    quant::Quantized,
    venue::{self, Forecast, MarketContext, Policy, Protocol},
};
use std::io::{BufReader, Cursor};
#[test]
fn bounded_lines_handle_small_chunks_crlf_blank_and_eof() {
    let mut lines = Lines::new(BufReader::with_capacity(2, Cursor::new(b"a\r\n\nbb")), 4).unwrap();
    assert_eq!(lines.next_line().unwrap().unwrap(), (1, &b"a\r\n"[..]));
    assert_eq!(lines.next_line().unwrap().unwrap(), (2, &b"\n"[..]));
    assert_eq!(lines.next_line().unwrap().unwrap(), (3, &b"bb"[..]));
    assert!(lines.next_line().unwrap().is_none());
    let mut exact = Lines::new(Cursor::new(b"1234"), 4).unwrap();
    assert_eq!(exact.next_line().unwrap().unwrap().1, b"1234");
    let mut too_big = Lines::new(Cursor::new(b"1234\n"), 4).unwrap();
    assert!(too_big.next_line().is_err());
    assert!(Lines::new(Cursor::new(b""), 0).is_err());
}
#[test]
fn output_errors_are_returned_and_batch_finish_flushes() {
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("broken"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert!(
        JsonLines::new(Broken, true)
            .record(&serde_json::json!({"a":1}))
            .is_err()
    );
    let mut bytes = Vec::new();
    {
        let mut output = JsonLines::new(&mut bytes, false);
        output.record(&serde_json::json!({"a":1})).unwrap();
        output.finish().unwrap();
    }
    assert_eq!(bytes, b"{\"a\":1}\n");
}
#[test]
fn fixed_encoding_roundtrips_and_rejects_noncanonical_frames() {
    let frame = LevelFrame {
        token_key: 7,
        side: Side::Sell,
        price_micro: 999999,
        shares: 0,
        sequence: u64::MAX,
        time_ms: 42,
    };
    let b = frame.encode().unwrap();
    assert_eq!(b.len(), FRAME_BYTES);
    assert_eq!(LevelFrame::decode(&b).unwrap(), frame);
    for i in [0, 4, 5, 6, 7, 20, 21, 22, 23] {
        let mut corrupt = b;
        corrupt[i] = 255;
        assert!(LevelFrame::decode(&corrupt).is_err());
    }
    assert!(LevelFrame::decode(&b[..39]).is_err());
    let mut bad = frame;
    bad.price_micro = 1000000;
    assert!(bad.encode().is_err());
}
#[test]
fn quantized_weights_corruption_shape_padding_and_numeric_checks() {
    for bits in [2, 4, 8] {
        for group in [1, 3, 16, 64] {
            let weights: Vec<_> = (0..192).map(|i| ((i % 17) as f32 - 8.) / 7.).collect();
            let q = Quantized::new(&weights, bits, group).unwrap();
            let b = q.encode();
            let decoded = Quantized::decode(&b).unwrap();
            assert_eq!(decoded.encode(), b);
            assert_eq!(decoded.dot(&weights).unwrap(), q.dot(&weights).unwrap());
            for i in [0, 4, 5, 8, 12, 16, 20, 24, b.len() - 1] {
                let mut corrupt = b.clone();
                corrupt[i] ^= 1;
                assert!(Quantized::decode(&corrupt).is_err());
            }
            assert!(Quantized::decode(&b[..b.len() - 1]).is_err());
        }
    }
    let q = Quantized::new(&[1.; 3], 2, 1).unwrap();
    assert_eq!(
        Quantized::decode(&q.encode())
            .unwrap()
            .dot(&[2.; 3])
            .unwrap(),
        6.
    );
    assert!(Quantized::new(&[0., f32::from_bits(1)], 2, 2).is_err());
    assert!(Quantized::new(&[f32::MAX, -f32::MAX], 4, 2).is_err());
}
#[test]
fn grouped_kernel_matches_decoded_dense_reference() {
    for bits in [2, 4, 8] {
        let w: Vec<_> = (0..1024).map(|i| (i as f32 * 0.37).sin()).collect();
        let x: Vec<_> = (0..1024).map(|i| (i as f32 * 0.13).cos()).collect();
        let q = Quantized::new(&w, bits, 64).unwrap();
        let dense: f32 = q.dequantize().iter().zip(&x).map(|(a, b)| a * b).sum();
        assert_eq!(q.dot(&x).unwrap(), dense);
    }
}
#[test]
fn fee_and_fractional_kelly_round_down_and_bound() {
    assert_eq!(polymarket_fee_bound(500000, 70000, false).unwrap(), 17500);
    assert_eq!(polymarket_fee_bound(500000, 70000, true).unwrap(), 0);
    assert_eq!(polymarket_fee_bound(1, 1, false).unwrap(), 1);
    assert_eq!(fractional_kelly(271535, 500000, 1000000).unwrap(), 135767);
    assert_eq!(fractional_kelly(1000000, 1000000, 50000).unwrap(), 50000);
    assert!(fractional_kelly(0, 1000001, 0).is_err());
    let q = binary_quote_with_fee(600000, 450000, 17325).unwrap();
    assert_eq!(q.effective_cost_micro, 467325);
    assert_eq!(q.expected_edge_micro, 132675);
    assert!(q.full_kelly_fraction_micro < 271535);
    for price in [1, 100, 10000, 450000, 500000, 999999] {
        assert_eq!(
            polymarket_fee_bound(price, 70000, false).unwrap(),
            polymarket_fee_bound(1000000 - price, 70000, false).unwrap()
        );
    }
}
fn inputs() -> (MarketContext, Policy, Forecast) {
    let value: serde_json::Value =
        serde_json::from_str(include_str!("../../fixtures/polygon-plan.json")).unwrap();
    (
        serde_json::from_value(value["market"].clone()).unwrap(),
        serde_json::from_value(value["policy"].clone()).unwrap(),
        serde_json::from_value(value["forecast"].clone()).unwrap(),
    )
}
#[test]
fn polygon_planning_domains_amounts_sizing_and_fail_closed() {
    let (mut m, mut p, mut f) = inputs();
    let out = venue::plan(&m, &p, &f, 1100).unwrap();
    assert_eq!(out.maker_amount, "9000000");
    assert_eq!(out.taker_amount, "20000000");
    assert_eq!(out.reserved_cost_micro, 9346500);
    assert!(!out.submission_enabled);
    assert_eq!(out.domain_version, "3");
    assert_eq!(out.order_type, "FOK");
    m.protocol = Protocol::Ctf;
    assert_eq!(venue::plan(&m, &p, &f, 1100).unwrap().domain_version, "2");
    m.protocol = Protocol::CtfNegRisk;
    assert!(
        venue::plan(&m, &p, &f, 1100)
            .unwrap()
            .verifying_contract
            .starts_with("0xe222")
    );
    p.polygon_enabled = false;
    assert!(venue::plan(&m, &p, &f, 1100).is_err());
    p.polygon_enabled = true;
    m.chain_id = 8453;
    assert!(venue::plan(&m, &p, &f, 1100).is_err());
    m.chain_id = 137;
    m.ask_micro += 1;
    assert!(venue::plan(&m, &p, &f, 1100).is_err());
    m.ask_micro -= 1;
    assert!(venue::plan(&m, &p, &f, 2901).is_err());
    assert!(venue::plan(&m, &p, &f, 899).is_err());
    p.allowed_assets.clear();
    assert!(venue::plan(&m, &p, &f, 1100).is_err());
    p.allowed_assets.insert("123".into());
    m.available_shares = 4;
    assert!(venue::plan(&m, &p, &f, 1100).is_err());
    m.available_shares = 100;
    f.uncertainty_micro = 650000;
    assert!(venue::plan(&m, &p, &f, 1100).is_err());
}
#[test]
fn asset_id_uint256_and_backend_readiness() {
    assert!(venue::decimal_asset(
        "115792089237316195423570985008687907853269984665640564039457584007913129639935"
    ));
    assert!(!venue::decimal_asset(
        "115792089237316195423570985008687907853269984665640564039457584007913129639936"
    ));
    for s in ["", "0", "0123", "-1", "abc"] {
        assert!(!venue::decimal_asset(s));
    }
    assert_eq!(select(true, false, false, false), Backend::Cpu);
    assert_eq!(select(true, false, true, false), Backend::MlxCpu);
    assert_eq!(select(true, false, true, true), Backend::MlxMetal);
    assert_eq!(select(true, true, true, true), Backend::Cuda);
    assert_eq!(select(false, false, false, true), Backend::Cpu);
}

#[test]
fn loaded_cpu_model_batches_without_reprobes() {
    let q = Quantized::new(&[1.; 64], 2, 64).unwrap();
    let model = polymarket_hft::hardware::LoadedModel::new(q, "cpu").unwrap();
    assert_eq!(model.backend(), Backend::Cpu);
    assert_eq!(model.infer_batch(&[2.; 128]).unwrap(), vec![128., 128.]);
    assert!(model.infer_batch(&[2.; 63]).is_err());
    assert!(model.infer_batch(&[f32::NAN; 64]).is_err());
}
#[cfg(any(feature = "mlx", feature = "mlx-cpu"))]
#[test]
fn loaded_mlx_cpu_model_reuses_weights_and_matches_packed_reference() {
    let weights: Vec<_> = (0..128).map(|i| ((i % 17) as f32 - 8.) / 7.).collect();
    for bits in [2, 4, 8] {
        let q = Quantized::new(&weights, bits, 64).unwrap();
        let expected = q.dot(&vec![0.5; 128]).unwrap();
        let model = polymarket_hft::hardware::LoadedModel::new(q, "mlx-cpu").unwrap();
        for _ in 0..4 {
            for value in model.infer_batch(&vec![0.5; 256]).unwrap() {
                assert!(
                    (value - expected).abs() < 1e-4,
                    "bits={bits}: {value} != {expected}"
                );
            }
        }
    }
}
#[test]
fn venue_decimal_is_exact_and_refuses_rounding_or_overflow() {
    assert_eq!(venue::decimal_micro("0.0025").unwrap(), 2500);
    assert_eq!(venue::decimal_micro("5.01").unwrap(), 5010000);
    assert_eq!(
        venue::decimal_micro("18446744073709.551615").unwrap(),
        u64::MAX
    );
    for s in [
        "",
        "1.",
        ".5",
        "-1",
        "+1",
        "1e-2",
        "0.0000001",
        "18446744073709.551616",
    ] {
        assert!(venue::decimal_micro(s).is_err());
    }
}
#[test]
fn venue_book_normalization_uses_best_tick_and_conservative_whole_depth() {
    let b = serde_json::json!({"asset_id":"123","timestamp":"1000","tick_size":"0.005","min_order_size":"5.01","neg_risk":false,"asks":[{"price":"0.55","size":"30"},{"price":"0.45","size":"8.9"},{"price":"0.45","size":"1.8"}]});
    let m = venue::normalize_book(&b, 70000, false).unwrap();
    assert_eq!(m.ask_micro, 450000);
    assert_eq!(m.available_shares, 9);
    assert_eq!(m.min_shares, 6);
    assert!(!m.accepting_orders);
    assert_eq!(m.protocol, Protocol::Ctf);
    let mut unknown = b.clone();
    unknown["version"] = serde_json::json!("v3");
    assert!(venue::normalize_book(&unknown, 70000, true).is_err());
    unknown = b.clone();
    unknown["asks"][0]["price"] = serde_json::json!("0.551");
    assert!(venue::normalize_book(&unknown, 70000, true).is_err());
}

#[cfg(any(feature = "mlx", feature = "mlx-cpu"))]
#[test]
fn direct_mlx_rejects_bad_batches_and_nonfinite_readbacks() {
    use polymarket_hft::mlx::MlxLinear;
    assert!(MlxLinear::new_on_device(&[], 2, false).is_err());
    assert!(MlxLinear::new_on_device(&[f32::NAN; 64], 2, false).is_err());
    let model = MlxLinear::new_on_device(&[1.; 64], 4, false).unwrap();
    for data in [
        vec![],
        vec![0.; 63],
        vec![f32::INFINITY; 64],
        vec![0.; 64 * 257],
    ] {
        assert!(model.dot_batch(&data).is_err());
    }
    assert!(model.dot(&[f32::MAX; 64]).is_err());
    assert!(model.dot_batch(&[f32::MAX; 64]).is_err());
    assert_eq!(
        model.dot_batch(&vec![1.; 64 * 256]).unwrap(),
        vec![64.; 256]
    );
}
