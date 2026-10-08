# Polymarket weather HFT

Archived prototype. The supported engine and validation commands are now the Rust workspace at the project root. This prototype's live gateway is disabled under the Base-only policy; the historical live instructions below no longer enable execution.

Tick-to-trade desk for Polymarket daily temperature markets. METAR/ASOS updates recompute truncated fair value against the live CLOB. TypeSafe **Jev** sits on the unstructured path (NWS text, operator paste, webhooks) and never on the observation hot path.

## Layout

```
weather / METAR / stdin / POST /ingest
        │
        ├─ numeric obs ──► StationDay (daily max/min)
        │                      │
        │                      ▼
        │              truncated-high model ──► edge vs L2 book ──► FAK
        │
        └─ text ──► Jev (Choice/Noul/Score fan-out) ──► inject / forecast / ignore / halt
```

Code owns sizing, risk, and orders. Jev returns probabilities; thresholds live in `hft/config.py`.

## Setup

```bash
cp .env.example .env
# TYPESAFE_API_KEY from https://console.typesafe.ai/keys
# POLYMARKET_PRIVATE_KEY + POLYMARKET_WALLET_ADDRESS only for live

uv sync --extra dev
uv run pytest
uv run hft discover
uv run hft tick "KLGA 211351Z 31012KT 10SM FEW040 22/10 A3012" --mu 71 --sigma 1.8
```

Paper (default): CLOB websocket + METAR poll, no orders leave the box.

```bash
uv run hft paper
```

Push a signal:

```bash
curl -s localhost:8787/ingest -H 'content-type: application/json' \
  -d '{"type":"metar","station":"KLGA","temp_f":72}'

uv run hft ingest --text "KLGA 211551Z daily high now 73F on the NOAA hourly"
```

Live is opt-in: `HFT_MODE=live`, `HFT_DRY_RUN=false`, funded Polymarket wallet. Session starts in paper unless you do that.

## Jev questions (one `system_one` call)

`is_weather_signal`, `is_resolution_grade`, `metric`, `city`, `material_update`, `should_trade`, `action`, `urgency`, `widens_uncertainty`, plus a Choice over regex-extracted Fahrenheit candidates. Act only when `should_trade` and `is_weather_signal` are above `HFT_NOUL_YES` (default 0.8) and Choice confidence clears `HFT_MIN_CONFIDENCE`.

## Risk

Per-order and per-city notional caps, orders/sec limiter, daily loss halt, Jev `halt` kill switch. Minimum CLOB size is 5 shares.
