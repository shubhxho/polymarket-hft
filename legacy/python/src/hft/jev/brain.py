from __future__ import annotations

import logging
import os
import re
from dataclasses import dataclass
from typing import Any

from hft.config import Settings
from hft.jev.questions import weather_ingest_questions

log = logging.getLogger("hft.jev")

_NUM = re.compile(
    r"(?<![\d.])(-?\d{1,3}(?:\.\d+)?)\s*(?:°\s*)?([FfCc])?\b"
)


@dataclass(slots=True)
class JevDecision:
    action: str
    city: str
    metric: str
    should_trade: float
    is_weather: float
    is_resolution: float
    material: float
    urgency: float
    confidence: float
    temp_f: float | None
    raw_answers: dict[str, Any]


def extract_temp_candidates(text: str) -> list[str]:
    found: list[str] = []
    for m in _NUM.finditer(text or ""):
        val = float(m.group(1))
        unit = (m.group(2) or "F").upper()
        if unit == "C":
            val = val * 9.0 / 5.0 + 32.0
        if -80 <= val <= 140:
            token = str(int(round(val)))
            if token not in found:
                found.append(token)
        if len(found) >= 8:
            break
    return found


class JevBrain:
    """Semantic path. Never sits on the METAR hot path."""

    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        self._client = None
        self.enabled = bool(os.environ.get("TYPESAFE_API_KEY"))

    async def _client_ctx(self):
        from typesafe_sdk import AsyncTypeSafeClient

        if self._client is None:
            self._client = AsyncTypeSafeClient(model=self.settings.typesafe_model)
        return self._client

    async def aclose(self) -> None:
        if self._client is not None:
            await self._client.aclose()
            self._client = None

    async def judge(self, text: str, parsed: dict | None = None) -> JevDecision | None:
        if not self.enabled:
            log.warning("TYPESAFE_API_KEY unset; skipping Jev")
            return None
        candidates = extract_temp_candidates(text)
        questions = weather_ingest_questions(candidates)
        client = await self._client_ctx()
        state = {
            "text": text,
            "parsed": parsed or {},
            "candidates_f": candidates,
        }
        result = await client.system_one(
            state=state,
            questions=questions,
            model=self.settings.typesafe_model,
        )
        answers = result.answers
        action = answers["action"]
        city = answers["city"]
        metric = answers["metric"]
        temp_f = None
        if "reported_temp_f" in answers and answers["reported_temp_f"].choice != "none":
            try:
                temp_f = float(answers["reported_temp_f"].choice)
            except ValueError:
                temp_f = None
        confs = [
            getattr(answers[k], "confidence", 1.0)
            for k in ("action", "city", "metric")
            if k in answers
        ]
        return JevDecision(
            action=action.choice,
            city=city.choice,
            metric=metric.choice,
            should_trade=answers["should_trade"].noul,
            is_weather=answers["is_weather_signal"].noul,
            is_resolution=answers["is_resolution_grade"].noul,
            material=answers["material_update"].noul,
            urgency=float(answers["urgency"].score),
            confidence=min(confs) if confs else 0.0,
            temp_f=temp_f,
            raw_answers={k: _plain(v) for k, v in answers.items()},
        )


def _plain(ans: Any) -> dict:
    out: dict[str, Any] = {"type": getattr(ans, "type", None)}
    for attr in ("noul", "choice", "score", "confidence", "probabilities"):
        if hasattr(ans, attr):
            val = getattr(ans, attr)
            out[attr] = val
    return out
