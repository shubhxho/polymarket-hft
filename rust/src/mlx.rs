use crate::Result;
use mlx_rs::{
    Array, Device, Stream,
    ops::{quantize, quantized_matmul},
    with_stream,
};

/// Native MLX quantized matrix multiply, accessed through the Rust C-API binding.
/// Explicit eval is required because MLX operations are lazy.
pub struct MlxLinear {
    stream: Stream,
    weights: Array,
    scales: Array,
    biases: Array,
    width: usize,
    bits: i32,
}
impl MlxLinear {
    fn device() -> Device {
        if cfg!(feature = "mlx") {
            Device::gpu()
        } else {
            Device::cpu()
        }
    }
    pub fn new(weights: &[f32], bits: i32) -> Result<Self> {
        if ![2, 4, 8].contains(&bits)
            || weights.is_empty()
            || !weights.len().is_multiple_of(64)
            || weights.len() > i32::MAX as usize
            || weights.iter().any(|x| !x.is_finite())
        {
            return Err("invalid_mlx_weights".into());
        }
        let dense = Array::from_slice(weights, &[1, weights.len() as i32]);
        let stream = Stream::try_default_on_device(&Self::device()).map_err(|e| e.to_string())?;
        let (weights_q, scales, biases) =
            with_stream(&stream, || quantize(&dense, 64, bits)).map_err(|e| e.to_string())?;
        weights_q.eval().map_err(|e| e.to_string())?;
        scales.eval().map_err(|e| e.to_string())?;
        biases.eval().map_err(|e| e.to_string())?;
        Ok(Self {
            stream,
            weights: weights_q,
            scales,
            biases,
            width: weights.len(),
            bits,
        })
    }
    pub fn dot(&self, input: &[f32]) -> Result<f32> {
        if input.len() != self.width || input.iter().any(|x| !x.is_finite()) {
            return Err("invalid_mlx_input".into());
        }
        let x = Array::from_slice(input, &[1, self.width as i32]);
        let output = with_stream(&self.stream, || {
            quantized_matmul(
                &x,
                &self.weights,
                &self.scales,
                &self.biases,
                true,
                64,
                self.bits,
            )
        })
        .map_err(|e| e.to_string())?;
        output.eval().map_err(|e| e.to_string())?;
        let value = output.item_exact::<f32>();
        if !value.is_finite() {
            return Err("mlx_inference_overflow".into());
        }
        Ok(value)
    }
    /// Batch cold-path inference to amortize dispatch. Money remains integer-valued in Rust.
    pub fn dot_batch(&self, input: &[f32]) -> Result<Vec<f32>> {
        if input.is_empty()
            || !input.len().is_multiple_of(self.width)
            || input.len() / self.width > i32::MAX as usize
            || input.iter().any(|x| !x.is_finite())
        {
            return Err("invalid_mlx_batch".into());
        }
        let batch = input.len() / self.width;
        let x = Array::from_slice(input, &[batch as i32, self.width as i32]);
        let output = with_stream(&self.stream, || {
            quantized_matmul(
                &x,
                &self.weights,
                &self.scales,
                &self.biases,
                true,
                64,
                self.bits,
            )
        })
        .map_err(|e| e.to_string())?;
        output.eval().map_err(|e| e.to_string())?;
        let values = output.as_slice::<f32>().to_vec();
        if values.iter().any(|x| !x.is_finite()) {
            return Err("mlx_inference_overflow".into());
        }
        Ok(values)
    }
}
