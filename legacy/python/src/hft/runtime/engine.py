from __future__ import annotations

import asyncio
import logging
from datetime import datetime, timezone

from hft.config import Settings
from hft.core.book import L2Book
from hft.core.clock import now_ns
from hft.core.types import WeatherTick
from hft.jev.brain import JevBrain, JevDecision
from hft.polymarket.clob_ws import ClobMarketWS
from hft.polymarket.gamma import WeatherEvent, discover_weather
from hft.polymarket.gateway import LiveGateway, PaperGateway
from hft.runtime.ingest_http import start_ingest_server
from hft.strategy.risk import RiskEngine
from hft.strategy.weather_hft import WeatherHFT
from hft.weather.feeds import StationDay, WeatherHub
from hft.weather.metar import parse_metar
from hft.weather.stations import STATIONS

log = logging.getLogger("hft.engine")


class Engine:
    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        self.events: list[WeatherEvent] = []
        self.books: dict[str, L2Book] = {}
        self.events_by_city: dict[str, list[WeatherEvent]] = {}
        self.strategy = WeatherHFT(settings)
        self.risk = RiskEngine(settings)
        self.gateway = PaperGateway() if settings.mode == "paper" or settings.dry_run else LiveGateway()
        self.jev = JevBrain(settings)
        self.hub: WeatherHub | None = None
        self._lock = asyncio.Lock()

    async def bootstrap(self) -> None:
        self.events = await discover_weather(
            gamma=self.settings.gamma,
            cities=self.settings.city_list,
        )
        self.events_by_city = {}
        self.books = {}
        for ev in self.events:
            self.events_by_city.setdefault(ev.city, []).append(ev)
            for m in ev.markets:
                self.books[m.yes_token] = L2Book(m.yes_token, tick=m.tick)
        log.info("discovered %d weather events, %d tokens", len(self.events), len(self.books))

    async def run(self) -> None:
        await self.bootstrap()
        self.hub = WeatherHub(self.settings, on_tick=self._on_weather)
        ws = ClobMarketWS(self.settings.clob_ws, self.books)
        loop = asyncio.get_running_loop()
        httpd = start_ingest_server(self.settings.ingest_port, loop, self.handle_ingest)
        try:
            async with asyncio.TaskGroup() as tg:
                tg.create_task(self.hub.run(), name="weather")
                tg.create_task(ws.run(), name="clob-ws")
                tg.create_task(self._heartbeat(), name="hb")
        finally:
            httpd.shutdown()
            await self.hub.aclose()
            await self.jev.aclose()

    async def _on_weather(self, day: StationDay, tick: WeatherTick, changed: bool) -> None:
        if not changed and tick.temp_f is None:
            return
        await self._trade_city(day.station.city, day)

    async def _trade_city(self, city: str, day: StationDay) -> None:
        async with self._lock:
            for event in self.events_by_city.get(city, []):
                intents = self.strategy.intents(
                    event, day, self.books, self.gateway.position
                )
                for intent in intents:
                    reason = self.risk.allow(intent, self.gateway.position(intent.token_id))
                    if reason:
                        log.debug("risk block %s %s", reason, intent.reason)
                        continue
                    fill = await self.gateway.execute(intent)
                    if fill:
                        self.risk.record(intent)

    async def handle_ingest(self, payload: dict) -> None:
        """Operator / webhook / stdin JSON. Jev gates unstructured text."""
        kind = payload.get("type") or payload.get("kind") or "text"
        if kind in {"metar", "obs", "tick"}:
            tick = None
            if payload.get("raw"):
                tick = parse_metar(str(payload["raw"]))
            if tick is None:
                icao = (payload.get("station") or payload.get("icao") or "").upper()
                temp = payload.get("temp_f")
                if icao and temp is not None:
                    tick = WeatherTick(
                        station=icao,
                        temp_f=float(temp),
                        precip_in=payload.get("precip_in"),
                        obs_unix=int(payload.get("obs_unix") or datetime.now(timezone.utc).timestamp()),
                        recv_ns=now_ns(),
                        raw=str(payload),
                        source="ingest",
                        daily_max_f=float(temp),
                    )
            if tick and self.hub:
                await self.hub.ingest_tick(tick)
            return
        text = payload.get("text") or payload.get("body") or payload.get("state") or ""
        if not text:
            return
        decision = await self.jev.judge(text, parsed=payload)
        if decision is None:
            log.info("ingest (no jev): %s", text[:160])
            return
        await self.apply_jev(decision, text)

    async def apply_jev(self, decision: JevDecision, text: str) -> None:
        log.info(
            "jev action=%s city=%s metric=%s trade=%.2f conf=%.2f temp=%s",
            decision.action,
            decision.city,
            decision.metric,
            decision.should_trade,
            decision.confidence,
            decision.temp_f,
        )
        if decision.action == "halt" or decision.confidence < 0.4:
            if decision.action == "halt":
                self.risk.halt("jev_halt")
            return
        yes = self.settings.noul_yes
        if decision.should_trade < yes or decision.is_weather < yes:
            return
        if decision.action == "ignore":
            return
        city = decision.city
        if city == "other" or city not in STATIONS:
            return
        if self.hub is None:
            return
        day = self.hub.days.get(city)
        if day is None:
            return
        if decision.action == "update_forecast" and decision.temp_f is not None:
            day.mu = decision.temp_f
            if decision.raw_answers.get("widens_uncertainty", {}).get("noul", 0) > 0.7:
                day.sigma = max(day.sigma, 3.5)
            await self._trade_city(city, day)
            return
        if decision.action == "inject_observation" and decision.temp_f is not None:
            if decision.is_resolution < self.settings.noul_yes:
                return
            tick = WeatherTick(
                station=day.station.icao,
                temp_f=decision.temp_f,
                precip_in=None,
                obs_unix=int(datetime.now(timezone.utc).timestamp()),
                recv_ns=now_ns(),
                raw=text,
                source="jev",
                daily_max_f=decision.temp_f,
            )
            await self.hub.ingest_tick(tick)

    async def _heartbeat(self) -> None:
        while True:
            await asyncio.sleep(15)
            log.info(
                "hb events=%d books=%d fills=%d halted=%s",
                len(self.events),
                len(self.books),
                len(self.gateway.fills),
                self.risk.state.halted,
            )
