# Input and configuration reference

The Rust types in rust/src/engine.rs are authoritative. JSON uses snake_case event tags and rejects unknown fields. Prices/cash are unsigned integer millionths of a dollar; shares are unsigned whole units. Epoch timestamps use milliseconds. IDs are caller-provided strings.

| Event type | Fields besides type | Effect |
|---|---|---|
| market | token, city, tick, chain_id, fee_bps | Registers a paper market; only chain 137 |
| snapshot | token, bids, asks, sequence, time_ms | Replaces book; level objects contain price and shares |
| update | token, side, level, sequence, time_ms | Replaces one level; zero shares removes it |
| trade | token, side, fair_micro, shares, time_ms | Attempts a paper buy/sell against eligible depth |
| rebalance | token, fair_micro, target_shares, time_ms | Attempts the difference from existing inventory |
| weather | tokens, buckets, metric, observed, mu, sigma, hours, shares, time_ms | Chooses the eligible buy with highest top-level edge |
| halt | reason | Halts all execution |

side is buy or sell. Snapshot bids are buy-side liquidity and asks are sell-side liquidity. Update side identifies the resting side. Weather buckets use inclusive signed integer lo and hi; null is not accepted. Use sufficiently wide finite outer bounds to retain probability mass. metric is high or low. Token and bucket counts must match. Temperature, mu and sigma share the same unit; examples use Fahrenheit. hours is remaining time in hours.

## Default risk configuration

Supply the complete configuration as the optional replay/durable-replay argument. There is no implicit environment override.

| Field | Default | Meaning |
|---|---:|---|
| chain_id | 137 | Polygon mainnet only |
| cash_micro | 1000000000 | Initial paper cash, $1,000 |
| max_order_micro | 25000000 | Order budget, $25 |
| max_city_micro | 150000000 | City cost-basis cap, $150 |
| max_position | 500 | Shares per market |
| min_shares | 5 | Minimum requested/executed size |
| min_edge_micro | 40000 | Minimum fee-adjusted edge, $0.04/share |
| max_book_age_ms | 2000 | Maximum snapshot/update age |
| max_orders_per_second | 8 | Rolling order-attempt limit |
| max_loss_micro | 75000000 | Session loss threshold, $75 |

Fees use flat notional basis points, rounded up per share. Risk caps and book freshness can reduce fills or reject an order. Rebalance is a target request, not a guaranteed final holding.

## News envelope

Each line has item and response. item contains id, source_url, station, published_ms, received_ms and text. station is four uppercase ASCII letters/digits. Text is capped at 16 KiB, ID at 128 bytes and URL at 2,048 bytes. Publication cannot be after reception, more than 15 minutes old, or reverse accepted receive time. HTTPS URLs must declare aviationweather.gov, weather.gov or noaa.gov (or their subdomains), without credentials or a non-443 port.

response is null for review fallback, or a typed JevResponse with model, answers and usage. See fixtures/news.jsonl for a complete synthetic example. --jev accepts only null responses and generates the answer itself. Accepted content is journaled; input rejection leaves no news record. Repeated old batches can reject by reversed time before duplicate detection.

## Library ownership

Engine::new(config) constructs ephemeral state. Engine::apply(event) returns Result<Option<Fill>, String>; risk errors can still update rate/time/halt state. Read state through cash_micro(), realized_pnl_micro(), config() and liquidation_equity_micro(now).

Journal::open(path, optional_config) reconstructs durable state. Journal::apply(event) returns an outer storage Result and an inner execution Result. An outer error means storage failed and the event must not be treated as applied. is_replayed(index, event) supports contiguous prefix recovery. validate_news checks before an optional paid API call; ingest_news persists accepted item/response before installing its decision.

Callers must serialize access and handle both result layers. There is no promised thread-safe shared engine, network service or stable C/Dart ABI.
