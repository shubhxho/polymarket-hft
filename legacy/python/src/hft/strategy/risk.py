from __future__ import annotations

import time
from dataclasses import dataclass, field

from hft.config import Settings
from hft.core.types import Intent


@dataclass
class RiskState:
    realized_pnl: float = 0.0
    halted: bool = False
    halt_reason: str = ""
    order_times: list[float] = field(default_factory=list)
    city_notional: dict[str, float] = field(default_factory=dict)


class RiskEngine:
    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        self.state = RiskState()

    def halt(self, reason: str) -> None:
        self.state.halted = True
        self.state.halt_reason = reason

    def allow(self, intent: Intent, position: float) -> str | None:
        if self.state.halted:
            return f"halted:{self.state.halt_reason}"
        if self.state.realized_pnl <= -self.settings.max_daily_loss_usd:
            self.halt("daily_loss")
            return "daily_loss"
        now = time.monotonic()
        self.state.order_times = [t for t in self.state.order_times if now - t < 1.0]
        if len(self.state.order_times) >= self.settings.max_orders_per_sec:
            return "rate_limit"
        notional = intent.price * intent.size
        if notional > self.settings.max_order_usd + 1e-9:
            return "max_order"
        city_n = self.state.city_notional.get(intent.city, 0.0) + notional
        if city_n > self.settings.max_city_usd:
            return "max_city"
        if intent.size < 5:
            return "min_size"
        if position + intent.size > 500:
            return "max_position"
        return None

    def record(self, intent: Intent) -> None:
        self.state.order_times.append(time.monotonic())
        self.state.city_notional[intent.city] = (
            self.state.city_notional.get(intent.city, 0.0) + intent.price * intent.size
        )
