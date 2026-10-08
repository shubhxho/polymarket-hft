from hft.weather.metar import parse_metar
from hft.weather.stations import station_for_text, station_from_description


def test_parse_klga():
    raw = "KLGA 211351Z 31012KT 10SM FEW040 22/10 A3012"
    t = parse_metar(raw)
    assert t is not None
    assert t.station == "KLGA"
    assert t.temp_f == 72  # 22C


def test_negative_temp():
    raw = "KLGA 211351Z 31012KT 10SM FEW040 M04/M10 A3012"
    t = parse_metar(raw)
    assert t is not None
    assert t.temp_f == 25  # -4C


def test_station_from_noaa_url():
    desc = "highest reading ... available here: https://www.weather.gov/wrh/timeseries?site=klga"
    st = station_from_description(desc)
    assert st is not None
    assert st.icao == "KLGA"


def test_city_alias():
    st = station_for_text("Highest temperature in NYC on September 21?")
    assert st is not None
    assert st.city == "nyc"
