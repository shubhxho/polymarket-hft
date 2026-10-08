from __future__ import annotations

import time


def now_ns() -> int:
    return time.perf_counter_ns()


def unix_ns() -> int:
    return time.time_ns()
