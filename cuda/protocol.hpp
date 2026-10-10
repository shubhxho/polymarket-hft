#pragma once
// Host-side HCI1 validation shared by the CUDA executable and CPU-only tests.
// HQT1 checksum verification stays in Rust Quantized::decode; this boundary
// receives canonical frames encoded by Rust, not arbitrary model downloads.
#include <cstdint>
#include <cstring>
#include <cmath>
#include <stdexcept>
#include <vector>
namespace hft {
inline uint32_t u32(const uint8_t* b) {
  return uint32_t(b[0]) | (uint32_t(b[1]) << 8) | (uint32_t(b[2]) << 16) | (uint32_t(b[3]) << 24);
}
inline float f32(const uint8_t* b) {
  uint32_t bits = u32(b); float value; std::memcpy(&value, &bits, 4); return value;
}
struct Frame {
  uint32_t rows, bits, group, width, packed;
  std::vector<uint8_t> weights;
  std::vector<float> scales, biases, input;
};
inline Frame parse(const std::vector<uint8_t>& b) {
  if (b.size() < 72 || b.size() > 224u * 1024 * 1024) throw std::runtime_error("input_size");
  if (std::memcmp(b.data(), "HCI1", 4) || u32(b.data() + 12)) throw std::runtime_error("input_header");
  uint32_t rows = u32(b.data() + 4), qbytes = u32(b.data() + 8);
  if (!rows || rows > 256 || qbytes < 56 || uint64_t(qbytes) + 16 > b.size()) throw std::runtime_error("shape");
  auto q = b.data() + 16;
  if (std::memcmp(q, "HQT1", 4) || q[5] || q[6] || q[7]) throw std::runtime_error("weights_header");
  uint32_t bits = q[4], group = u32(q + 8), width = u32(q + 12), packed = u32(q + 16), groups = u32(q + 20);
  if ((bits != 2 && bits != 4 && bits != 8) || !width || width > 16u * 1024 * 1024 || !group || width % group ||
      groups != width / group || packed != (uint64_t(width) * bits + 7) / 8 ||
      qbytes != 24 + uint64_t(groups) * 8 + packed + 32 || uint64_t(width) * rows > 16u * 1024 * 1024 ||
      b.size() != 16 + uint64_t(qbytes) + uint64_t(width) * rows * 4) throw std::runtime_error("weights_shape");
  Frame out{rows, bits, group, width, packed, {}, {}, {}, {}};
  out.scales.reserve(groups); out.biases.reserve(groups);
  for (uint32_t i = 0; i < groups; ++i) {
    float scale = f32(q + 24 + i * 8), bias = f32(q + 28 + i * 8);
    if (!std::isfinite(scale) || scale <= 0 || !std::isfinite(bias) ||
        !std::isfinite(scale * float((1u << bits) - 1) + bias)) throw std::runtime_error("weights_nonfinite");
    out.scales.push_back(scale); out.biases.push_back(bias);
  }
  auto w = q + 24 + groups * 8;
  uint32_t used = (width * bits) % 8;
  if (used && (w[packed - 1] >> used)) throw std::runtime_error("weights_padding");
  out.weights.assign(w, w + packed);
  out.input.reserve(size_t(width) * rows);
  for (size_t i = 0; i < size_t(width) * rows; ++i) {
    float value = f32(b.data() + 16 + qbytes + i * 4);
    if (!std::isfinite(value)) throw std::runtime_error("input_nonfinite");
    out.input.push_back(value);
  }
  return out;
}
}
