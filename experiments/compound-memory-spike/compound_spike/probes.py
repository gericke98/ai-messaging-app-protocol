"""Probe-task generation for the M0 gate.

Probes derive from the user's RAW ingested messages — the same source both arm B
(raw excerpts) and arm C (distilled profile) draw on. Generating them from the
distilled profile would bias scoring toward arm C; from raw messages is neutral.
"""
import random
import re

from compound_spike.llm import complete
from compound_spike.models import Message
from config import PROBE_MODEL

PROBE_SYSTEM = (
    "You write realistic tasks a user might ask a brand-new AI assistant, based on a "
    "window of their past conversations. The task must be something where knowing the "
    "user well would visibly improve the answer. Reply with the task only — one or two "
    "sentences, no preamble, no quotes."
)


def normalize(task: str) -> str:
    return re.sub(r"[^a-z0-9]+", " ", task.lower()).strip()


def sample_windows(messages: list[Message], n_windows: int, window: int = 12,
                   seed: int = 0) -> list[str]:
    """Deterministically sample contiguous message windows, rendered as text."""
    if not messages:
        return []
    rng = random.Random(seed)
    starts = list(range(0, max(1, len(messages) - window + 1)))
    rng.shuffle(starts)
    out = []
    for s in starts[: n_windows]:
        chunk = messages[s: s + window]
        out.append("\n".join(f"{m.role}: {m.text}" for m in chunk))
    return out


def generate_probes(messages: list[Message], n: int, seed: int = 0,
                    extra: list[str] | None = None) -> list[str]:
    """Generate >= n unique probe tasks. `extra` (user-supplied real tasks) lead
    the list and count toward n. Raises ValueError if n unique probes can't be
    produced from the available windows."""
    probes: list[str] = []
    seen: set[str] = set()
    for task in (extra or []):
        key = normalize(task)
        if key and key not in seen:
            seen.add(key)
            probes.append(task.strip())

    # Over-sample windows: dedupe will eat some candidates.
    windows = sample_windows(messages, n_windows=max(n * 4, 16), seed=seed)
    for w in windows:
        if len(probes) >= n:
            break
        candidate = complete(model=PROBE_MODEL, system=PROBE_SYSTEM, user=w,
                             temperature=1.0).strip()
        key = normalize(candidate)
        if key and key not in seen:
            seen.add(key)
            probes.append(candidate)

    if len(probes) < n:
        raise ValueError(
            f"only generated {len(probes)}/{n} unique probes — "
            "need more source messages or extra tasks"
        )
    return probes[:n]
