from __future__ import annotations

from dataclasses import dataclass

import numpy as np

from hft.core.clock import now_ns

N_TICKS = 1001  # 0.000 .. 1.000 at 0.001


def _tick_index(price: float, tick: float) -> int:
    i = int(round(price / tick))
    return max(0, min(N_TICKS - 1, i))


@dataclass(slots=True)
class Top:
    bid: float
    ask: float
    bid_sz: float
    ask_sz: float
    ts_ns: int

    @property
    def mid(self) -> float:
        return 0.5 * (self.bid + self.ask)

    @property
    def spread(self) -> float:
        return self.ask - self.bid

    @property
    def microprice(self) -> float:
        den = self.bid_sz + self.ask_sz
        if den <= 0:
            return self.mid
        return (self.ask * self.bid_sz + self.bid * self.ask_sz) / den


class L2Book:
    """Tick-indexed L2 book. Hot path: O(1) top-of-book, numpy sizes."""

    __slots__ = (
        "token_id",
        "tick",
        "bids",
        "asks",
        "best_bid_i",
        "best_ask_i",
        "ts_ns",
        "hash",
    )

    def __init__(self, token_id: str, tick: float = 0.01) -> None:
        self.token_id = token_id
        self.tick = tick
        self.bids = np.zeros(N_TICKS, dtype=np.float64)
        self.asks = np.zeros(N_TICKS, dtype=np.float64)
        self.best_bid_i = -1
        self.best_ask_i = N_TICKS
        self.ts_ns = 0
        self.hash = ""

    def apply_snapshot(
        self,
        bids: list[tuple[float, float]] | list[dict],
        asks: list[tuple[float, float]] | list[dict],
        book_hash: str = "",
    ) -> None:
        self.bids.fill(0.0)
        self.asks.fill(0.0)
        for px, sz in _levels(bids):
            self.bids[_tick_index(px, self.tick)] = sz
        for px, sz in _levels(asks):
            self.asks[_tick_index(px, self.tick)] = sz
        self._recompute_best()
        self.hash = book_hash
        self.ts_ns = now_ns()

    def apply_level(self, price: float, size: float, side: str) -> None:
        i = _tick_index(price, self.tick)
        if side.upper() in {"BUY", "BID"}:
            self.bids[i] = size
            if size > 0 and i > self.best_bid_i:
                self.best_bid_i = i
            elif i == self.best_bid_i and size <= 0:
                self._recompute_best_bid()
        else:
            self.asks[i] = size
            if size > 0 and i < self.best_ask_i:
                self.best_ask_i = i
            elif i == self.best_ask_i and size <= 0:
                self._recompute_best_ask()
        self.ts_ns = now_ns()

    def top(self) -> Top | None:
        if self.best_bid_i < 0 or self.best_ask_i >= N_TICKS:
            return None
        bid = self.best_bid_i * self.tick
        ask = self.best_ask_i * self.tick
        return Top(
            bid=bid,
            ask=ask,
            bid_sz=float(self.bids[self.best_bid_i]),
            ask_sz=float(self.asks[self.best_ask_i]),
            ts_ns=self.ts_ns,
        )

    def ask_size_upto(self, price: float) -> float:
        hi = _tick_index(price, self.tick) + 1
        return float(self.asks[:hi].sum())

    def bid_size_from(self, price: float) -> float:
        lo = _tick_index(price, self.tick)
        return float(self.bids[lo:].sum())

    def _recompute_best(self) -> None:
        self._recompute_best_bid()
        self._recompute_best_ask()

    def _recompute_best_bid(self) -> None:
        nz = np.flatnonzero(self.bids)
        self.best_bid_i = int(nz[-1]) if nz.size else -1

    def _recompute_best_ask(self) -> None:
        nz = np.flatnonzero(self.asks)
        self.best_ask_i = int(nz[0]) if nz.size else N_TICKS


def _levels(raw: list) -> list[tuple[float, float]]:
    out: list[tuple[float, float]] = []
    for item in raw:
        if isinstance(item, dict):
            out.append((float(item["price"]), float(item["size"])))
        else:
            out.append((float(item[0]), float(item[1])))
    return out
