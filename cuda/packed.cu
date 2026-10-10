// Experimental CUDA port of the affine packed CPU dot product. Not validated on this host.
#include <cuda_runtime.h>
#include <cstdint>
#include <cmath>
#include <fstream>
#include <iostream>
#include <vector>
#include <cstring>
#include <stdexcept>
#include "protocol.hpp"
static void check(cudaError_t e){if(e!=cudaSuccess)throw std::runtime_error(cudaGetErrorString(e));}
// Eight warp partials and one block barrier; bit width is a compile-time constant.
// No fast-math: preserve ordinary float semantics for the CPU parity gate.
template <uint32_t Bits>
__global__ void packed_dot(const uint8_t* __restrict__ w,
                          const float* __restrict__ scales,
                          const float* __restrict__ biases,
                          const float* __restrict__ x,
                          float* __restrict__ y, uint32_t width, uint32_t group) {
  __shared__ float warp_sums[8];
  float sum = 0.0f;
  const uint32_t row = blockIdx.x;
  const uint32_t lane = threadIdx.x & 31;
  const uint32_t warp = threadIdx.x >> 5;
  for (uint32_t i = threadIdx.x; i < width; i += blockDim.x) {
    const uint32_t q = (w[i / (8 / Bits)] >> ((i % (8 / Bits)) * Bits)) & ((1u << Bits) - 1);
    sum += (float(q) * scales[i / group] + biases[i / group]) * x[size_t(row) * width + i];
  }
  for (uint32_t offset = 16; offset; offset >>= 1)
    sum += __shfl_down_sync(0xffffffffu, sum, offset);
  if (lane == 0) warp_sums[warp] = sum;
  __syncthreads();
  if (warp == 0) {
    sum = lane < 8 ? warp_sums[lane] : 0.0f;
    for (uint32_t offset = 16; offset; offset >>= 1)
      sum += __shfl_down_sync(0xffffffffu, sum, offset);
    if (lane == 0) y[row] = sum;
  }
}
struct Stream { cudaStream_t value; Stream(){check(cudaStreamCreateWithFlags(&value,cudaStreamNonBlocking));} ~Stream(){cudaStreamDestroy(value);} Stream(const Stream&)=delete; };
struct DeviceBuffer {void* p=nullptr;DeviceBuffer(size_t n){check(cudaMalloc(&p,n));}~DeviceBuffer(){if(p)cudaFree(p);}DeviceBuffer(const DeviceBuffer&)=delete;};
int main(int argc,char** argv){try{
 if(argc==2&&std::string(argv[1])=="--probe"){int n=0;check(cudaGetDeviceCount(&n));if(n<1)return 1;check(cudaSetDevice(0));check(cudaFree(nullptr));return 0;}
 if(argc!=3)throw std::runtime_error("usage: hft-cuda input.bin output.bin");
 std::ifstream f(argv[1],std::ios::binary|std::ios::ate);if(!f)throw std::runtime_error("input_missing");auto length=f.tellg();if(length<72||length>224*1024*1024)throw std::runtime_error("input_size");f.seekg(0);std::vector<uint8_t> b(static_cast<size_t>(length));if(!f.read(reinterpret_cast<char*>(b.data()),b.size()))throw std::runtime_error("input_read");
 auto frame = hft::parse(b);
 const auto rows = frame.rows, bits = frame.bits, group = frame.group, width = frame.width, packed = frame.packed;
 const auto groups = frame.scales.size();
 auto& scales = frame.scales; auto& biases = frame.biases; auto& x = frame.input;
 std::vector<float> y(rows);
 check(cudaSetDevice(0));cudaDeviceProp properties{};check(cudaGetDeviceProperties(&properties,0));if(properties.maxThreadsPerBlock<256||rows>uint32_t(properties.maxGridSize[0]))throw std::runtime_error("launch_coordinates_unsupported");
 Stream stream;
 DeviceBuffer dw(packed),ds(groups*4),db(groups*4),dx(x.size()*4),dy(rows*4);check(cudaMemcpyAsync(dw.p,frame.weights.data(),packed,cudaMemcpyHostToDevice,stream.value));check(cudaMemcpyAsync(ds.p,scales.data(),groups*4,cudaMemcpyHostToDevice,stream.value));check(cudaMemcpyAsync(db.p,biases.data(),groups*4,cudaMemcpyHostToDevice,stream.value));check(cudaMemcpyAsync(dx.p,x.data(),x.size()*4,cudaMemcpyHostToDevice,stream.value));
 switch(bits) {
 case 2: packed_dot<2><<<rows,256,0,stream.value>>>(static_cast<uint8_t*>(dw.p),static_cast<float*>(ds.p),static_cast<float*>(db.p),static_cast<float*>(dx.p),static_cast<float*>(dy.p),width,group); break;
 case 4: packed_dot<4><<<rows,256,0,stream.value>>>(static_cast<uint8_t*>(dw.p),static_cast<float*>(ds.p),static_cast<float*>(db.p),static_cast<float*>(dx.p),static_cast<float*>(dy.p),width,group); break;
 case 8: packed_dot<8><<<rows,256,0,stream.value>>>(static_cast<uint8_t*>(dw.p),static_cast<float*>(ds.p),static_cast<float*>(db.p),static_cast<float*>(dx.p),static_cast<float*>(dy.p),width,group); break;
}check(cudaGetLastError());check(cudaMemcpyAsync(y.data(),dy.p,rows*4,cudaMemcpyDeviceToHost,stream.value));check(cudaStreamSynchronize(stream.value));for(float v:y)if(!std::isfinite(v))throw std::runtime_error("output_nonfinite");
 std::ofstream o(argv[2],std::ios::binary|std::ios::trunc);o.write(reinterpret_cast<char*>(y.data()),rows*4);o.flush();if(!o)throw std::runtime_error("output_write");return 0;
 }catch(const std::exception& e){std::cerr<<e.what()<<'\n';return 1;}}
