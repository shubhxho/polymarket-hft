from __future__ import annotations

import asyncio
import logging
from collections.abc import Awaitable, Callable
from datetime import datetime, timezone
from zoneinfo import ZoneInfo

import httpx
import orjson

from hft.config import Settings
from hft.core.types import WeatherTick
from hft.weather.metar import from_aviationweather
from hft.weather.stations import STATIONS, Station

log = logging.getLogger("hft.weather")

UA = {"User-Agent": "polymarket-hft/0.1 (weather signal desk)"}


class StationDay:
    __slots__ = ("station", "local_date", "max_f", "min_f", "last", "mu", "sigma")

    def __init__(self, station: Station) -> None:
        self.station = station
        self.local_date = ""
        self.max_f: float | None = None
        self.min_f: float | None = None
        self.last: WeatherTick | None = None
        self.mu: float = 70.0
        self.sigma: float = 2.4

    def remaining_hours(self, now: datetime | None = None) -> float:
        tz = ZoneInfo(self.station.tz)
        n = now.astimezone(tz) if now else datetime.now(tz)
        end = n.replace(hour=23, minute=59, second=59, microsecond=0)
        return max(0.0, (end - n).total_seconds() / 3600.0)

    def apply(self, tick: WeatherTick) -> bool:
        """Return True if daily max/min changed (hot-path trigger)."""
        tz = ZoneInfo(self.station.tz)
        local = datetime.fromtimestamp(tick.obs_unix, tz=timezone.utc).astimezone(tz)
        day = local.date().isoformat()
        if day != self.local_date:
            self.local_date = day
            self.max_f = None
            self.min_f = None
        changed = False
        if tick.temp_f is not None:
            if self.max_f is None or tick.temp_f > self.max_f:
                self.max_f = tick.temp_f
                changed = True
            if self.min_f is None or tick.temp_f < self.min_f:
                self.min_f = tick.temp_f
                changed = True
        self.last = tick
        return changed


class WeatherHub:
    def __init__(
        self,
        settings: Settings,
        on_tick: Callable[[StationDay, WeatherTick, bool], Awaitable[None]],
    ) -> None:
        self.settings = settings
        self.on_tick = on_tick
        self.days = {c: StationDay(STATIONS[c]) for c in settings.city_list if c in STATIONS}
        self._http = httpx.AsyncClient(headers=UA, timeout=8.0, http2=False)

    async def aclose(self) -> None:
        await self._http.aclose()

    async def run(self) -> None:
        await asyncio.gather(self._metar_loop(), self._forecast_loop())

    async def ingest_tick(self, tick: WeatherTick) -> None:
        st = None
        for day in self.days.values():
            if day.station.icao == tick.station:
                st = day
                break
        if st is None:
            return
        changed = st.apply(tick)
        await self.on_tick(st, tick, changed)

    async def _metar_loop(self) -> None:
        ids = ",".join(d.station.icao for d in self.days.values())
        interval = max(1.0, self.settings.metar_poll_ms / 1000.0)
        while True:
            try:
                r = await self._http.get(
                    self.settings.aviationweather,
                    params={"ids": ids, "format": "json", "hours": 1},
                )
                r.raise_for_status()
                rows = orjson.loads(r.content)
                if isinstance(rows, dict):
                    rows = rows.get("features") or rows.get("data") or []
                for row in rows:
                    tick = from_aviationweather(row if isinstance(row, dict) else {})
                    if tick:
                        await self.ingest_tick(tick)
            except Exception:
                log.exception("metar poll failed")
            await asyncio.sleep(interval)

    async def _forecast_loop(self) -> None:
        while True:
            for day in self.days.values():
                try:
                    await self._refresh_forecast(day)
                except Exception:
                    log.exception("forecast failed %s", day.station.city)
            await asyncio.sleep(self.settings.forecast_poll_s)

    async def _refresh_forecast(self, day: StationDay) -> None:
        st = day.station
        r = await self._http.get(
            self.settings.open_meteo,
            params={
                "latitude": st.lat,
                "longitude": st.lon,
                "daily": "temperature_2m_max",
                "temperature_unit": "fahrenheit",
                "forecast_days": 1,
                "timezone": st.tz,
            },
        )
        r.raise_for_status()
        data = orjson.loads(r.content)
        daily = data.get("daily") or {}
        highs = daily.get("temperature_2m_max") or []
        if highs:
            day.mu = float(highs[0])
            # Open-Meteo single run: keep a conservative sigma; ensemble can replace this.
            day.sigma = max(1.4, abs(day.mu) * 0.03 + 1.2)
