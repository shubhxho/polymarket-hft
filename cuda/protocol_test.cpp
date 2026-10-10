#include "protocol.hpp"
#include <iostream>
#include <limits>
#include <string>
static void put(std::vector<uint8_t>& b, size_t at, uint32_t n) {
  for (size_t i=0;i<4;++i) b.at(at+i)=uint8_t(n>>(8*i));
}
static std::vector<uint8_t> frame(uint32_t bits=2,uint32_t width=1,uint32_t rows=1) {
  uint32_t packed=(width*bits+7)/8, qbytes=24+8+packed+32;
  std::vector<uint8_t> b(16+qbytes+width*rows*4);
  std::memcpy(b.data(),"HCI1",4); put(b,4,rows); put(b,8,qbytes);
  std::memcpy(b.data()+16,"HQT1",4); b[20]=uint8_t(bits);
  put(b,24,width); put(b,28,width); put(b,32,packed); put(b,36,1);
  put(b,40,0x3f800000); // scale 1
  for(size_t i=16+qbytes;i<b.size();i+=4) put(b,i,0x3f800000);
  return b;
}
static void require(bool ok) { if(!ok) throw std::runtime_error("assertion_failed"); }
static void reject(const std::vector<uint8_t>& b,const std::string& error) {
  try { (void)hft::parse(b); } catch(const std::runtime_error& e) { require(e.what()==error); return; }
  throw std::runtime_error("accepted_invalid_frame: "+error);
}
int main() { try {
  for(auto bits:{2u,4u,8u}) for(auto width:{1u,7u,64u,257u}) for(auto rows:{1u,3u,256u}) {
    auto parsed=hft::parse(frame(bits,width,rows));
    require(parsed.rows==rows && parsed.width==width && parsed.bits==bits && parsed.input.size()==size_t(width)*rows);
  }
  auto good=frame();
  for(size_t n=0;n<good.size();++n) { auto b=good; b.resize(n); reject(b,n<72?"input_size":(n<good.size()-4?"shape":"weights_shape")); }
  for(auto at:{0u,12u}) { auto b=good; b[at]=1; reject(b,"input_header"); }
  for(auto rows:{0u,257u,0xffffffffu}) {auto b=good;put(b,4,rows);reject(b,"shape");}
  for(auto count:{0u,55u,0xffffffffu}) {auto b=good;put(b,8,count);reject(b,"shape");}
  for(auto at:{16u,21u,22u,23u}) {auto b=good;b[at]=1;reject(b,"weights_header");}
  for(auto at:{24u,28u,32u,36u}) {auto b=good;put(b,at,0);reject(b,"weights_shape");}
  {auto b=good;b[20]=3;reject(b,"weights_shape");}
  {auto b=good;b.push_back(0);reject(b,"weights_shape");}
  {auto b=good;b[48]=4;reject(b,"weights_padding");}
  for(auto value:{0u,0xbf800000u,0x7fc00000u,0x7f800000u,0x7f7fffffu}) {auto b=good;put(b,40,value);reject(b,"weights_nonfinite");}
  for(auto value:{0x7fc00000u,0x7f800000u,0xff800000u}) {
    auto b=good;put(b,44,value);reject(b,"weights_nonfinite");
    b=good;put(b,b.size()-4,value);reject(b,"input_nonfinite");
  }
  std::cout<<"CUDA host protocol tests passed (no GPU execution)\n";return 0;
} catch(const std::exception& e) {std::cerr<<e.what()<<'\n';return 1;} }
