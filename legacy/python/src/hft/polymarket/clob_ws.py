from __future__ import annotations

import asyncio
import logging
from collections.abc import Callable

import orjson
from websockets.asyncio.client import connect

from hft.core.book import L2Book

log = logging.getLogger("hft.ws")


class ClobMarketWS:
    def __init__(
        self,
        url: str,
        books: dict[str, L2Book],
        on_top: Callable[[str, L2Book], None] | None = None,
    ) -> None:
        self.url = url
        self.books = books
        self.on_top = on_top
        self._stop = asyncio.Event()

    def stop(self) -> None:
        self._stop.set()

    async def run(self) -> None:
        token_ids = list(self.books)
        if not token_ids:
            return
        while not self._stop.is_set():
            try:
                async with connect(
                    self.url,
                    ping_interval=10,
                    ping_timeout=10,
                    max_size=2**22,
                    compression=None,
                ) as ws:
                    await ws.send(
                        orjson.dumps(
                            {
                                "type": "market",
                                "assets_ids": token_ids,
                                "custom_feature_enabled": True,
                            }
                        )
                    )
                    async for raw in ws:
                        if self._stop.is_set():
                            return
                        if raw == "PONG" or raw == b"PONG":
                            continue
                        self._on_message(raw)
            except Exception:
                log.exception("clob ws reconnect")
                await asyncio.sleep(1.0)

    def _on_message(self, raw: str | bytes) -> None:
        try:
            msg = orjson.loads(raw)
        except Exception:
            return
        if isinstance(msg, list):
            for item in msg:
                if isinstance(item, dict):
                    self._handle(item)
            return
        if isinstance(msg, dict):
            self._handle(msg)

    def _handle(self, msg: dict) -> None:
        et = msg.get("event_type") or msg.get("type")
        if et == "book":
            token = msg.get("asset_id") or msg.get("assetId")
            book = self.books.get(token)
            if not book:
                return
            book.tick = float(msg.get("tick_size") or book.tick)
            book.apply_snapshot(msg.get("bids") or [], msg.get("asks") or [], msg.get("hash") or "")
            if self.on_top:
                self.on_top(token, book)
        elif et == "price_change":
            for ch in msg.get("price_changes") or []:
                token = ch.get("asset_id") or msg.get("asset_id")
                book = self.books.get(token)
                if not book:
                    continue
                book.apply_level(
                    float(ch["price"]),
                    float(ch.get("size") or 0),
                    str(ch.get("side") or "BUY"),
                )
                if self.on_top:
                    self.on_top(token, book)
        elif et == "best_bid_ask":
            token = msg.get("asset_id")
            book = self.books.get(token)
            if not book:
                return
            bid = msg.get("best_bid")
            ask = msg.get("best_ask")
            if bid is not None:
                book.apply_level(float(bid), float(msg.get("best_bid_size") or 1), "BUY")
            if ask is not None:
                book.apply_level(float(ask), float(msg.get("best_ask_size") or 1), "SELL")
            if self.on_top:
                self.on_top(token, book)
