//! Explicit GPU gate: skipped unless requested on a CUDA machine.
use polymarket_hft::{
    hardware::{Backend, LoadedModel},
    quant::Quantized,
};

#[test]
#[ignore = "requires HFT_CUDA_HELPER, nvcc-built helper and an NVIDIA GPU"]
fn cuda_matches_packed_cpu_across_bits_groups_widths_and_batches() {
    assert!(
        std::env::var_os("HFT_CUDA_HELPER").is_some(),
        "set HFT_CUDA_HELPER"
    );
    for bits in [2, 4, 8] {
        for (width, group) in [
            (1, 1),
            (7, 1),
            (64, 64),
            (128, 64),
            (257, 1),
            (1024, 64),
            (8192, 64),
        ] {
            let weights: Vec<f32> = (0..width)
                .map(|i| ((i * 17 % 101) as f32 - 50.0) / 50.0)
                .collect();
            let cpu =
                LoadedModel::new(Quantized::new(&weights, bits, group).unwrap(), "cpu").unwrap();
            let cuda = LoadedModel::new(Quantized::new(&weights, bits, group).unwrap(), "cuda")
                .expect("CUDA helper must probe successfully");
            assert_eq!(cuda.backend(), Backend::Cuda);
            for rows in [1, 3, 32, 256] {
                let input: Vec<f32> = (0..width * rows)
                    .map(|i| ((i * 13 % 53) as f32 - 26.0) / 26.0)
                    .collect();
                let expected = cpu.infer_batch(&input).unwrap();
                let actual = cuda.infer_batch(&input).unwrap();
                assert_eq!(actual.len(), rows);
                for (a, b) in actual.into_iter().zip(expected) {
                    assert!(
                        (a - b).abs() <= 1e-4 + 1e-4 * b.abs(),
                        "bits={bits} width={width} group={group} rows={rows}: {a} != {b}"
                    );
                }
            }
            assert!(cuda.infer_batch(&[f32::NAN]).is_err());
            assert!(cuda.infer_batch(&vec![0.0; width * 257]).is_err());
        }
    }
}

// Exercise the Rust subprocess protocol on CPU-only hosts without claiming GPU execution.
#[cfg(unix)]
#[test]
fn cuda_bridge_checks_helper_status_output_shape_and_finite_values() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    let dir = std::env::temp_dir().join(format!("hft-cuda-bridge-test-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(dir.clone());
    let weights = dir.join("weights.hqt");
    let input = dir.join("input.json");
    let helper = dir.join("helper.sh");
    fs::write(
        &weights,
        Quantized::new(&[0.5; 64], 2, 64).unwrap().encode(),
    )
    .unwrap();
    fs::write(&input, serde_json::to_vec(&vec![1.0; 64]).unwrap()).unwrap();
    for (body, expected_error) in [
        ("printf '\\000\\000\\000\\102' > \"$2\"", None),
        ("printf 'bad' > \"$2\"", Some("cuda_output_shape")),
        (
            "printf '\\000\\000\\300\\177' > \"$2\"",
            Some("cuda_output_nonfinite"),
        ),
        (
            "printf '\\000\\000\\370\\101' > \"$2\"",
            Some("cuda_startup_parity_failed"),
        ),
        ("exit 1", Some("cuda_helper_exit_failed")),
        (
            "printf '\\000\\000\\000\\102\\000' > \"$2\"",
            Some("cuda_output_shape"),
        ),
        (
            "printf '\\000\\000\\200\\177' > \"$2\"",
            Some("cuda_output_nonfinite"),
        ),
        (
            "printf '\\000\\000\\200\\377' > \"$2\"",
            Some("cuda_output_nonfinite"),
        ),
        ("exec sleep 5", Some("cuda_helper_timed_out")),
    ] {
        fs::write(
            &helper,
            format!("#!/bin/sh\nif [ \"$1\" = --probe ]; then exit 0; fi\n{body}\n"),
        )
        .unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_hft-infer"))
            .env("HFT_CUDA_HELPER", &helper)
            .env("HFT_CUDA_TIMEOUT_MS", "100")
            .args([weights.to_str().unwrap(), input.to_str().unwrap(), "cuda"])
            .output()
            .unwrap();
        if let Some(error) = expected_error {
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stderr).contains(error));
        } else {
            assert!(output.status.success());
            let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(result["backend"], "cuda");
            assert_eq!(result["values"], serde_json::json!([32.0]));
        }
    }
    for value in ["0", "invalid", "30001"] {
        let output = Command::new(env!("CARGO_BIN_EXE_hft-infer"))
            .env("HFT_CUDA_HELPER", &helper)
            .env("HFT_CUDA_TIMEOUT_MS", value)
            .args([weights.to_str().unwrap(), input.to_str().unwrap(), "cuda"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("invalid_cuda_timeout"));
    }
}

#[cfg(unix)]
#[test]
fn cuda_bridge_emits_canonical_frames_and_rejects_batches_before_launch() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    let dir = std::env::temp_dir().join(format!("hft-cuda-framing-test-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(dir.clone());
    let weights = dir.join("weights.hqt");
    let input = dir.join("input.json");
    let helper = dir.join("helper.sh");
    let encoded = Quantized::new(&[0.5; 64], 2, 64).unwrap().encode();
    fs::write(&weights, &encoded).unwrap();
    fs::write(&helper, "#!/bin/sh\nif [ \"$1\" = --probe ]; then exit 0; fi\ncp \"$1\" \"$0.frame\"\nprintf '\\000\\000\\000\\102' > \"$2\"\n").unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
    for (values, valid) in [
        (vec![1.0; 64], true),
        (vec![], false),
        (vec![1.0; 63], false),
        (vec![1.0; 65], false),
        (vec![1.0; 64 * 257], false),
    ] {
        fs::write(&input, serde_json::to_vec(&values).unwrap()).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_hft-infer"))
            .env("HFT_CUDA_HELPER", &helper)
            .args([weights.to_str().unwrap(), input.to_str().unwrap(), "cuda"])
            .output()
            .unwrap();
        assert_eq!(output.status.success(), valid);
        if !valid {
            assert!(String::from_utf8_lossy(&output.stderr).contains("invalid_inference_batch"));
        }
        // Startup parity always sends a single all-ones row. Invalid user batches
        // must never replace it with a second subprocess request.
        let frame = fs::read(dir.join("helper.sh.frame")).unwrap();
        assert_eq!(&frame[..4], b"HCI1");
        assert_eq!(u32::from_le_bytes(frame[4..8].try_into().unwrap()), 1);
        assert_eq!(
            u32::from_le_bytes(frame[8..12].try_into().unwrap()) as usize,
            encoded.len()
        );
        assert_eq!(&frame[12..16], &[0; 4]);
        assert_eq!(&frame[16..16 + encoded.len()], encoded);
        assert_eq!(frame.len(), 16 + encoded.len() + 64 * 4);
        assert!(
            frame[16 + encoded.len()..]
                .chunks_exact(4)
                .all(|b| f32::from_le_bytes(b.try_into().unwrap()) == 1.0)
        );
    }
    fs::write(&helper, "#!/bin/sh\nexit 1\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_hft-infer"))
        .env("HFT_CUDA_HELPER", &helper)
        .args([weights.to_str().unwrap(), input.to_str().unwrap(), "cuda"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("backend_unavailable"));
}
