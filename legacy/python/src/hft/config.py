from __future__ import annotations

import os
from functools import lru_cache
from typing import Literal

from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_file=".env",
        env_prefix="HFT_",
        extra="ignore",
    )

    mode: Literal["paper", "live"] = "paper"
    cities: str = "nyc,london,tokyo,hong_kong,los_angeles,paris"
    dry_run: bool = True
    min_edge: float = 0.04
    kelly_fraction: float = 0.25
    max_order_usd: float = 25.0
    max_city_usd: float = 150.0
    max_daily_loss_usd: float = 75.0
    max_orders_per_sec: float = 8.0
    min_confidence: float = 0.75
    noul_yes: float = 0.8
    noul_no: float = 0.2
    metar_poll_ms: int = 8000
    forecast_poll_s: int = 180
    ingest_port: int = 8787
    taker_fee_bps: float = 0.0
    clob_ws: str = "wss://ws-subscriptions-clob.polymarket.com/ws/market"
    clob_rest: str = "https://clob.polymarket.com"
    gamma: str = "https://gamma-api.polymarket.com"
    aviationweather: str = "https://aviationweather.gov/api/data/metar"
    open_meteo: str = "https://api.open-meteo.com/v1/forecast"

    @property
    def city_list(self) -> tuple[str, ...]:
        return tuple(c.strip().lower() for c in self.cities.split(",") if c.strip())

    @property
    def live_enabled(self) -> bool:
        return self.mode == "live" and not self.dry_run

    @property
    def typesafe_model(self) -> str:
        return os.environ.get("TYPESAFE_DEFAULT_MODEL", "jev-latest")


@lru_cache(maxsize=1)
def settings() -> Settings:
    return Settings()
