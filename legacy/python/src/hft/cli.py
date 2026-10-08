from __future__ import annotations

import argparse
import asyncio
import json
import logging
import os
import sys

import orjson

from hft.config import settings
from hft.jev.brain import JevBrain
from hft.polymarket.gamma import discover_weather
from hft.runtime.engine import Engine
from hft.weather.fair import bucket_probabilities
from hft.weather.metar import parse_metar


def _log() -> None:
    logging.basicConfig(
        level=os.environ.get("HFT_LOG", "INFO"),
        format="%(asctime)s %(levelname)s %(name)s %(message)s",
    )
    try:
        import uvloop

        uvloop.install()
    except ImportError:
        pass


def main(argv: list[str] | None = None) -> None:
    _log()
    p = argparse.ArgumentParser(prog="hft", description="Polymarket weather HFT")
    sub = p.add_subparsers(dest="cmd", required=True)

    sub.add_parser("discover", help="list live weather events")
    run = sub.add_parser("paper", help="run paper trading")
    live = sub.add_parser("live", help="run live (requires keys + HFT_DRY_RUN=false)")
    ing = sub.add_parser("ingest", help="push unstructured weather text through Jev")
    ing.add_argument("--text", help="raw text")
    ing.add_argument("--file", help="jsonl or plaintext file")
    tick = sub.add_parser("tick", help="price a METAR against discovered books (offline math)")
    tick.add_argument("metar")
    tick.add_argument("--mu", type=float, default=70.0)
    tick.add_argument("--sigma", type=float, default=2.2)

    args = p.parse_args(argv)
    if args.cmd == "discover":
        asyncio.run(_discover())
    elif args.cmd == "paper":
        s = settings().model_copy(update={"mode": "paper"})
        asyncio.run(Engine(s).run())
    elif args.cmd == "live":
        s = settings().model_copy(update={"mode": "live"})
        asyncio.run(Engine(s).run())
    elif args.cmd == "ingest":
        asyncio.run(_ingest(args.text, args.file))
    elif args.cmd == "tick":
        _tick(args.metar, args.mu, args.sigma)


async def _discover() -> None:
    s = settings()
    events = await discover_weather(gamma=s.gamma, cities=s.city_list)
    for e in events:
        print(f"{e.slug}  {e.city}/{e.station.icao}  {e.metric}  {len(e.markets)} buckets")
        for m in e.markets:
            print(
                f"  {m.bucket.label:18} book {m.best_bid:.3f}/{m.best_ask:.3f}  {m.yes_token[:12]}…"
            )
    if not events:
        print("no live weather events for configured cities", file=sys.stderr)


async def _ingest(text: str | None, path: str | None) -> None:
    s = settings()
    brain = JevBrain(s)
    chunks: list[str] = []
    if text:
        chunks.append(text)
    if path:
        raw = open(path, encoding="utf-8").read()
        if path.endswith(".jsonl"):
            for line in raw.splitlines():
                if line.strip():
                    obj = json.loads(line)
                    chunks.append(obj.get("text") or obj.get("body") or line)
        else:
            chunks.append(raw)
    if not chunks and not sys.stdin.isatty():
        chunks.append(sys.stdin.read())
    if not chunks:
        print("pass --text, --file, or stdin", file=sys.stderr)
        sys.exit(2)
    try:
        for c in chunks:
            d = await brain.judge(c)
            if d is None:
                print(orjson.dumps({"error": "no_typesafe_key", "text": c[:200]}).decode())
                continue
            print(
                orjson.dumps(
                    {
                        "action": d.action,
                        "city": d.city,
                        "metric": d.metric,
                        "should_trade": d.should_trade,
                        "confidence": d.confidence,
                        "temp_f": d.temp_f,
                        "urgency": d.urgency,
                    }
                ).decode()
            )
    finally:
        await brain.aclose()


def _tick(metar: str, mu: float, sigma: float) -> None:
    t = parse_metar(metar)
    if t is None:
        print("could not parse METAR", file=sys.stderr)
        sys.exit(2)
    from hft.weather.buckets import Bucket

    buckets = [
        Bucket("<=65", float("-inf"), 65),
        Bucket("66-67", 66, 67),
        Bucket("68-69", 68, 69),
        Bucket("70-71", 70, 71),
        Bucket("72-73", 72, 73),
        Bucket(">=74", 74, float("inf")),
    ]
    probs = bucket_probabilities(buckets, obs_max_f=t.temp_f, mu=mu, sigma=sigma, remaining_hours=6)
    print(f"{t.station} temp={t.temp_f}F  forecast μ={mu} σ={sigma}")
    for b, p in zip(buckets, probs, strict=True):
        print(f"  {b.label:8}  {p:6.3f}")


if __name__ == "__main__":
    main()
