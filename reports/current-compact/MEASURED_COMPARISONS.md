# Measured comparisons

We compare our cached best-price lookup against explicitly implemented full-scan and BTree references. This is an operation comparison on identical synthetic books, not a competitor engine or HRT result.

64 markets, 1,001 possible price slots, 512 lookups per timed batch, 100 warmup batches, 500 samples per backend and five rounds. Backend order rotates each round. Query order is deterministic and identical across backends. Every queried best bid/ask is checked for equality before timing. Raw batch times are retained.

| Levels per side | Cached median range ns/lookup | Full scan / cached median ratio range | BTree / cached median ratio range |
|---|---:|---:|---:|
| 1 | 2.44–6.92 | 53.21–147.53x | 0.33–0.93x |
| 16 | 2.44–2.52 | 145.61–148.03x | 1.06–1.13x |
| 128 | 2.44–2.52 | 142.20–148.03x | 1.10–1.13x |

## What we measured faster

Compact correctness profile. Timings are diagnostic; no optimized performance claim is established.

![Measured reference comparison](comparison.svg)

Ratios greater than one mean cached lookup was faster in that round. Ranges are observed round-to-round variation, not statistical confidence intervals. These are amortized batch values, not individual lookup latency percentiles. Setup, snapshots, updates, matching, feed/network latency and durable writes are excluded. Cached metadata avoids searching; maintaining that cache has a separate cost. No CPU pinning or hardware performance counters were used.

No claim is made that we beat hftbacktest, Barter, the Polymarket client or HRT: no comparable whole-system implementation/data was run. Packed quantization storage reductions are reported separately; smaller weights do not imply faster or more accurate trading.

Reproduce: make compare. Raw samples: [comparison.json](comparison.json). References: [COMPARISONS.md](COMPARISONS.md).
