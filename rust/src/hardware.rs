//! Hardware hints and executable backend readiness are separate facts.
use crate::{Result, quant::Quantized};
use serde::Serialize;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    Cpu,
    MlxCpu,
    MlxMetal,
    Cuda,
}
#[derive(Serialize)]
pub struct Hardware {
    pub os: &'static str,
    pub arch: &'static str,
    pub apple_silicon: bool,
    pub nvidia_runtime_detected: bool,
    pub cuda_helper_ready: bool,
    pub mlx_cpu_compiled: bool,
    pub mlx_metal_compiled: bool,
    pub selected: Backend,
}
fn succeeds(mut command: Command) -> bool {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let Ok(mut child) = command.spawn() else {
        return false;
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(s)) => return s.success(),
            Err(_) => break,
            Ok(None) => (),
        };
        if start.elapsed() > Duration::from_secs(3) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = child.kill();
    let _ = child.wait();
    false
}
pub fn select(apple: bool, cuda_ready: bool, mlx_cpu: bool, mlx_metal: bool) -> Backend {
    if cuda_ready {
        Backend::Cuda
    } else if apple && mlx_metal {
        Backend::MlxMetal
    } else if mlx_cpu {
        Backend::MlxCpu
    } else {
        Backend::Cpu
    }
}
pub fn detect() -> Hardware {
    let apple = cfg!(all(target_os = "macos", target_arch = "aarch64"));
    let mut command = Command::new("nvidia-smi");
    command.args(["--query-gpu=name", "--format=csv,noheader"]);
    let nvidia = succeeds(command);
    let helper = std::env::var_os("HFT_CUDA_HELPER").map(PathBuf::from);
    let cuda = helper.as_ref().is_some_and(|path| {
        let mut command = Command::new(path);
        command.arg("--probe");
        succeeds(command)
    });
    let mlx_cpu = cfg!(any(feature = "mlx", feature = "mlx-cpu"));
    let mlx_metal = cfg!(feature = "mlx");
    Hardware {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        apple_silicon: apple,
        nvidia_runtime_detected: nvidia,
        cuda_helper_ready: cuda,
        mlx_cpu_compiled: mlx_cpu,
        mlx_metal_compiled: mlx_metal,
        selected: select(apple, cuda, mlx_cpu, mlx_metal),
    }
}
/// A loaded CPU/MLX model probes once and retains quantized device weights.
pub struct LoadedModel {
    weights: Quantized,
    backend: Backend,
    #[cfg(any(feature = "mlx", feature = "mlx-cpu"))]
    mlx: Option<crate::mlx::MlxLinear>,
}
impl LoadedModel {
    pub fn new(weights: Quantized, requested: &str) -> Result<Self> {
        // An explicit CPU request needs no hardware probe.
        let backend = if requested == "cpu" {
            Backend::Cpu
        } else {
            let hw = detect();
            match requested {
                "auto" => hw.selected,
                "mlx-cpu" if hw.mlx_cpu_compiled => Backend::MlxCpu,
                "mlx-metal" if hw.apple_silicon && hw.mlx_metal_compiled => Backend::MlxMetal,
                "cuda" if hw.cuda_helper_ready => Backend::Cuda,
                _ => return Err("backend_unavailable".into()),
            }
        };
        #[cfg(any(feature = "mlx", feature = "mlx-cpu"))]
        let mlx = if matches!(backend, Backend::MlxCpu | Backend::MlxMetal) {
            if weights.group() != 64 {
                return Err("mlx_requires_group64".into());
            }
            Some(crate::mlx::MlxLinear::from_quantized(
                &weights,
                backend == Backend::MlxMetal,
            )?)
        } else {
            None
        };
        Ok(Self {
            weights,
            backend,
            #[cfg(any(feature = "mlx", feature = "mlx-cpu"))]
            mlx,
        })
    }
    pub fn backend(&self) -> Backend {
        self.backend
    }
    pub fn infer_batch(&self, input: &[f32]) -> Result<Vec<f32>> {
        if input.is_empty()
            || !input.len().is_multiple_of(self.weights.len())
            || input.len() > 16 * 1024 * 1024
            || input.len() / self.weights.len() > 256
            || input.iter().any(|x| !x.is_finite())
        {
            return Err("invalid_inference_batch".into());
        }
        match self.backend {
            Backend::Cpu => input
                .chunks_exact(self.weights.len())
                .map(|row| self.weights.dot(row))
                .collect(),
            Backend::Cuda => cuda_batch(&self.weights, input),
            Backend::MlxCpu | Backend::MlxMetal => {
                #[cfg(any(feature = "mlx", feature = "mlx-cpu"))]
                {
                    self.mlx
                        .as_ref()
                        .ok_or("mlx_model_missing")?
                        .dot_batch(input)
                }
                #[cfg(not(any(feature = "mlx", feature = "mlx-cpu")))]
                {
                    Err("mlx_not_compiled".into())
                }
            }
        }
    }
}
struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn cuda_batch(weights: &Quantized, input: &[f32]) -> Result<Vec<f32>> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "hft-cuda-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
    let temp = Temp(dir);
    let encoded = weights.encode();
    let mut payload = Vec::with_capacity(16 + encoded.len() + input.len() * 4);
    payload.extend_from_slice(b"HCI1");
    payload.extend_from_slice(&((input.len() / weights.len()) as u32).to_le_bytes());
    payload.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
    payload.extend_from_slice(&[0; 4]);
    payload.extend_from_slice(&encoded);
    for x in input {
        payload.extend_from_slice(&x.to_le_bytes());
    }
    let source = temp.0.join("input.bin");
    let output = temp.0.join("output.bin");
    std::fs::write(&source, payload).map_err(|e| e.to_string())?;
    let mut command =
        Command::new(std::env::var_os("HFT_CUDA_HELPER").ok_or("cuda_helper_missing")?);
    command.arg(&source).arg(&output);
    if !succeeds(command) {
        return Err("cuda_helper_failed_or_timed_out".into());
    }
    let expected = input.len() / weights.len() * 4;
    let file = std::fs::File::open(&output).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() != expected as u64 {
        return Err("cuda_output_shape".into());
    }
    let mut bytes = Vec::with_capacity(expected);
    std::io::Read::read_to_end(
        &mut std::io::Read::take(file, expected as u64 + 1),
        &mut bytes,
    )
    .map_err(|e| e.to_string())?;
    if bytes.len() != input.len() / weights.len() * 4 {
        return Err("cuda_output_shape".into());
    }
    let values: Vec<_> = bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    if values.iter().any(|x| !x.is_finite()) {
        return Err("cuda_output_nonfinite".into());
    }
    Ok(values)
}
