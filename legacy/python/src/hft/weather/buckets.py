from __future__ import annotations

import math
import re
from dataclasses import dataclass

_OR_BELOW = re.compile(
    r"(?:be\s+)?(?P<h>-?\d+)\s*°?\s*[Ff]?\s+or\s+(?:below|lower|colder|less)",
    re.I,
)
_OR_ABOVE = re.compile(
    r"(?:be\s+)?(?P<l>-?\d+)\s*°?\s*[Ff]?\s+or\s+(?:higher|above|hotter|more|greater)",
    re.I,
)
_RANGE = re.compile(
    r"(?P<l>-?\d+)\s*[-–]\s*(?P<h>-?\d+)\s*°?\s*[Ff]?",
    re.I,
)
_SINGLE = re.compile(r"(?P<v>-?\d+)\s*°?\s*[Ff]", re.I)


@dataclass(frozen=True, slots=True)
class Bucket:
    label: str
    lo: float
    hi: float

    def contains(self, temp_f: float) -> bool:
        return self.lo <= temp_f <= self.hi

    @property
    def mid(self) -> float:
        if math.isinf(self.lo) and math.isinf(self.hi):
            return 0.0
        if math.isinf(self.lo):
            return self.hi
        if math.isinf(self.hi):
            return self.lo
        return 0.5 * (self.lo + self.hi)


def parse_bucket(label: str) -> Bucket | None:
    text = (label or "").replace("°", " ")
    m = _OR_BELOW.search(text)
    if m:
        h = float(m.group("h"))
        return Bucket(label=label.strip(), lo=float("-inf"), hi=h)
    m = _OR_ABOVE.search(text)
    if m:
        lo = float(m.group("l"))
        return Bucket(label=label.strip(), lo=lo, hi=float("inf"))
    m = _RANGE.search(text)
    if m:
        return Bucket(label=label.strip(), lo=float(m.group("l")), hi=float(m.group("h")))
    m = _SINGLE.search(text)
    if m:
        v = float(m.group("v"))
        return Bucket(label=label.strip(), lo=v, hi=v)
    return None
