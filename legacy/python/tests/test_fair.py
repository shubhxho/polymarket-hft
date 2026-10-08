from hft.weather.buckets import Bucket
from hft.weather.fair import bucket_probabilities


BUCKETS = [
    Bucket("<=69", float("-inf"), 69),
    Bucket("70-71", 70, 71),
    Bucket("72-73", 72, 73),
    Bucket(">=74", 74, float("inf")),
]


def test_obs_kills_lower_buckets():
    p = bucket_probabilities(BUCKETS, obs_max_f=72, mu=71, sigma=2.0, remaining_hours=4)
    assert p[0] == 0.0
    assert p[1] == 0.0
    assert p[2] > 0.5
    assert abs(sum(p) - 1.0) < 1e-9


def test_end_of_day_pins():
    p = bucket_probabilities(BUCKETS, obs_max_f=70, mu=80, sigma=3.0, remaining_hours=0.05)
    # tiny remaining time: mass stays on current bucket
    assert p[1] > 0.6


def test_open_day_follows_forecast():
    p = bucket_probabilities(BUCKETS, obs_max_f=None, mu=72.5, sigma=1.2, remaining_hours=14)
    assert p[2] == max(p)
