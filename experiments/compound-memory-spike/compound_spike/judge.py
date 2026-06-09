"""Judging: legacy human blind table + the M0 gate's LLM judge.

The gate judge scores all three arms side-by-side per probe (within-probe
calibration), blind to arm identity, against the PINNED rubric below. Changing
the rubric invalidates comparability across runs — bump RUBRIC_VERSION and
rerun all arms if you must edit it.
"""
import json
import random
import re

import pandas as pd

from compound_spike.llm import complete
from config import JUDGE_MODEL, JUDGE_TEMPERATURE

RUBRIC_VERSION = "v1-2026-06-09"

RUBRIC = """Score each answer 1-5 on how useful it is FOR THIS SPECIFIC USER:
5 - Immediately actionable; correctly reflects the user's context, preferences,
    projects, and constraints; nothing generic that contradicts what is known.
4 - Strongly personalized with minor generic filler or one small misread.
3 - Competent but mostly generic; would fit almost any user; asks for
    information a well-briefed assistant should already know.
2 - Generic AND partially off-target for this user, or padded with irrelevant
    material.
1 - Unhelpful, wrong about the user, or ignores the task.
Judge ONLY personalization-adjusted usefulness. Do not reward length. Do not
penalize an answer for being concise."""

JUDGE_SYSTEM = (
    "You are a strict, consistent evaluator. Apply the rubric exactly. "
    "Reply with a single JSON object mapping each label to an integer score 1-5, "
    'e.g. {"OUT-1": 3, "OUT-2": 5, "OUT-3": 1}. No other text.\n\n' + RUBRIC
)


def build_blind_table(results: list[dict], seed: int = 0):
    rng = random.Random(seed)
    shuffled = results[:]
    rng.shuffle(shuffled)
    rows, mapping = [], {}
    for i, r in enumerate(shuffled):
        label = f"OUT-{i+1:02d}"
        mapping[label] = {"consumer": r["consumer"], "arm": r["arm"],
                          "context_size": r.get("context_tokens", r.get("context_chars"))}
        rows.append({"label": label, "answer": r["answer"], "score": ""})
    df = pd.DataFrame(rows, columns=["label", "answer", "score"])
    return df, mapping


def _parse_scores(raw: str, labels: list[str]) -> dict[str, int]:
    text = raw.strip()
    fenced = re.search(r"```(?:json)?\s*(.*?)\s*```", text, re.DOTALL)
    if fenced:
        text = fenced.group(1)
    data = json.loads(text)
    if set(data.keys()) != set(labels):
        raise ValueError(f"judge returned labels {sorted(data)} != {sorted(labels)}")
    out = {}
    for label, score in data.items():
        s = int(score)
        if not 1 <= s <= 5:
            raise ValueError(f"score out of range for {label}: {score}")
        out[label] = s
    return out


def judge_probe(task: str, answers: dict[str, str], seed: int = 0) -> dict[str, int]:
    """Blind side-by-side scoring of one probe's answers. `answers` maps arm name
    to answer text; returns arm name -> 1-5 score. One retry on malformed output."""
    rng = random.Random(seed)
    arms = sorted(answers)
    rng.shuffle(arms)
    labels = [f"OUT-{i+1}" for i in range(len(arms))]
    label_to_arm = dict(zip(labels, arms))

    blocks = "\n\n".join(
        f"{label}: {answers[arm]}" for label, arm in label_to_arm.items()
    )
    user = f"TASK:\n{task}\n\nANSWERS:\n{blocks}"

    last_error = None
    for _ in range(2):
        raw = complete(model=JUDGE_MODEL, system=JUDGE_SYSTEM, user=user,
                       temperature=JUDGE_TEMPERATURE)
        try:
            by_label = _parse_scores(raw, labels)
            return {label_to_arm[label]: score for label, score in by_label.items()}
        except (ValueError, json.JSONDecodeError) as e:
            last_error = e
    raise ValueError(f"judge output unparseable after retry: {last_error}")
