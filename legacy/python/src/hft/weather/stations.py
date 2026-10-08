from __future__ import annotations

import re
from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class Station:
    city: str
    icao: str
    tz: str
    lat: float
    lon: float
    aliases: tuple[str, ...]


STATIONS: dict[str, Station] = {
    "nyc": Station("nyc", "KLGA", "America/New_York", 40.777, -73.872, ("nyc", "new york", "new york city", "laguardia")),
    "los_angeles": Station("los_angeles", "KLAX", "America/Los_Angeles", 33.942, -118.408, ("los angeles", "la", "lax")),
    "chicago": Station("chicago", "KORD", "America/Chicago", 41.974, -87.907, ("chicago", "ord")),
    "miami": Station("miami", "KMIA", "America/New_York", 25.795, -80.287, ("miami",)),
    "london": Station("london", "EGLC", "Europe/London", 51.505, 0.055, ("london",)),
    "paris": Station("paris", "LFPB", "Europe/Paris", 48.969, 2.441, ("paris", "le bourget")),
    "tokyo": Station("tokyo", "RJTT", "Asia/Tokyo", 35.553, 139.781, ("tokyo", "haneda")),
    "hong_kong": Station("hong_kong", "VHHH", "Asia/Hong_Kong", 22.308, 113.918, ("hong kong", "hk")),
    "beijing": Station("beijing", "ZBAA", "Asia/Shanghai", 40.080, 116.584, ("beijing",)),
    "shanghai": Station("shanghai", "ZSPD", "Asia/Shanghai", 31.143, 121.805, ("shanghai",)),
    "seoul": Station("seoul", "RKSI", "Asia/Seoul", 37.460, 126.440, ("seoul", "incheon")),
    "taipei": Station("taipei", "RCTP", "Asia/Taipei", 25.078, 121.233, ("taipei",)),
    "wuhan": Station("wuhan", "ZHHH", "Asia/Shanghai", 30.783, 114.208, ("wuhan",)),
    "guangzhou": Station("guangzhou", "ZGGG", "Asia/Shanghai", 23.392, 113.299, ("guangzhou",)),
}

_ICAO_RE = re.compile(r"site=([A-Za-z0-9]{4})", re.I)
_BY_ICAO = {s.icao: s for s in STATIONS.values()}
_BY_ALIAS: dict[str, Station] = {}
for _s in STATIONS.values():
    _BY_ALIAS[_s.city] = _s
    for a in _s.aliases:
        _BY_ALIAS[a.lower()] = _s


def station_for_icao(icao: str) -> Station | None:
    return _BY_ICAO.get(icao.upper())


def station_from_description(text: str) -> Station | None:
    m = _ICAO_RE.search(text or "")
    if m:
        return station_for_icao(m.group(1))
    return None


def station_for_text(text: str) -> Station | None:
    low = (text or "").lower()
    hit = station_from_description(low)
    if hit:
        return hit
    # longest alias first so "new york city" beats "york"
    for alias, st in sorted(_BY_ALIAS.items(), key=lambda kv: -len(kv[0])):
        if alias in low:
            return st
    return None
