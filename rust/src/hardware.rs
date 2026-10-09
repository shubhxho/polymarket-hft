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
#[derive(Debug, PartialEq, Eq)]
enum ProcessOutcome {
    Success,
    SpawnFailed,
    WaitFailed,
    ExitFailed,
    TimedOut,
}
fn run_bounded(mut command: Command, timeout: Duration) -> ProcessOutcome {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let Ok(mut child) = command.spawn() else {
        return ProcessOutcome::SpawnFailed;
    };
    let start = Instant::now();
    let mut pause = Duration::from_millis(1);
    let outcome = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    ProcessOutcome::Success
                } else {
                    ProcessOutcome::ExitFailed
                };
            }
            Err(_) => break ProcessOutcome::WaitFailed,
            Ok(None) => (),
        }
        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            break ProcessOutcome::TimedOut;
        }
        std::thread::sleep(pause.min(remaining));
        pause = (pause * 2).min(Duration::from_millis(4));
    };
    if matches!(
        outcome,
        ProcessOutcome::TimedOut | ProcessOutcome::WaitFailed
    ) {
        let _ = child.kill();
        let _ = child.wait();
    }
    outcome
}
fn succeeds(command: Command) -> bool {
    run_bounded(command, Duration::from_secs(3)) == ProcessOutcome::Success
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
    let helper = std::env::var_os("HFT_CUDA_HELPER").map(PathBuf::from);
    detect_with_helper(helper.as_deref())
}
fn detect_with_helper(helper: Option<&std::path::Path>) -> Hardware {
    let apple = cfg!(all(target_os = "macos", target_arch = "aarch64"));
    let mut command = Command::new("nvidia-smi");
    command.args(["--query-gpu=name", "--format=csv,noheader"]);
    let nvidia = succeeds(command);
    let cuda = helper.is_some_and(|path| {
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
    cuda: Option<CudaModel>,
    #[cfg(any(feature = "mlx", feature = "mlx-cpu"))]
    mlx: Option<crate::mlx::MlxLinear>,
}
impl LoadedModel {
    pub fn new(weights: Quantized, requested: &str) -> Result<Self> {
        let helper = std::env::var_os("HFT_CUDA_HELPER").map(PathBuf::from);
        // An explicit CPU request needs no hardware probe.
        let backend = if requested == "cpu" {
            Backend::Cpu
        } else {
            let hw = detect_with_helper(helper.as_deref());
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
        let cuda = if backend == Backend::Cuda {
            Some(CudaModel {
                helper: helper.ok_or("cuda_helper_missing")?,
                timeout: cuda_timeout()?,
                encoded: weights.encode(),
            })
        } else {
            None
        };
        if let Some(model) = &cuda {
            // A device probe alone does not establish that loaded inference is usable.
            // This startup smoke gate is additional to the full NVIDIA parity suite.
            let input = vec![1.0; weights.len()];
            let expected = weights.dot(&input)?;
            let actual = model.infer_batch(weights.len(), &input)?;
            if actual.len() != 1 || (actual[0] - expected).abs() > 1e-4 + 1e-4 * expected.abs() {
                return Err("cuda_startup_parity_failed".into());
            }
        }
        Ok(Self {
            cuda,
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
                .map(|row| self.weights.dot_finite_row(row))
                .collect(),
            Backend::Cuda => self
                .cuda
                .as_ref()
                .ok_or("cuda_model_missing")?
                .infer_batch(self.weights.len(), input),
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
fn cuda_timeout() -> Result<Duration> {
    let millis = match std::env::var("HFT_CUDA_TIMEOUT_MS") {
        Ok(value) => value.parse::<u64>().map_err(|_| "invalid_cuda_timeout")?,
        Err(std::env::VarError::NotPresent) => 3000,
        Err(_) => return Err("invalid_cuda_timeout".into()),
    };
    if !(1..=30_000).contains(&millis) {
        return Err("invalid_cuda_timeout".into());
    }
    Ok(Duration::from_millis(millis))
}
struct CudaModel {
    helper: PathBuf,
    timeout: Duration,
    encoded: Vec<u8>,
}
impl CudaModel {
    fn infer_batch(&self, width: usize, input: &[f32]) -> Result<Vec<f32>> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "hft-cuda-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
        let temp = Temp(dir);
        let encoded = &self.encoded;
        let mut payload = Vec::with_capacity(16 + encoded.len() + input.len() * 4);
        payload.extend_from_slice(b"HCI1");
        payload.extend_from_slice(&((input.len() / width) as u32).to_le_bytes());
        payload.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        payload.extend_from_slice(&[0; 4]);
        payload.extend_from_slice(encoded);
        for x in input {
            payload.extend_from_slice(&x.to_le_bytes());
        }
        let source = temp.0.join("input.bin");
        let output = temp.0.join("output.bin");
        std::fs::write(&source, payload).map_err(|e| e.to_string())?;
        let mut command = Command::new(&self.helper);
        command.arg(&source).arg(&output);
        match run_bounded(command, self.timeout) {
            ProcessOutcome::Success => (),
            ProcessOutcome::TimedOut => return Err("cuda_helper_timed_out".into()),
            ProcessOutcome::ExitFailed => return Err("cuda_helper_exit_failed".into()),
            ProcessOutcome::SpawnFailed => return Err("cuda_helper_spawn_failed".into()),
            ProcessOutcome::WaitFailed => return Err("cuda_helper_wait_failed".into()),
        }
        let expected = input.len() / width * 4;
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
        if bytes.len() != input.len() / width * 4 {
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
}
