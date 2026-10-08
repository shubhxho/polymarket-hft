from __future__ import annotations

import re
from datetime import datetime, timezone

from hft.core.clock import now_ns
from hft.core.types import WeatherTick

_TEMP = re.compile(r"\s(M?\d{2})/(M?\d{2})\s")
_ICAO = re.compile(r"^([A-Z]{4})\s")


def temp_c_to_f(c: float) -> float:
    return c * 9.0 / 5.0 + 32.0


def _decode_c(token: str) -> float:
    if token.startswith("M"):
        return -float(token[1:])
    return float(token)


def parse_metar(raw: str, *, obs_unix: int | None = None, source: str = "metar") -> WeatherTick | None:
    text = (raw or "").strip()
    if not text:
        return None
    icao_m = _ICAO.search(text)
    if not icao_m:
        return None
    temp_f = None
    tm = _TEMP.search(f" {text} ")
    if tm:
        temp_f = round(temp_c_to_f(_decode_c(tm.group(1))))
    return WeatherTick(
        station=icao_m.group(1),
        temp_f=temp_f,
        precip_in=None,
        obs_unix=obs_unix if obs_unix is not None else int(datetime.now(timezone.utc).timestamp()),
        recv_ns=now_ns(),
        raw=text,
        source=source,
        daily_max_f=temp_f,
        daily_min_f=temp_f,
    )


def from_aviationweather(row: dict) -> WeatherTick | None:
    icao = (row.get("icaoId") or row.get("station") or "").upper()
    if not icao:
        return None
    temp_c = row.get("temp")
    temp_f = round(temp_c_to_f(float(temp_c))) if temp_c is not None else None
    obs = row.get("obsTime") or row.get("reportTime")
    if isinstance(obs, str):
        try:
            obs_unix = int(datetime.fromisoformat(obs.replace("Z", "+00:00")).timestamp())
        except ValueError:
            obs_unix = int(datetime.now(timezone.utc).timestamp())
    elif obs is not None:
        obs_unix = int(obs)
    else:
        obs_unix = int(datetime.now(timezone.utc).timestamp())
    raw = row.get("rawOb") or row.get("raw") or ""
    parsed = parse_metar(raw, obs_unix=obs_unix) if raw else None
    if parsed and parsed.temp_f is not None:
        temp_f = parsed.temp_f
    return WeatherTick(
        station=icao,
        temp_f=temp_f,
        precip_in=None,
        obs_unix=obs_unix,
        recv_ns=now_ns(),
        raw=raw or icao,
        source="aviationweather",
        daily_max_f=temp_f,
        daily_min_f=temp_f,
    )
