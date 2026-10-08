from __future__ import annotations

import math
from dataclasses import dataclass

from hft.config import Settings
from hft.core.book import L2Book
from hft.core.clock import now_ns
from hft.core.types import Intent, Metric, Side
from hft.polymarket.gamma import WeatherEvent
from hft.weather.fair import bucket_probabilities
from hft.weather.feeds import StationDay


@dataclass(slots=True)
class Quote:
    token_id: str
    fair: float
    bid: float
    ask: float
    edge_buy: float
    edge_sell: float
    label: str


def _round_tick(px: float, tick: float) -> float:
    n = round(px / tick)
    return max(tick, min(1.0 - tick, n * tick))


class WeatherHFT:
    """Deterministic METAR → book. Jev never called here."""

    def __init__(self, settings: Settings) -> None:
        self.settings = settings

    def quotes(
        self,
        event: WeatherEvent,
        day: StationDay,
        books: dict[str, L2Book],
    ) -> list[Quote]:
        if event.metric not in {Metric.HIGH_TEMP, Metric.LOW_TEMP}:
            return []
        obs = day.max_f if event.metric == Metric.HIGH_TEMP else day.min_f
        mu = day.mu
        if event.metric == Metric.LOW_TEMP and day.min_f is not None:
            mu = min(day.mu, day.min_f)
        probs = bucket_probabilities(
            [m.bucket for m in event.markets],
            obs_max_f=obs if event.metric == Metric.HIGH_TEMP else None,
            mu=mu,
            sigma=day.sigma,
            remaining_hours=day.remaining_hours(),
        )
        out: list[Quote] = []
        for mkt, fair in zip(event.markets, probs, strict=True):
            book = books.get(mkt.yes_token)
            top = book.top() if book else None
            bid = top.bid if top else mkt.best_bid
            ask = top.ask if top else mkt.best_ask
            fee = self.settings.taker_fee_bps / 1e4
            edge_buy = fair - ask - fee
            edge_sell = bid - fair - fee
            out.append(
                Quote(
                    token_id=mkt.yes_token,
                    fair=fair,
                    bid=bid,
                    ask=ask,
                    edge_buy=edge_buy,
                    edge_sell=edge_sell,
                    label=mkt.bucket.label,
                )
            )
        return out

    def intents(
        self,
        event: WeatherEvent,
        day: StationDay,
        books: dict[str, L2Book],
        position_of,
    ) -> list[Intent]:
        quotes = self.quotes(event, day, books)
        if not quotes:
            return []
        min_edge = self.settings.min_edge
        best = max(quotes, key=lambda q: q.edge_buy)
        intents: list[Intent] = []
        recv = now_ns()
        mkt_by_token = {m.yes_token: m for m in event.markets}
        if best.edge_buy >= min_edge:
            mkt = mkt_by_token[best.token_id]
            book = books.get(best.token_id)
            depth = book.ask_size_upto(best.ask) if book else 20.0
            size = self._size(best.fair, best.ask, best.edge_buy, depth, mkt.min_size)
            if size >= mkt.min_size:
                intents.append(
                    Intent(
                        token_id=best.token_id,
                        side=Side.BUY,
                        price=_round_tick(best.ask, mkt.tick),
                        size=size,
                        order_type="FAK",
                        reason=f"buy {best.label} fair={best.fair:.3f} ask={best.ask:.3f}",
                        edge=best.edge_buy,
                        fair=best.fair,
                        city=event.city,
                        event_slug=event.slug,
                        recv_ns=recv,
                    )
                )
        # fade buckets that observations have already killed
        for q in quotes:
            if q.edge_sell < min_edge:
                continue
            pos = position_of(q.token_id)
            if pos <= 0:
                continue
            mkt = mkt_by_token[q.token_id]
            size = min(pos, max(mkt.min_size, math.floor(pos)))
            if size < mkt.min_size:
                continue
            intents.append(
                Intent(
                    token_id=q.token_id,
                    side=Side.SELL,
                    price=_round_tick(q.bid, mkt.tick),
                    size=float(size),
                    order_type="FAK",
                    reason=f"exit {q.label} fair={q.fair:.3f} bid={q.bid:.3f}",
                    edge=q.edge_sell,
                    fair=q.fair,
                    city=event.city,
                    event_slug=event.slug,
                    recv_ns=recv,
                )
            )
        return intents

    def _size(self, fair: float, ask: float, edge: float, depth: float, min_size: float) -> float:
        # fractional Kelly on a binary, bankroll = max_order_usd
        p = min(0.99, max(0.01, fair))
        b = (1.0 / ask) - 1.0 if ask > 0 else 0.0
        if b <= 0:
            return 0.0
        kelly = (p * (b + 1) - 1) / b
        k = max(0.0, kelly) * self.settings.kelly_fraction
        usd = min(self.settings.max_order_usd, k * self.settings.max_city_usd)
        shares = usd / max(ask, 0.01)
        shares = min(shares, depth * 0.25)
        if shares < min_size:
            if usd >= min_size * ask and edge >= self.settings.min_edge + 0.02:
                shares = min_size
            else:
                return 0.0
        return float(math.floor(shares * 100) / 100.0)
