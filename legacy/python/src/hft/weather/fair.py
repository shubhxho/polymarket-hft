from __future__ import annotations

import math

from hft.weather.buckets import Bucket

_SQRT2 = math.sqrt(2.0)


def _cdf(x: float, mu: float, sigma: float) -> float:
    if sigma <= 1e-9:
        return 1.0 if x >= mu else 0.0
    return 0.5 * (1.0 + math.erf((x - mu) / (sigma * _SQRT2)))


def remaining_sigma(base_sigma: float, remaining_hours: float) -> float:
    """Intraday: as the day dies, the unresolved tail of the daily high shrinks."""
    h = max(0.0, remaining_hours)
    # 14h of convective heating ~ full forecast sigma; 0h -> tiny
    scale = min(1.0, h / 14.0)
    return max(0.15, base_sigma * (0.18 + 0.82 * scale))


def bucket_probabilities(
    buckets: list[Bucket],
    *,
    obs_max_f: float | None,
    mu: float,
    sigma: float,
    remaining_hours: float,
) -> list[float]:
    """P(final daily high in bucket) given observed max so far.

    Final high = max(obs_max, X) where X ~ N(mu, sigma_t) is the would-be
    unconstrained daily high. Lower buckets already crossed by obs_max
    are zeroed — this is the METAR tick edge.
    """
    if not buckets:
        return []
    sig = remaining_sigma(sigma, remaining_hours)
    w = min(1.0, max(0.0, remaining_hours) / 14.0)
    if obs_max_f is not None:
        mu = (1.0 - w) * obs_max_f + w * mu
        # cannot print a high below what has already printed
        mu = max(mu, obs_max_f)
    floor = obs_max_f if obs_max_f is not None else float("-inf")
    raw = [0.0] * len(buckets)
    for i, b in enumerate(buckets):
        if b.hi < floor:
            raw[i] = 0.0
            continue
        if b.contains(floor) or (b.lo <= floor <= b.hi):
            # stays here unless X exceeds the bucket high
            if math.isinf(b.hi):
                raw[i] = 1.0
            else:
                raw[i] = _cdf(b.hi + 0.5, mu, sig)  # integer F bins, +0.5 continuity
            continue
        # bucket strictly above the observed floor
        lo = b.lo - 0.5
        hi = b.hi + 0.5 if not math.isinf(b.hi) else float("inf")
        p_hi = 1.0 if math.isinf(hi) else _cdf(hi, mu, sig)
        p_lo = _cdf(lo, mu, sig)
        raw[i] = max(0.0, p_hi - p_lo)

    s = sum(raw)
    if s <= 1e-12:
        # degenerate: pin on the bucket that currently contains obs
        out = [0.0] * len(buckets)
        if obs_max_f is not None:
            for i, b in enumerate(buckets):
                if b.contains(obs_max_f):
                    out[i] = 1.0
                    return out
        out[len(buckets) // 2] = 1.0
        return out
    return [p / s for p in raw]
