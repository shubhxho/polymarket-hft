use crate::Result;
use mlx_rs::{
    Array,
    ops::{quantize, quantized_matmul},
};

/// Native MLX quantized matrix multiply, accessed through the Rust C-API binding.
/// Explicit eval is required because MLX operations are lazy.
pub struct MlxLinear {
    weights: Array,
    scales: Array,
    biases: Array,
    width: usize,
    bits: i32,
}
impl MlxLinear {
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
        let (weights_q, scales, biases) = quantize(&dense, 64, bits).map_err(|e| e.to_string())?;
        weights_q.eval().map_err(|e| e.to_string())?;
        scales.eval().map_err(|e| e.to_string())?;
        biases.eval().map_err(|e| e.to_string())?;
        Ok(Self {
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
        let output = quantized_matmul(
            &x,
            &self.weights,
            &self.scales,
            &self.biases,
            true,
            64,
            self.bits,
        )
        .map_err(|e| e.to_string())?;
        output.eval().map_err(|e| e.to_string())?;
        let value = output.item::<f32>();
        if !value.is_finite() {
            return Err("mlx_inference_overflow".into());
        }
        Ok(value)
    }
}
