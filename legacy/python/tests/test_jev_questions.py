from hft.jev.brain import extract_temp_candidates
from hft.jev.questions import weather_ingest_questions
from hft.polymarket.gamma import parse_event


def test_fan_out_includes_core_ids():
    q = weather_ingest_questions(["72", "21"])
    for key in (
        "is_weather_signal",
        "is_resolution_grade",
        "metric",
        "city",
        "should_trade",
        "action",
        "reported_temp_f",
    ):
        assert key in q


def test_extract_temps():
    text = "KLGA hourly high is now 72°F (22C) after 21/1351Z"
    c = extract_temp_candidates(text)
    assert "72" in c


def test_parse_gamma_event():
    raw = {
        "title": "Highest temperature in NYC on September 21?",
        "slug": "highest-temperature-in-nyc-on-september-21-2026",
        "description": "NOAA at LaGuardia ... timeseries?site=klga",
        "markets": [
            {
                "question": "Will the highest temperature in New York City be between 70-71°F on September 21?",
                "groupItemTitle": "70-71°F",
                "slug": "a",
                "conditionId": "0x1",
                "outcomes": '["Yes", "No"]',
                "clobTokenIds": '["111", "222"]',
                "orderPriceMinTickSize": 0.01,
                "orderMinSize": 5,
                "negRisk": True,
                "acceptingOrders": True,
                "closed": False,
                "bestBid": 0.6,
                "bestAsk": 0.64,
            },
            {
                "question": "Will the highest temperature in New York City be between 72-73°F on September 21?",
                "groupItemTitle": "72-73°F",
                "slug": "b",
                "conditionId": "0x2",
                "outcomes": '["Yes", "No"]',
                "clobTokenIds": '["333", "444"]',
                "orderPriceMinTickSize": 0.01,
                "orderMinSize": 5,
                "negRisk": True,
                "acceptingOrders": True,
                "closed": False,
                "bestBid": 0.2,
                "bestAsk": 0.24,
            },
        ],
    }
    ev = parse_event(raw, ("nyc",))
    assert ev is not None
    assert ev.station.icao == "KLGA"
    assert len(ev.markets) == 2
    assert ev.markets[0].yes_token == "111"
