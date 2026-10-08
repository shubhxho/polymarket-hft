from __future__ import annotations

from typesafe_sdk import Choice, Noul, NoulCriteria, Score

from hft.weather.stations import STATIONS

CITY_CRITERIA = {
    city: {
        "what": f"The observation or text is about {st.aliases[0]} / {st.icao}",
        "not_for": "A different city or station",
        "examples": list(st.aliases[:3]),
    }
    for city, st in STATIONS.items()
}
CITY_CRITERIA["other"] = {
    "what": "A city this desk does not trade",
    "not_for": "Any mapped weather city",
    "examples": ["Austin", "Madrid"],
}


def weather_ingest_questions(candidate_temps: list[str] | None = None) -> dict:
    """Speculative fan-out: every judgment the desk might need on one Jev call."""
    questions: dict = {
        "is_weather_signal": Noul(
            instructions={
                "question": "Does `text` contain a meteorological observation or forecast that could reprice a city temperature or precipitation market?",
                "inspect": "`text`",
            },
            criteria=NoulCriteria(
                true="Reports temperature, precipitation, or a forecast update for a specific place and time",
                false="Unrelated news, banter, or a market comment with no weather content",
            ),
        ),
        "is_resolution_grade": Noul(
            instructions={
                "question": "Is `text` an official station observation (METAR/ASOS/NOAA hourly) rather than a casual or city-center reading?",
                "inspect": "`text`",
            },
            criteria=NoulCriteria(
                true="METAR, ASOS, NOAA hourly, Weather.gov timeseries, or a named ICAO station",
                false="Anecdote, phone weather app, or unnamed location",
            ),
        ),
        "metric": Choice(
            instructions={
                "question": "Which weather contract family does `text` speak to?",
                "focus": "The measurable outcome, not the city.",
            },
            criteria={
                "high_temp": {
                    "what": "Daily highest temperature / afternoon max",
                    "not_for": "Overnight low or precipitation",
                    "examples": ["daily high 72F", "max temp 22C"],
                },
                "low_temp": {
                    "what": "Daily lowest temperature / overnight min",
                    "not_for": "Afternoon high or rain",
                    "examples": ["overnight low 41"],
                },
                "precip": {
                    "what": "Rain, snow, or measurable precipitation",
                    "not_for": "Temperature",
                    "examples": ["0.12 inches of rain", "measurable snow"],
                },
                "other": {
                    "what": "Wind, hurricane landfall, air quality, or none of the above",
                    "not_for": "A plain high/low/precip reading",
                    "examples": ["gusts to 40kt"],
                },
            },
        ),
        "city": Choice(
            instructions={
                "question": "Which traded city is `text` about?",
                "focus": "Prefer an ICAO/ASOS station over a metro name.",
            },
            criteria=CITY_CRITERIA,
        ),
        "material_update": Noul(
            instructions="Does this update the expected daily high/low or precipitation enough that a market maker should reprice immediately?",
            criteria=NoulCriteria(
                true="New daily max/min, a busted forecast, or a warning that changes the remaining-day distribution",
                false="Routine observation inside the already-known range",
            ),
        ),
        "should_trade": Noul(
            instructions={
                "question": "Should an automated desk act on this input right now, assuming the numeric fields in `parsed` are accurate?",
                "inspect": ["`text`", "`parsed`"],
                "focus": "Act only on resolution-relevant, timely, unambiguous weather.",
            },
            criteria=NoulCriteria(
                true="Clear observation or forecast for a mapped station on a live market day",
                false="Stale, ambiguous, wrong station, or commentary",
            ),
        ),
        "action": Choice(
            instructions="What should the trading engine do with this input?",
            criteria={
                "inject_observation": "Treat it as a new station reading and recompute fair value",
                "update_forecast": "Shift the forecast mean/sigma only",
                "ignore": "No trading action",
                "halt": "Kill switch — data looks poisoned or the market is resolving on something else",
            },
        ),
        "urgency": Score(
            instructions="How time-sensitive is this for an intraday temperature book?",
            criteria=[
                "Background / can wait for the next forecast cycle",
                "Useful this hour",
                "Tick-to-trade: a new extreme or a warning that other desks will see in seconds",
            ],
        ),
        "widens_uncertainty": Noul(
            instructions="Should remaining-day forecast sigma be widened (model disagreement, convective bust, missing obs)?",
        ),
    }
    if candidate_temps:
        options = {t: f"The reported temperature in Fahrenheit is {t}" for t in candidate_temps}
        options["none"] = "None of the candidate numbers is the observation"
        questions["reported_temp_f"] = Choice(
            instructions={
                "question": "Which candidate is the station temperature observation in Fahrenheit?",
                "candidates": candidate_temps,
                "focus": "Pick the reading, not a forecast or a time stamp.",
            },
            criteria=options,
        )
    return questions
