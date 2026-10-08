from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum
from typing import Any


class Side(StrEnum):
    BUY = "BUY"
    SELL = "SELL"


class Metric(StrEnum):
    HIGH_TEMP = "high_temp"
    LOW_TEMP = "low_temp"
    PRECIP = "precip"
    OTHER = "other"


@dataclass(slots=True, frozen=True)
class WeatherTick:
    station: str
    temp_f: float | None
    precip_in: float | None
    obs_unix: int
    recv_ns: int
    raw: str
    source: str = "metar"
    daily_max_f: float | None = None
    daily_min_f: float | None = None


@dataclass(slots=True)
class Intent:
    token_id: str
    side: Side
    price: float
    size: float
    order_type: str
    reason: str
    edge: float
    fair: float
    city: str
    event_slug: str
    recv_ns: int
    extra: dict[str, Any] = field(default_factory=dict)
