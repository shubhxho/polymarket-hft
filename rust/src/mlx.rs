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
    pub fn new(weights: &[f32], bits: i32) -> Result<Self> {
        Self::new_on_device(weights, bits, cfg!(feature = "mlx"))
    }
    pub fn new_on_device(weights: &[f32], bits: i32, gpu: bool) -> Result<Self> {
        if gpu && !cfg!(feature = "mlx") {
            return Err("mlx_metal_not_compiled".into());
        }
        if ![2, 4, 8].contains(&bits)
            || weights.is_empty()
            || !weights.len().is_multiple_of(64)
            || weights.len() > 16 * 1024 * 1024
            || weights.iter().any(|x| !x.is_finite())
        {
            return Err("invalid_mlx_weights".into());
        }

        let device = if gpu { Device::gpu() } else { Device::cpu() };
        let stream = Stream::try_default_on_device(&device).map_err(|e| e.to_string())?;
        let (weights_q, scales, biases) = with_stream(&stream, || {
            let dense = Array::from_slice(weights, &[1, weights.len() as i32]);
            quantize(&dense, 64, bits)
        })
        .map_err(|e| e.to_string())?;
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
    /// Import exact affine codes/parameters instead of requantizing a decoded model.
    pub fn from_quantized(q: &crate::quant::Quantized, gpu: bool) -> Result<Self> {
        if q.group() != 64
            || !q.len().is_multiple_of(64)
            || q.len() > 16 * 1024 * 1024
            || (gpu && !cfg!(feature = "mlx"))
        {
            return Err("invalid_mlx_packed_layout_or_device".into());
        }
        let (packed, scales, biases) = q.parts();
        let words: Vec<u32> = packed
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        let device = if gpu { Device::gpu() } else { Device::cpu() };
        let stream = Stream::try_default_on_device(&device).map_err(|e| e.to_string())?;
        let (weights, scales, biases) = with_stream(&stream, || {
            (
                Array::from_slice(&words, &[1, words.len() as i32]),
                Array::from_slice(scales, &[1, scales.len() as i32]),
                Array::from_slice(biases, &[1, biases.len() as i32]),
            )
        });
        weights.eval().map_err(|e| e.to_string())?;
        scales.eval().map_err(|e| e.to_string())?;
        biases.eval().map_err(|e| e.to_string())?;
        Ok(Self {
            stream,
            weights,
            scales,
            biases,
            width: q.len(),
            bits: i32::from(q.bits()),
        })
    }
    pub fn dot(&self, input: &[f32]) -> Result<f32> {
        if input.len() != self.width || input.iter().any(|x| !x.is_finite()) {
            return Err("invalid_mlx_input".into());
        }
        let output = with_stream(&self.stream, || {
            let x = Array::from_slice(input, &[1, self.width as i32]);
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
        if output.shape() != [1, 1] {
            return Err("mlx_output_shape".into());
        }
        let value = output.try_item_exact::<f32>().map_err(|e| e.to_string())?;
        if !value.is_finite() {
            return Err("mlx_inference_overflow".into());
        }
        Ok(value)
    }
    /// Batch cold-path inference to amortize dispatch. Money remains integer-valued in Rust.
    pub fn dot_batch(&self, input: &[f32]) -> Result<Vec<f32>> {
        if input.is_empty()
            || !input.len().is_multiple_of(self.width)
            || input.len() / self.width > 256
            || input.len() > 16 * 1024 * 1024
            || input.iter().any(|x| !x.is_finite())
        {
            return Err("invalid_mlx_batch".into());
        }
        let batch = input.len() / self.width;
        let output = with_stream(&self.stream, || {
            let x = Array::from_slice(input, &[batch as i32, self.width as i32]);
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
        if output.shape() != [batch as i32, 1] {
            return Err("mlx_output_shape".into());
        }
        let values = output
            .try_as_slice::<f32>()
            .map_err(|e| e.to_string())?
            .to_vec();
        if values.iter().any(|x| !x.is_finite()) {
            return Err("mlx_inference_overflow".into());
        }
        Ok(values)
    }
}
