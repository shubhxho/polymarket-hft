from __future__ import annotations

import asyncio
import json
import logging
import re
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone

import httpx
import orjson

from hft.core.types import Metric
from hft.weather.buckets import Bucket, parse_bucket
from hft.weather.stations import STATIONS, Station, station_for_text, station_from_description

log = logging.getLogger("hft.gamma")

UA = {"User-Agent": "Mozilla/5.0 polymarket-hft/0.1"}

_DATE = re.compile(r"on\s+([A-Za-z]+\s+\d{1,2})", re.I)


@dataclass(slots=True)
class WeatherMarket:
    question: str
    slug: str
    condition_id: str
    yes_token: str
    no_token: str
    tick: float
    min_size: float
    neg_risk: bool
    bucket: Bucket
    best_bid: float
    best_ask: float


@dataclass(slots=True)
class WeatherEvent:
    title: str
    slug: str
    description: str
    city: str
    station: Station
    metric: Metric
    event_date: str
    markets: list[WeatherMarket]


def _loads_maybe(value):
    if isinstance(value, str):
        try:
            return json.loads(value)
        except json.JSONDecodeError:
            return value
    return value


def _metric(title: str) -> Metric:
    t = title.lower()
    if "lowest temperature" in t or "low temperature" in t:
        return Metric.LOW_TEMP
    if "rain" in t or "precip" in t or "snow" in t:
        return Metric.PRECIP
    if "highest temperature" in t or "high temperature" in t or "temperature" in t:
        return Metric.HIGH_TEMP
    return Metric.OTHER


def _parse_market(raw: dict) -> WeatherMarket | None:
    if raw.get("closed") or not raw.get("acceptingOrders", True):
        return None
    outcomes = _loads_maybe(raw.get("outcomes")) or []
    tokens = _loads_maybe(raw.get("clobTokenIds")) or []
    if not isinstance(outcomes, list) or len(tokens) < 2:
        return None
    yes_i = 0
    for i, o in enumerate(outcomes):
        if str(o).lower() == "yes":
            yes_i = i
            break
    no_i = 1 - yes_i if len(tokens) > 1 else 1
    label = raw.get("groupItemTitle") or raw.get("question") or ""
    bucket = parse_bucket(label) or parse_bucket(raw.get("question") or "")
    if bucket is None:
        return None
    prices = _loads_maybe(raw.get("outcomePrices")) or []
    bid = float(raw.get("bestBid") or 0)
    ask = float(raw.get("bestAsk") or 1)
    if prices and len(prices) > yes_i:
        try:
            mid = float(prices[yes_i])
            if bid <= 0:
                bid = max(0.0, mid - 0.01)
            if ask >= 1:
                ask = min(1.0, mid + 0.01)
        except (TypeError, ValueError):
            pass
    return WeatherMarket(
        question=raw.get("question") or label,
        slug=raw.get("slug") or "",
        condition_id=raw.get("conditionId") or "",
        yes_token=str(tokens[yes_i]),
        no_token=str(tokens[no_i]),
        tick=float(raw.get("orderPriceMinTickSize") or 0.01),
        min_size=float(raw.get("orderMinSize") or 5),
        neg_risk=bool(raw.get("negRisk")),
        bucket=bucket,
        best_bid=bid,
        best_ask=ask,
    )


def parse_event(raw: dict, allowed_cities: tuple[str, ...]) -> WeatherEvent | None:
    title = raw.get("title") or ""
    desc = raw.get("description") or ""
    st = station_from_description(desc) or station_for_text(title)
    if st is None or (allowed_cities and st.city not in allowed_cities):
        return None
    markets: list[WeatherMarket] = []
    for m in raw.get("markets") or []:
        parsed = _parse_market(m)
        if parsed:
            markets.append(parsed)
    if len(markets) < 2:
        return None
    markets.sort(key=lambda x: (x.bucket.lo if x.bucket.lo != float("-inf") else -1e9))
    return WeatherEvent(
        title=title,
        slug=raw.get("slug") or "",
        description=desc,
        city=st.city,
        station=st,
        metric=_metric(title),
        event_date=_event_date(title, raw),
        markets=markets,
    )


def _event_date(title: str, raw: dict) -> str:
    end = raw.get("endDate") or raw.get("endDateIso")
    if end:
        return str(end)[:10]
    return title


async def discover_weather(
    *,
    gamma: str,
    cities: tuple[str, ...],
    client: httpx.AsyncClient | None = None,
) -> list[WeatherEvent]:
    own = client is None
    http = client or httpx.AsyncClient(headers=UA, timeout=20.0)
    queries = []
    today = datetime.now(timezone.utc).date()
    dates = [today + timedelta(days=i) for i in range(0, 3)]
    for city in cities:
        st = STATIONS.get(city)
        if not st:
            continue
        pretty = st.aliases[0]
        for d in dates:
            queries.append(f"highest temperature in {pretty} on {d.strftime('%B')} {d.day}")
            queries.append(f"lowest temperature in {pretty} on {d.strftime('%B')} {d.day}")
        queries.append(f"highest temperature in {pretty}")
    seen: dict[str, WeatherEvent] = {}
    sem = asyncio.Semaphore(8)

    async def one(q: str) -> None:
        async with sem:
            r = await http.get(f"{gamma}/public-search", params={"q": q, "limit": 12})
        if r.status_code >= 400:
            log.warning("gamma search %s -> %s", q, r.status_code)
            return
        payload = orjson.loads(r.content)
        for ev in payload.get("events") or []:
            parsed = parse_event(ev, cities)
            if parsed:
                seen[parsed.slug] = parsed

    try:
        await asyncio.gather(*(one(q) for q in queries))
    finally:
        if own:
            await http.aclose()
    return list(seen.values())
