# Reference systems

Primary-source references reviewed for architecture and evaluation coverage. No whole-system competitor benchmarks were run and no outperformance claim is established.

| Reference | Surface | Gap in this project |
|---|---|---|
| [hftbacktest](https://github.com/nkaz001/hftbacktest) | Rust/Python replay with queue and latency modeling | Historical data, queue, latency and impact models |
| [Barter](https://github.com/barter-rs/barter-rs) | Rust trading/backtesting infrastructure | Service lifecycle and authenticated adapters |
| [Polymarket Rust client](https://github.com/Polymarket/rs-clob-client) | Official venue client | Base-only policy blocks the published settlement venue |
| Hudson River Trading | Proprietary production systems | No supplied reproducible comparable benchmark |

Local references measure BTree and full-scan top lookup only. They do not compare funds or trading returns.

Jev integration follows the [TypeSafe API](https://docs.typesafe.ai/api), [Noul primitive](https://docs.typesafe.ai/primitives/noul) and [evidence cookbook](https://docs.typesafe.ai/cookbooks/citation_check). Gates use synthetic responses; live quality and cost remain unmeasured.

[Polymarket's contract reference](https://docs.polymarket.com/resources/contracts) documents Polygon contracts. Synthetic Base-tagged fixtures do not imply a Base exchange deployment.
