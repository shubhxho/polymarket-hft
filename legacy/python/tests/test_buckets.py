from hft.weather.buckets import parse_bucket


def test_range():
    b = parse_bucket("70-71°F")
    assert b is not None
    assert b.lo == 70 and b.hi == 71
    assert b.contains(70.0)
    assert not b.contains(72)


def test_or_below():
    b = parse_bucket("59°F or below")
    assert b is not None
    assert b.contains(-10)
    assert b.contains(59)
    assert not b.contains(60)


def test_or_higher():
    b = parse_bucket("78°F or higher")
    assert b is not None
    assert b.contains(78)
    assert b.contains(110)
    assert not b.contains(77)


def test_question_form():
    b = parse_bucket("Will the highest temperature in New York City be between 72-73°F on September 21?")
    assert b is not None
    assert (b.lo, b.hi) == (72, 73)
