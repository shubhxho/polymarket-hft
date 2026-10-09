use crate::Result;
use sha2::{Digest, Sha256};
const MAX_WEIGHTS: usize = 16 * 1024 * 1024;

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
            || weights.len() > MAX_WEIGHTS
            || !weights.len().is_multiple_of(group)
            || weights.iter().any(|x| !x.is_finite())
        {
            return Err("invalid_quantization".into());
        }
        let mut out = Self {
            packed: vec![0; (weights.len() * usize::from(bits)).div_ceil(8)],
            scales: Vec::with_capacity(weights.len() / group),
            biases: Vec::with_capacity(weights.len() / group),
            bits,
            group,
            len: weights.len(),
        };
        let max_q = ((1u16 << bits) - 1) as f32;
        for (g, values) in weights.chunks_exact(group).enumerate() {
            let min = values.iter().copied().fold(f32::INFINITY, f32::min);
            let max = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let scale = if max == min { 1.0 } else { (max - min) / max_q };
            if !scale.is_finite() || scale <= 0.0 {
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
        self.dot_finite_row(input)
    }
    /// Internal loaded-model path: caller has checked shape and finite values for the entire batch.
    pub(crate) fn dot_finite_row(&self, input: &[f32]) -> Result<f32> {
        debug_assert_eq!(input.len(), self.len);
        let sum = match self.bits {
            2 => self.dot_packed::<2>(input),
            4 => self.dot_packed::<4>(input),
            8 => self.dot_packed::<8>(input),
            _ => unreachable!(),
        };
        if !sum.is_finite() {
            return Err("inference_overflow".into());
        }
        Ok(sum)
    }
    fn dot_packed<const BITS: usize>(&self, input: &[f32]) -> f32 {
        let mut sum = 0.0;
        let mask = ((1u16 << BITS) - 1) as u8;
        for (g, values) in input.chunks_exact(self.group).enumerate() {
            let scale = self.scales[g];
            let bias = self.biases[g];
            let start = g * self.group;
            for (j, &x) in values.iter().enumerate() {
                let i = start + j;
                let q = (self.packed[i / (8 / BITS)] >> ((i % (8 / BITS)) * BITS)) & mask;
                sum += (f32::from(q) * scale + bias) * x;
            }
        }
        sum
    }
    #[cfg(any(feature = "mlx", feature = "mlx-cpu"))]
    pub(crate) fn parts(&self) -> (&[u8], &[f32], &[f32]) {
        (&self.packed, &self.scales, &self.biases)
    }
    pub fn dequantize(&self) -> Vec<f32> {
        let mask = ((1u16 << self.bits) - 1) as u8;
        (0..self.len)
            .map(|i| {
                let bit = i * usize::from(self.bits);
                let q = (self.packed[bit / 8] >> (bit % 8)) & mask;
                f32::from(q) * self.scales[i / self.group] + self.biases[i / self.group]
            })
            .collect()
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn bits(&self) -> u8 {
        self.bits
    }
    pub fn group(&self) -> usize {
        self.group
    }
    /// Canonical little-endian HQT1 weights with SHA-256 corruption detection.
    /// Digest is not authentication. Limits are checked before allocating on decode.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(24 + self.storage_bytes() + 32);
        bytes.extend_from_slice(b"HQT1");
        bytes.push(self.bits);
        bytes.extend_from_slice(&[0; 3]);
        for n in [self.group, self.len, self.packed.len(), self.scales.len()] {
            bytes.extend_from_slice(&(n as u32).to_le_bytes());
        }
        for (&scale, &bias) in self.scales.iter().zip(&self.biases) {
            bytes.extend_from_slice(&scale.to_le_bytes());
            bytes.extend_from_slice(&bias.to_le_bytes());
        }
        bytes.extend_from_slice(&self.packed);
        let digest = Sha256::digest(&bytes);
        bytes.extend_from_slice(&digest);
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 56
            || &bytes[..4] != b"HQT1"
            || bytes[5..8] != [0; 3]
            || ![2, 4, 8].contains(&bytes[4])
        {
            return Err("invalid_quantized_header".into());
        }
        let read = |i| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        let (group, len, packed_len, groups) = (read(8), read(12), read(16), read(20));
        let bits = bytes[4];
        if len == 0
            || len > MAX_WEIGHTS
            || group == 0
            || !len.is_multiple_of(group)
            || groups != len / group
            || packed_len != (len * usize::from(bits)).div_ceil(8)
            || bytes.len() != 24 + groups * 8 + packed_len + 32
        {
            return Err("invalid_quantized_shape".into());
        }
        let digest = Sha256::digest(&bytes[..bytes.len() - 32]);
        if digest.as_slice() != &bytes[bytes.len() - 32..] {
            return Err("quantized_checksum_mismatch".into());
        }
        let mut scales = Vec::with_capacity(groups);
        let mut biases = Vec::with_capacity(groups);
        for chunk in bytes[24..24 + groups * 8].chunks_exact(8) {
            let scale = f32::from_le_bytes(chunk[..4].try_into().unwrap());
            let bias = f32::from_le_bytes(chunk[4..].try_into().unwrap());
            if !scale.is_finite()
                || scale <= 0.0
                || !bias.is_finite()
                || !(scale * ((1u16 << bits) - 1) as f32 + bias).is_finite()
            {
                return Err("invalid_quantized_parameters".into());
            }
            scales.push(scale);
            biases.push(bias);
        }
        let packed = bytes[24 + groups * 8..bytes.len() - 32].to_vec();
        let used = (len * usize::from(bits)) % 8;
        if used != 0 && packed.last().unwrap() >> used != 0 {
            return Err("noncanonical_quantized_padding".into());
        }
        Ok(Self {
            packed,
            scales,
            biases,
            bits,
            group,
            len,
        })
    }
}
