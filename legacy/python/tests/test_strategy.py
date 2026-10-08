from hft.config import Settings
from hft.core.book import L2Book
from hft.core.types import Metric, Side
from hft.polymarket.gamma import WeatherEvent, WeatherMarket
from hft.strategy.weather_hft import WeatherHFT
from hft.weather.buckets import Bucket
from hft.weather.feeds import StationDay
from hft.weather.stations import STATIONS


def _event() -> WeatherEvent:
    mkts = []
    for label, lo, hi, yes in [
        ("68-69°F", 68, 69, "t68"),
        ("70-71°F", 70, 71, "t70"),
        ("72-73°F", 72, 73, "t72"),
    ]:
        mkts.append(
            WeatherMarket(
                question=label,
                slug=label,
                condition_id="c",
                yes_token=yes,
                no_token=yes + "n",
                tick=0.01,
                min_size=5,
                neg_risk=True,
                bucket=Bucket(label, lo, hi),
                best_bid=0.20,
                best_ask=0.80,
            )
        )
    return WeatherEvent(
        title="Highest temperature in NYC",
        slug="nyc-high",
        description="site=klga",
        city="nyc",
        station=STATIONS["nyc"],
        metric=Metric.HIGH_TEMP,
        event_date="2026-09-21",
        markets=mkts,
    )


def test_buys_underpriced_live_bucket():
    ev = _event()
    day = StationDay(STATIONS["nyc"])
    day.max_f = 72
    day.mu = 72
    day.sigma = 0.6
    books = {}
    for m in ev.markets:
        b = L2Book(m.yes_token, tick=0.01)
        if m.yes_token == "t72":
            b.apply_snapshot(bids=[[0.40, 50]], asks=[[0.45, 80]])
        else:
            b.apply_snapshot(bids=[[0.05, 50]], asks=[[0.10, 80]])
        books[m.yes_token] = b
    strat = WeatherHFT(Settings(min_edge=0.04, max_order_usd=40, kelly_fraction=0.5))
    intents = strat.intents(ev, day, books, lambda _t: 0.0)
    assert intents
    buy = [i for i in intents if i.side == Side.BUY]
    assert buy and buy[0].token_id == "t72"
    assert buy[0].edge > 0
