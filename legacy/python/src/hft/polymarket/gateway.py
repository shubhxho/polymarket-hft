from __future__ import annotations

import logging
from dataclasses import dataclass

from hft.core.types import Intent, Side

log = logging.getLogger("hft.exec")


@dataclass
class Fill:
    token_id: str
    side: str
    price: float
    size: float
    order_id: str
    paper: bool


class PaperGateway:
    def __init__(self) -> None:
        self.fills: list[Fill] = []
        self.positions: dict[str, float] = {}
        self.cash_usd: float = 0.0
        self.notional_by_city: dict[str, float] = {}

    async def execute(self, intent: Intent) -> Fill | None:
        signed = intent.size if intent.side == Side.BUY else -intent.size
        self.positions[intent.token_id] = self.positions.get(intent.token_id, 0.0) + signed
        notional = intent.price * intent.size
        self.cash_usd -= notional if intent.side == Side.BUY else -notional
        self.notional_by_city[intent.city] = self.notional_by_city.get(intent.city, 0.0) + notional
        fill = Fill(
            token_id=intent.token_id,
            side=intent.side,
            price=intent.price,
            size=intent.size,
            order_id=f"paper-{len(self.fills)+1}",
            paper=True,
        )
        self.fills.append(fill)
        log.info(
            "PAPER %s %s sz=%.2f px=%.3f edge=%.3f %s",
            intent.side,
            intent.token_id[:10],
            intent.size,
            intent.price,
            intent.edge,
            intent.reason,
        )
        return fill

    def position(self, token_id: str) -> float:
        return self.positions.get(token_id, 0.0)


class LiveGateway:
    def __init__(self) -> None:
        self._client = None
        self.positions: dict[str, float] = {}
        self.notional_by_city: dict[str, float] = {}
        self.cash_usd: float = 0.0
        self.fills: list[Fill] = []

    async def connect(self) -> None:
        raise RuntimeError(
            "Base-only policy: live Polymarket settlement is unsupported. "
            "Use the Rust paper replay engine."
        )

    async def execute(self, intent: Intent) -> Fill | None:
        if self._client is None:
            await self.connect()
        client = self._client
        amount = f"{intent.price * intent.size:.2f}"
        if hasattr(client, "place_market_order"):
            resp = await client.place_market_order(
                token_id=intent.token_id,
                side=intent.side.value,
                amount=amount,
            )
        else:
            raise RuntimeError("polymarket client missing place_market_order")
        ok = getattr(resp, "ok", True)
        if ok is False:
            log.error("live order rejected %s", getattr(resp, "message", resp))
            return None
        order_id = getattr(resp, "order_id", None) or getattr(resp, "orderId", "") or ""
        signed = intent.size if intent.side == Side.BUY else -intent.size
        self.positions[intent.token_id] = self.positions.get(intent.token_id, 0.0) + signed
        fill = Fill(
            token_id=intent.token_id,
            side=intent.side,
            price=intent.price,
            size=intent.size,
            order_id=str(order_id),
            paper=False,
        )
        self.fills.append(fill)
        return fill

    def position(self, token_id: str) -> float:
        return self.positions.get(token_id, 0.0)


OrderGateway = PaperGateway | LiveGateway
