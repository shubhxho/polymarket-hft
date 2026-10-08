use crate::Result;

/// Packed affine group quantization reference kernel. Never used to quantize money.
pub struct Quantized {
    packed: Vec<u8>,
    scales: Vec<f32>,
    biases: Vec<f32>,
    bits: u8,
    group: usize,
    len: usize,
}
impl Quantized {
    pub fn new(weights: &[f32], bits: u8, group: usize) -> Result<Self> {
        if ![2, 4, 8].contains(&bits)
            || group == 0
            || weights.is_empty()
            || !weights.len().is_multiple_of(group)
            || weights.iter().any(|x| !x.is_finite())
        {
            return Err("invalid_quantization".into());
        }
        let mut out = Self {
            packed: vec![0; (weights.len() * usize::from(bits)).div_ceil(8)],
            scales: Vec::new(),
            biases: Vec::new(),
            bits,
            group,
            len: weights.len(),
        };
        let max_q = ((1u16 << bits) - 1) as f32;
        for (g, values) in weights.chunks_exact(group).enumerate() {
            let min = values.iter().copied().fold(f32::INFINITY, f32::min);
            let max = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let scale = if max == min { 1.0 } else { (max - min) / max_q };
            if !scale.is_finite() {
                return Err("quantization_range_overflow".into());
            }
            out.scales.push(scale);
            out.biases.push(min);
            for (j, &weight) in values.iter().enumerate() {
                let q = ((weight - min) / scale).round().clamp(0.0, max_q) as u8;
                let bit = (g * group + j) * usize::from(bits);
                out.packed[bit / 8] |= q << (bit % 8);
            }
        }
        Ok(out)
    }
    pub fn storage_bytes(&self) -> usize {
        self.packed.len() + 8 * self.scales.len()
    }
    pub fn dot(&self, input: &[f32]) -> Result<f32> {
        if input.len() != self.len || input.iter().any(|x| !x.is_finite()) {
            return Err("invalid_input_shape_or_value".into());
        }
        let mask = ((1u16 << self.bits) - 1) as u8;
        let mut sum = 0.0;
        for (i, &x) in input.iter().enumerate() {
            let bit = i * usize::from(self.bits);
            let q = (self.packed[bit / 8] >> (bit % 8)) & mask;
            let g = i / self.group;
            sum += (f32::from(q) * self.scales[g] + self.biases[g]) * x;
        }
        if !sum.is_finite() {
            return Err("inference_overflow".into());
        }
        Ok(sum)
    }
}
