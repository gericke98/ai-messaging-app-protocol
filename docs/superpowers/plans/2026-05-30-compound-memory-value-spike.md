# Compound Memory Value Spike — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a throwaway Python harness that proves or kills the hypothesis that a distilled cross-agent compound-memory profile makes a fresh agent visibly more useful on a real task than no memory or raw transcript excerpts, at equal-or-smaller token budget.

**Architecture:** A small CLI pipeline — `ingest` (parse ChatGPT + Claude exports → normalized message log) → `distill` (Claude Opus 4.8 map-reduce → compound profile JSON) → `harness` (run one task through 2 consumer models × 3 arms {cold, raw, compound} under token-budget parity) → `judge` (blind side-by-side table + human rubric). Deterministic logic (parsing, reconciliation, token gating, injection rendering, blind table) is unit-tested with TDD; non-deterministic LLM calls sit behind a thin interface, mocked in tests and exercised in a real run.

**Tech Stack:** Python 3.11+, `litellm` (unified LLM calls), `tiktoken` (single shared tokenizer for the parity gate), `pandas` (blind results table), `pytest`. No eval framework. Models: Claude Opus 4.8 distills; GPT-5.5 + Gemini 3.1 Pro consume.

---

## File Structure

```
experiments/compound-memory-spike/
├── README.md                      # how to run; throwaway notice
├── requirements.txt               # pinned deps
├── .gitignore                     # data/, out/, .env, __pycache__
├── config.py                      # model names, budget N, paths (single source of truth)
├── compound_spike/
│   ├── __init__.py
│   ├── models.py                  # Message, Provenance, Fact, Profile dataclasses + JSON (de)serialize
│   ├── tokens.py                  # TokenCounter (tiktoken) + budget-parity gate
│   ├── ingest.py                  # ChatGPT + Claude export parsers → list[Message]
│   ├── reconcile.py               # pure ADD/UPDATE/SUPERSEDE recency-wins reconciliation
│   ├── llm.py                     # thin litellm wrapper: complete(model, system, user, temp)
│   ├── distill.py                 # chunk → extract (LLM) → reconcile → Profile
│   ├── inject.py                  # render_profile() + select_raw_excerpts() under budget
│   ├── harness.py                 # run task × consumer × arm; save outputs w/ hidden arm ids
│   ├── judge.py                   # blind randomized table + score capture + unblind
│   └── cli.py                     # argparse entrypoints: ingest|distill|run|judge
└── tests/
    ├── __init__.py
    ├── fixtures/
    │   ├── chatgpt_export_sample.json
    │   └── claude_export_sample.json
    ├── test_models.py
    ├── test_tokens.py
    ├── test_ingest.py
    ├── test_reconcile.py
    ├── test_distill.py
    ├── test_inject.py
    ├── test_harness.py
    └── test_judge.py
```

Responsibilities: `models.py` owns the data shapes and is imported everywhere; `reconcile.py` is pure logic reused by `distill.py`; `llm.py` is the ONLY module that touches network, so everything else is testable without keys.

---

## Task 1: Project scaffold + branch

**Files:**
- Create: `experiments/compound-memory-spike/requirements.txt`
- Create: `experiments/compound-memory-spike/.gitignore`
- Create: `experiments/compound-memory-spike/config.py`
- Create: `experiments/compound-memory-spike/README.md`
- Create: `experiments/compound-memory-spike/compound_spike/__init__.py`
- Create: `experiments/compound-memory-spike/tests/__init__.py`

- [ ] **Step 1: Create the working branch**

Run:
```bash
git checkout -b spike/compound-memory-value
```
Expected: `Switched to a new branch 'spike/compound-memory-value'`

- [ ] **Step 2: Write `requirements.txt`**

```
litellm==1.55.10
tiktoken==0.8.0
pandas==2.2.3
pytest==8.3.4
python-dotenv==1.0.1
```

- [ ] **Step 3: Write `.gitignore`**

```
data/
out/
.env
__pycache__/
*.pyc
.pytest_cache/
```

- [ ] **Step 4: Write `config.py`**

```python
"""Single source of truth for the spike's tunables."""
from pathlib import Path

ROOT = Path(__file__).parent
DATA_DIR = ROOT / "data"          # user's real exports (gitignored)
OUT_DIR = ROOT / "out"            # results (gitignored)

# Models — confirm availability when wiring keys (see README).
DISTILL_MODEL = "anthropic/claude-opus-4-8"
CONSUMER_MODELS = ["openai/gpt-5.5", "gemini/gemini-3.1-pro"]

# Budget N: shared context-token ceiling for arms B and C (counted with TOKENIZER).
# Arm B fills up to N; arm C must use <= N.
BUDGET_N = 3500

# Single shared tokenizer for the parity gate. Parity only needs B and C counted
# with the SAME counter, so absolute per-vendor accuracy is irrelevant.
TOKENIZER = "o200k_base"

DISTILL_CHUNK_TOKENS = 6000       # window size for map-reduce extraction
EXTRACT_TEMPERATURE = 0.0
CONSUME_TEMPERATURE = 0.3
```

- [ ] **Step 5: Write `README.md`**

````markdown
# Compound Memory Value Spike (THROWAWAY)

Tests whether a distilled cross-agent memory profile beats no-memory and raw-transcript
arms on a real task, at equal token budget. See
`docs/superpowers/specs/2026-05-30-compound-memory-value-spike-design.md`.

## Setup
```bash
cd experiments/compound-memory-spike
python -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
cp .env.example .env   # then fill in keys
```

`.env` needs: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`.
Confirm the model ids in `config.py` are live before running.

## Run
```bash
# 1. drop ChatGPT + Claude exports into data/
python -m compound_spike.cli ingest   --chatgpt data/chatgpt.json --claude data/claude.json
python -m compound_spike.cli distill                      # -> out/profile.json
python -m compound_spike.cli run      --task "<your real task>"   # -> out/results.json
python -m compound_spike.cli judge                        # -> out/blind_table.csv + scoring
```

## Tests
```bash
pytest -v
```
````

- [ ] **Step 6: Create empty package markers**

```bash
mkdir -p experiments/compound-memory-spike/compound_spike experiments/compound-memory-spike/tests/fixtures
touch experiments/compound-memory-spike/compound_spike/__init__.py experiments/compound-memory-spike/tests/__init__.py
```

- [ ] **Step 7: Commit**

```bash
git add experiments/compound-memory-spike
git commit -m "chore(spike): scaffold compound-memory value spike"
```

---

## Task 2: Data models (`models.py`)

**Files:**
- Create: `experiments/compound-memory-spike/compound_spike/models.py`
- Test: `experiments/compound-memory-spike/tests/test_models.py`

- [ ] **Step 1: Write the failing test**

```python
# tests/test_models.py
from compound_spike.models import Message, Fact, Provenance, Profile

def test_profile_roundtrips_through_json():
    profile = Profile(
        subject="user",
        facts=[
            Fact(
                id="f_1",
                category="preferences",
                statement="Prefers Rust over Go",
                status="active",
                superseded_by=None,
                first_seen="2025-11-02",
                last_seen="2026-04-18",
                provenance=[Provenance(source="claude", conv_id="c1", msg_id="m1", ts="2026-04-18")],
            )
        ],
    )
    restored = Profile.from_dict(profile.to_dict())
    assert restored == profile
    assert restored.facts[0].provenance[0].source == "claude"

def test_message_is_hashable_dataclass():
    m = Message(source="chatgpt", conv_id="c1", msg_id="m1", role="user", text="hi", ts="2026-01-01")
    assert m.text == "hi"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/test_models.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'compound_spike.models'`

- [ ] **Step 3: Write minimal implementation**

```python
# compound_spike/models.py
from __future__ import annotations
from dataclasses import dataclass, asdict, field

@dataclass
class Message:
    source: str        # "chatgpt" | "claude"
    conv_id: str
    msg_id: str
    role: str          # "user" | "assistant"
    text: str
    ts: str            # ISO-8601

@dataclass
class Provenance:
    source: str
    conv_id: str
    msg_id: str
    ts: str

@dataclass
class Fact:
    id: str
    category: str      # identity|preferences|projects|decisions|patterns|relationships
    statement: str
    status: str = "active"          # active | superseded
    superseded_by: str | None = None
    first_seen: str = ""
    last_seen: str = ""
    provenance: list[Provenance] = field(default_factory=list)

    def to_dict(self) -> dict:
        d = asdict(self)
        return d

    @classmethod
    def from_dict(cls, d: dict) -> "Fact":
        prov = [Provenance(**p) for p in d.get("provenance", [])]
        return cls(
            id=d["id"], category=d["category"], statement=d["statement"],
            status=d.get("status", "active"), superseded_by=d.get("superseded_by"),
            first_seen=d.get("first_seen", ""), last_seen=d.get("last_seen", ""),
            provenance=prov,
        )

@dataclass
class Profile:
    subject: str
    facts: list[Fact] = field(default_factory=list)

    def to_dict(self) -> dict:
        return {"subject": self.subject, "facts": [f.to_dict() for f in self.facts]}

    @classmethod
    def from_dict(cls, d: dict) -> "Profile":
        return cls(subject=d["subject"], facts=[Fact.from_dict(f) for f in d.get("facts", [])])

    def active(self) -> list[Fact]:
        return [f for f in self.facts if f.status == "active"]
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/test_models.py -v`
Expected: PASS (2 passed)

- [ ] **Step 5: Commit**

```bash
git add compound_spike/models.py tests/test_models.py
git commit -m "feat(spike): add core data models with JSON roundtrip"
```

---

## Task 3: Token counter + budget gate (`tokens.py`)

**Files:**
- Create: `experiments/compound-memory-spike/compound_spike/tokens.py`
- Test: `experiments/compound-memory-spike/tests/test_tokens.py`

- [ ] **Step 1: Write the failing test**

```python
# tests/test_tokens.py
from compound_spike.tokens import TokenCounter, assert_budget_parity
import pytest

def test_counter_counts_more_tokens_for_longer_text():
    c = TokenCounter()
    assert c.count("hello world") < c.count("hello world " * 50)

def test_parity_passes_when_compound_not_larger_than_raw():
    c = TokenCounter()
    raw = "word " * 200
    compound = "word " * 100
    # should not raise
    assert_budget_parity(raw=raw, compound=compound, counter=c)

def test_parity_fails_when_compound_exceeds_raw():
    c = TokenCounter()
    raw = "word " * 100
    compound = "word " * 200
    with pytest.raises(ValueError, match="budget parity violated"):
        assert_budget_parity(raw=raw, compound=compound, counter=c)
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/test_tokens.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'compound_spike.tokens'`

- [ ] **Step 3: Write minimal implementation**

```python
# compound_spike/tokens.py
import tiktoken
from config import TOKENIZER

class TokenCounter:
    """One shared tokenizer. Parity only needs B and C counted the SAME way."""
    def __init__(self, encoding_name: str = TOKENIZER):
        self._enc = tiktoken.get_encoding(encoding_name)

    def count(self, text: str) -> int:
        return len(self._enc.encode(text or ""))

def assert_budget_parity(raw: str, compound: str, counter: TokenCounter) -> None:
    r, c = counter.count(raw), counter.count(compound)
    if c > r:
        raise ValueError(
            f"budget parity violated: compound={c} tokens > raw={r} tokens"
        )

def fit_to_budget(text: str, budget: int, counter: TokenCounter) -> str:
    """Truncate text to <= budget tokens (token-accurate)."""
    enc = counter._enc
    toks = enc.encode(text or "")
    if len(toks) <= budget:
        return text
    return enc.decode(toks[:budget])
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/test_tokens.py -v`
Expected: PASS (3 passed)

- [ ] **Step 5: Commit**

```bash
git add compound_spike/tokens.py tests/test_tokens.py
git commit -m "feat(spike): token counter + budget-parity gate"
```

---

## Task 4: Test fixtures (export samples)

**Files:**
- Create: `experiments/compound-memory-spike/tests/fixtures/chatgpt_export_sample.json`
- Create: `experiments/compound-memory-spike/tests/fixtures/claude_export_sample.json`

- [ ] **Step 1: Write `chatgpt_export_sample.json`** (mirrors real ChatGPT `conversations.json`: tree `mapping`, a null-message root node, epoch `create_time`)

```json
[
  {
    "title": "Backend language choice",
    "create_time": 1730500000.0,
    "mapping": {
      "root": { "id": "root", "message": null, "parent": null, "children": ["n1"] },
      "n1": {
        "id": "n1",
        "message": {
          "id": "msg_a",
          "author": { "role": "user" },
          "create_time": 1730500001.0,
          "content": { "content_type": "text", "parts": ["I'm rewriting our backend. I prefer Rust over Go for it."] }
        },
        "parent": "root",
        "children": ["n2"]
      },
      "n2": {
        "id": "n2",
        "message": {
          "id": "msg_b",
          "author": { "role": "assistant" },
          "create_time": 1730500002.0,
          "content": { "content_type": "text", "parts": ["Rust is a solid choice for that."] }
        },
        "parent": "n1",
        "children": []
      }
    }
  }
]
```

- [ ] **Step 2: Write `claude_export_sample.json`** (mirrors real Claude `conversations.json`: `chat_messages`, one with legacy `text`, one with newer `content` array, ISO `created_at`)

```json
[
  {
    "uuid": "conv_x",
    "name": "Weekend project",
    "created_at": "2026-04-18T10:00:00Z",
    "chat_messages": [
      {
        "uuid": "cm1",
        "sender": "human",
        "text": "My side project is a SwiftUI habit tracker called Streaks.",
        "created_at": "2026-04-18T10:00:00Z"
      },
      {
        "uuid": "cm2",
        "sender": "assistant",
        "text": "",
        "content": [{ "type": "text", "text": "Nice — SwiftUI is great for that." }],
        "created_at": "2026-04-18T10:00:05Z"
      }
    ]
  }
]
```

- [ ] **Step 3: Commit**

```bash
git add tests/fixtures/chatgpt_export_sample.json tests/fixtures/claude_export_sample.json
git commit -m "test(spike): add ChatGPT + Claude export fixtures"
```

---

## Task 5: Ingest parsers (`ingest.py`)

**Files:**
- Create: `experiments/compound-memory-spike/compound_spike/ingest.py`
- Test: `experiments/compound-memory-spike/tests/test_ingest.py`

- [ ] **Step 1: Write the failing test**

```python
# tests/test_ingest.py
import json
from pathlib import Path
from compound_spike.ingest import parse_chatgpt, parse_claude, load_all

FIX = Path(__file__).parent / "fixtures"

def test_parse_chatgpt_extracts_user_and_assistant_in_order():
    data = json.loads((FIX / "chatgpt_export_sample.json").read_text())
    msgs = parse_chatgpt(data)
    assert [m.role for m in msgs] == ["user", "assistant"]
    assert msgs[0].source == "chatgpt"
    assert "Rust over Go" in msgs[0].text
    assert msgs[0].ts.startswith("2024-")  # epoch 1730500001 -> 2024-11
    assert msgs[0].conv_id and msgs[0].msg_id == "msg_a"

def test_parse_claude_handles_text_and_content_array():
    data = json.loads((FIX / "claude_export_sample.json").read_text())
    msgs = parse_claude(data)
    assert [m.role for m in msgs] == ["user", "assistant"]
    assert "Streaks" in msgs[0].text
    assert msgs[1].text == "Nice — SwiftUI is great for that."
    assert msgs[0].source == "claude"

def test_load_all_merges_both_sources(tmp_path):
    cg = FIX / "chatgpt_export_sample.json"
    cl = FIX / "claude_export_sample.json"
    msgs = load_all(chatgpt_path=cg, claude_path=cl)
    sources = {m.source for m in msgs}
    assert sources == {"chatgpt", "claude"}
    assert len(msgs) == 4
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/test_ingest.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'compound_spike.ingest'`

- [ ] **Step 3: Write minimal implementation**

```python
# compound_spike/ingest.py
import json
from datetime import datetime, timezone
from pathlib import Path
from compound_spike.models import Message

def _epoch_to_iso(epoch: float | None) -> str:
    if not epoch:
        return ""
    return datetime.fromtimestamp(epoch, tz=timezone.utc).isoformat()

def parse_chatgpt(data: list[dict]) -> list[Message]:
    out: list[Message] = []
    for conv in data:
        conv_id = conv.get("title", "") + str(conv.get("create_time", ""))
        nodes = conv.get("mapping", {})
        # order by message create_time; skip null-message (root) nodes
        items = []
        for node in nodes.values():
            msg = node.get("message")
            if not msg:
                continue
            role = (msg.get("author") or {}).get("role")
            if role not in ("user", "assistant"):
                continue
            parts = (msg.get("content") or {}).get("parts") or []
            text = "\n".join(p for p in parts if isinstance(p, str)).strip()
            if not text:
                continue
            items.append((msg.get("create_time") or 0.0, msg, role, text))
        items.sort(key=lambda t: t[0])
        for ct, msg, role, text in items:
            out.append(Message(
                source="chatgpt", conv_id=conv_id, msg_id=msg.get("id", ""),
                role=role, text=text, ts=_epoch_to_iso(ct),
            ))
    return out

def _claude_text(m: dict) -> str:
    if (m.get("text") or "").strip():
        return m["text"].strip()
    blocks = m.get("content") or []
    return "\n".join(b.get("text", "") for b in blocks if b.get("type") == "text").strip()

def parse_claude(data: list[dict]) -> list[Message]:
    out: list[Message] = []
    for conv in data:
        conv_id = conv.get("uuid", "")
        for m in conv.get("chat_messages", []):
            role = {"human": "user", "assistant": "assistant"}.get(m.get("sender", ""))
            if role is None:
                continue
            text = _claude_text(m)
            if not text:
                continue
            out.append(Message(
                source="claude", conv_id=conv_id, msg_id=m.get("uuid", ""),
                role=role, text=text, ts=m.get("created_at", ""),
            ))
    return out

def load_all(chatgpt_path: Path | None = None, claude_path: Path | None = None) -> list[Message]:
    msgs: list[Message] = []
    if chatgpt_path:
        msgs += parse_chatgpt(json.loads(Path(chatgpt_path).read_text()))
    if claude_path:
        msgs += parse_claude(json.loads(Path(claude_path).read_text()))
    return msgs
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/test_ingest.py -v`
Expected: PASS (3 passed)

- [ ] **Step 5: Commit**

```bash
git add compound_spike/ingest.py tests/test_ingest.py
git commit -m "feat(spike): ingest ChatGPT + Claude exports to normalized log"
```

---

## Task 6: Reconciliation logic (`reconcile.py`)

**Files:**
- Create: `experiments/compound-memory-spike/compound_spike/reconcile.py`
- Test: `experiments/compound-memory-spike/tests/test_reconcile.py`

Reconcile a flat list of candidate `Fact`s into a profile. Facts are matched by
`(category, normalized statement-key)`; contradictions on the same key resolve recency-wins:
the older fact is marked `superseded` (kept, not deleted) and linked via `superseded_by`.

- [ ] **Step 1: Write the failing test**

```python
# tests/test_reconcile.py
from compound_spike.models import Fact, Provenance
from compound_spike.reconcile import reconcile

def _fact(fid, statement, last_seen, key=None, category="preferences"):
    return Fact(id=fid, category=category, statement=statement,
                first_seen=last_seen, last_seen=last_seen,
                provenance=[Provenance("claude", "c", fid, last_seen)])

def test_duplicate_facts_merge_provenance_and_extend_last_seen():
    a = _fact("f1", "Prefers Rust", "2025-11-01")
    b = _fact("f2", "Prefers Rust", "2026-04-01")
    profile = reconcile([a, b], conflict_keys={"f1": "lang", "f2": "lang"})
    active = profile.active()
    assert len(active) == 1
    assert active[0].last_seen == "2026-04-01"
    assert len(active[0].provenance) == 2

def test_contradiction_supersedes_older_keeps_both():
    a = _fact("f1", "Lives in Madrid", "2025-01-01", category="identity")
    b = _fact("f2", "Lives in Lisbon", "2026-01-01", category="identity")
    profile = reconcile([a, b], conflict_keys={"f1": "city", "f2": "city"})
    active = profile.active()
    assert len(active) == 1 and active[0].statement == "Lives in Lisbon"
    superseded = [f for f in profile.facts if f.status == "superseded"]
    assert len(superseded) == 1 and superseded[0].superseded_by == "f2"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/test_reconcile.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'compound_spike.reconcile'`

- [ ] **Step 3: Write minimal implementation**

```python
# compound_spike/reconcile.py
from compound_spike.models import Fact, Profile

def _norm(s: str) -> str:
    return " ".join(s.lower().split())

def reconcile(facts: list[Fact], conflict_keys: dict[str, str]) -> Profile:
    """conflict_keys maps fact.id -> a semantic key. Facts sharing a (category, key)
    are reconciled: identical statements merge; differing statements => recency wins,
    older marked superseded. Facts with no shared key are kept as-is.
    Order-independent: we sort by last_seen so the newest wins deterministically."""
    ordered = sorted(facts, key=lambda f: f.last_seen)
    winners: dict[tuple[str, str], Fact] = {}
    result: list[Fact] = []

    for f in ordered:
        key = (f.category, conflict_keys.get(f.id, _norm(f.statement)))
        if key not in winners:
            winners[key] = f
            result.append(f)
            continue
        prev = winners[key]
        if _norm(prev.statement) == _norm(f.statement):
            # duplicate: merge provenance into the kept fact, extend last_seen
            prev.provenance.extend(f.provenance)
            prev.last_seen = max(prev.last_seen, f.last_seen)
            prev.first_seen = min(prev.first_seen, f.first_seen)
        else:
            # contradiction: newer (f) wins, older (prev) superseded but kept
            prev.status = "superseded"
            prev.superseded_by = f.id
            winners[key] = f
            result.append(f)
    return Profile(subject="user", facts=result + [
        f for f in ordered if f.status == "superseded" and f not in result
    ])
```

> Note: superseded facts are mutated in place inside `result`; the trailing comprehension
> is a safety net for any superseded fact not already in `result` (none in practice, but
> keeps the invariant "all facts retained"). `active()` filters to `status == "active"`.

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/test_reconcile.py -v`
Expected: PASS (2 passed)

- [ ] **Step 5: Commit**

```bash
git add compound_spike/reconcile.py tests/test_reconcile.py
git commit -m "feat(spike): recency-wins reconciliation with supersede-not-delete"
```

---

## Task 7: LLM wrapper (`llm.py`)

**Files:**
- Create: `experiments/compound-memory-spike/compound_spike/llm.py`
- Test: `experiments/compound-memory-spike/tests/test_harness.py` (mock target lives here; no standalone test for the network call)

This is the only network-touching module. Keep it a one-function seam so everything else mocks it.

- [ ] **Step 1: Write minimal implementation (no test — it is the mock seam)**

```python
# compound_spike/llm.py
from dotenv import load_dotenv
import litellm

load_dotenv()

def complete(model: str, system: str, user: str, temperature: float = 0.0) -> str:
    """Single chat completion via litellm. The one seam tests mock."""
    resp = litellm.completion(
        model=model,
        messages=[
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
        temperature=temperature,
    )
    return resp["choices"][0]["message"]["content"]
```

- [ ] **Step 2: Verify it imports**

Run: `python -c "from compound_spike import llm; print('ok')"`
Expected: `ok` (no network call made on import)

- [ ] **Step 3: Commit**

```bash
git add compound_spike/llm.py
git commit -m "feat(spike): litellm completion seam"
```

---

## Task 8: Distillation (`distill.py`)

**Files:**
- Create: `experiments/compound-memory-spike/compound_spike/distill.py`
- Test: `experiments/compound-memory-spike/tests/test_distill.py`

Pipeline: `chunk_conversations` (group messages per conversation, split into ≤`DISTILL_CHUNK_TOKENS` windows) → `extract_facts` (one LLM call per chunk, returns candidate facts JSON) → `reconcile` (Task 6) → `Profile`.

- [ ] **Step 1: Write the failing test**

```python
# tests/test_distill.py
import json
from compound_spike.models import Message
from compound_spike import distill
from compound_spike.tokens import TokenCounter

def _msgs(n, source="claude"):
    return [Message(source, "c1", f"m{i}", "user" if i % 2 == 0 else "assistant",
                    f"sentence number {i} " * 20, "2026-01-0{}".format(i % 9 + 1)) for i in range(n)]

def test_chunking_respects_token_budget():
    chunks = distill.chunk_conversations(_msgs(40), TokenCounter(), max_tokens=300)
    assert len(chunks) > 1
    counter = TokenCounter()
    assert all(counter.count(c.text) <= 300 + 200 for c in chunks)  # +1 message slack

def test_distill_calls_llm_per_chunk_and_reconciles(monkeypatch):
    # Fake the LLM: each chunk returns one fact about Rust (duplicate across chunks).
    fake_json = json.dumps({"facts": [{
        "category": "preferences", "statement": "Prefers Rust",
        "conflict_key": "lang",
        "provenance": [{"source": "claude", "conv_id": "c1", "msg_id": "m0", "ts": "2026-01-01"}],
        "first_seen": "2026-01-01", "last_seen": "2026-01-01"
    }]})
    monkeypatch.setattr(distill, "complete", lambda **kw: fake_json)
    profile = distill.distill(_msgs(40), TokenCounter())
    # duplicates across chunks reconcile down to one active fact
    assert len(profile.active()) == 1
    assert profile.active()[0].statement == "Prefers Rust"
    assert len(profile.active()[0].provenance) >= 2
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/test_distill.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'compound_spike.distill'`

- [ ] **Step 3: Write minimal implementation**

```python
# compound_spike/distill.py
import json
from dataclasses import dataclass
from compound_spike.models import Message, Fact, Provenance, Profile
from compound_spike.reconcile import reconcile
from compound_spike.tokens import TokenCounter
from compound_spike.llm import complete
from config import DISTILL_MODEL, DISTILL_CHUNK_TOKENS, EXTRACT_TEMPERATURE

@dataclass
class Chunk:
    conv_id: str
    text: str

EXTRACT_SYSTEM = """You extract durable facts about the USER from conversation excerpts.
Return ONLY JSON: {"facts":[{"category","statement","conflict_key","first_seen","last_seen","provenance":[{"source","conv_id","msg_id","ts"}]}]}
Rules: category in [identity,preferences,projects,decisions,patterns,relationships].
Emit a fact ONLY if a specific message supports it; cite that message in provenance.
conflict_key = a short stable slug for the underlying attribute (e.g. "backend_language") so
contradictory statements over time share a key. No speculation. If nothing durable, return {"facts":[]}."""

def chunk_conversations(messages: list[Message], counter: TokenCounter,
                        max_tokens: int = DISTILL_CHUNK_TOKENS) -> list[Chunk]:
    by_conv: dict[str, list[Message]] = {}
    for m in messages:
        by_conv.setdefault(m.conv_id, []).append(m)
    chunks: list[Chunk] = []
    for conv_id, msgs in by_conv.items():
        buf, buf_tokens = [], 0
        for m in msgs:
            line = f"[{m.source}|{m.msg_id}|{m.ts}] {m.role}: {m.text}"
            t = counter.count(line)
            if buf and buf_tokens + t > max_tokens:
                chunks.append(Chunk(conv_id, "\n".join(buf)))
                buf, buf_tokens = [], 0
            buf.append(line)
            buf_tokens += t
        if buf:
            chunks.append(Chunk(conv_id, "\n".join(buf)))
    return chunks

def extract_facts(chunk: Chunk, idx_start: int) -> list[Fact]:
    raw = complete(model=DISTILL_MODEL, system=EXTRACT_SYSTEM,
                   user=chunk.text, temperature=EXTRACT_TEMPERATURE)
    try:
        data = json.loads(raw)
    except json.JSONDecodeError:
        return []
    facts, keys = [], {}
    for i, fd in enumerate(data.get("facts", [])):
        fid = f"f_{idx_start + i}"
        prov = [Provenance(**p) for p in fd.get("provenance", [])]
        facts.append(Fact(
            id=fid, category=fd.get("category", "patterns"),
            statement=fd.get("statement", "").strip(),
            first_seen=fd.get("first_seen", ""), last_seen=fd.get("last_seen", ""),
            provenance=prov,
        ))
        keys[fid] = fd.get("conflict_key") or fd.get("statement", "")
    extract_facts._last_keys = keys  # carried out via distill()
    return facts

def distill(messages: list[Message], counter: TokenCounter) -> Profile:
    chunks = chunk_conversations(messages, counter)
    all_facts: list[Fact] = []
    conflict_keys: dict[str, str] = {}
    n = 0
    for ch in chunks:
        facts = extract_facts(ch, n)
        n += len(facts)
        all_facts += facts
        conflict_keys.update(getattr(extract_facts, "_last_keys", {}))
    return reconcile(all_facts, conflict_keys)
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/test_distill.py -v`
Expected: PASS (2 passed)

- [ ] **Step 5: Commit**

```bash
git add compound_spike/distill.py tests/test_distill.py
git commit -m "feat(spike): map-reduce distillation to compound profile"
```

---

## Task 9: Injection — profile render + raw excerpts (`inject.py`)

**Files:**
- Create: `experiments/compound-memory-spike/compound_spike/inject.py`
- Test: `experiments/compound-memory-spike/tests/test_inject.py`

`render_profile` turns the active profile into terse grouped bullets (no ids/timestamps/provenance) trimmed to budget. `select_raw_excerpts` is the deliberately-naive arm B: keyword overlap with the task + recency, trimmed to budget.

- [ ] **Step 1: Write the failing test**

```python
# tests/test_inject.py
from compound_spike.models import Message, Fact, Profile, Provenance
from compound_spike.inject import render_profile, select_raw_excerpts
from compound_spike.tokens import TokenCounter

def _profile():
    return Profile("user", [
        Fact("f1", "preferences", "Prefers Rust over Go", first_seen="2025-11", last_seen="2026-04",
             provenance=[Provenance("claude", "c", "m", "2026-04")]),
        Fact("f2", "projects", "Building a SwiftUI app called Streaks", last_seen="2026-04",
             provenance=[Provenance("chatgpt", "c", "m", "2026-04")]),
        Fact("f3", "identity", "Old city Madrid", status="superseded", superseded_by="f4"),
    ])

def test_render_profile_groups_active_only_and_strips_metadata():
    out = render_profile(_profile(), budget=500, counter=TokenCounter())
    assert "Rust" in out and "Streaks" in out
    assert "Madrid" not in out            # superseded excluded
    assert "f1" not in out and "2026-04" not in out  # ids/timestamps stripped

def test_render_profile_respects_budget():
    counter = TokenCounter()
    out = render_profile(_profile(), budget=3, counter=counter)
    assert counter.count(out) <= 3

def test_select_raw_excerpts_prefers_task_keyword_then_recency():
    msgs = [
        Message("claude", "c", "m1", "user", "Unrelated chat about cooking", "2025-01-01"),
        Message("claude", "c", "m2", "user", "I want help planning my Rust backend migration", "2026-04-01"),
    ]
    out = select_raw_excerpts(msgs, task="plan my Rust backend", budget=200, counter=TokenCounter())
    assert "Rust backend migration" in out
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/test_inject.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'compound_spike.inject'`

- [ ] **Step 3: Write minimal implementation**

```python
# compound_spike/inject.py
import re
from compound_spike.models import Message, Profile
from compound_spike.tokens import TokenCounter, fit_to_budget

def render_profile(profile: Profile, budget: int, counter: TokenCounter) -> str:
    groups: dict[str, list[str]] = {}
    for f in profile.active():
        groups.setdefault(f.category, []).append(f.statement)
    lines = []
    for cat in ["identity", "preferences", "projects", "decisions", "patterns", "relationships"]:
        if cat in groups:
            lines.append(f"## {cat.capitalize()}")
            lines += [f"- {s}" for s in groups[cat]]
    text = "What we know about the user:\n" + "\n".join(lines)
    return fit_to_budget(text, budget, counter)

def _tokens(s: str) -> set[str]:
    return set(re.findall(r"[a-z0-9]+", s.lower()))

def select_raw_excerpts(messages: list[Message], task: str, budget: int,
                        counter: TokenCounter) -> str:
    """Deliberately naive arm B: rank by keyword overlap with task, tie-break by recency."""
    task_toks = _tokens(task)
    scored = sorted(
        messages,
        key=lambda m: (len(_tokens(m.text) & task_toks), m.ts),
        reverse=True,
    )
    out, used = [], 0
    for m in scored:
        line = f"{m.role}: {m.text}"
        t = counter.count(line)
        if used + t > budget:
            continue
        out.append(line)
        used += t
    return "Relevant past messages:\n" + "\n".join(out)
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/test_inject.py -v`
Expected: PASS (3 passed)

- [ ] **Step 5: Commit**

```bash
git add compound_spike/inject.py tests/test_inject.py
git commit -m "feat(spike): profile render + naive raw-excerpt selection under budget"
```

---

## Task 10: Harness — run arms under parity (`harness.py`)

**Files:**
- Create: `experiments/compound-memory-spike/compound_spike/harness.py`
- Test: `experiments/compound-memory-spike/tests/test_harness.py`

Builds the three context strings, enforces parity (C ≤ B), runs each consumer × arm, returns structured results.

- [ ] **Step 1: Write the failing test**

```python
# tests/test_harness.py
from compound_spike.models import Message, Fact, Profile, Provenance
from compound_spike import harness
from compound_spike.tokens import TokenCounter

def _profile():
    return Profile("user", [Fact("f1", "preferences", "Prefers Rust", last_seen="2026",
                                 provenance=[Provenance("claude","c","m","2026")])])

def _msgs():
    return [Message("claude", "c", "m1", "user", "I love Rust backends", "2026-01-01")]

def test_run_all_produces_one_result_per_consumer_per_arm(monkeypatch):
    monkeypatch.setattr(harness, "complete",
                        lambda model, system, user, temperature: f"[{model}] answered")
    results = harness.run_all(
        task="plan my week", profile=_profile(), messages=_msgs(),
        counter=TokenCounter(), consumers=["openai/gpt-5.5", "gemini/gemini-3.1-pro"],
    )
    arms = {(r["consumer"], r["arm"]) for r in results}
    assert len(arms) == 6                       # 2 consumers x 3 arms
    cold = [r for r in results if r["arm"] == "cold"][0]
    assert cold["context_tokens"] == 0
    # parity: compound context tokens <= raw context tokens, per consumer
    for c in ["openai/gpt-5.5", "gemini/gemini-3.1-pro"]:
        raw = [r for r in results if r["arm"] == "raw" and r["consumer"] == c][0]
        comp = [r for r in results if r["arm"] == "compound" and r["consumer"] == c][0]
        assert comp["context_tokens"] <= raw["context_tokens"]
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/test_harness.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'compound_spike.harness'`

- [ ] **Step 3: Write minimal implementation**

```python
# compound_spike/harness.py
from compound_spike.models import Message, Profile
from compound_spike.tokens import TokenCounter, assert_budget_parity
from compound_spike.inject import render_profile, select_raw_excerpts
from compound_spike.llm import complete
from config import CONSUMER_MODELS, BUDGET_N, CONSUME_TEMPERATURE

SYSTEM_BASE = ("You are a helpful assistant. If context about the user is provided, "
               "use it to be specific and immediately useful. Do not ask for information "
               "already given.")

def build_contexts(task: str, profile: Profile, messages: list[Message],
                   counter: TokenCounter, budget: int = BUDGET_N) -> dict[str, str]:
    raw = select_raw_excerpts(messages, task, budget, counter)
    compound = render_profile(profile, budget, counter)
    assert_budget_parity(raw=raw, compound=compound, counter=counter)
    return {"cold": "", "raw": raw, "compound": compound}

def run_all(task: str, profile: Profile, messages: list[Message], counter: TokenCounter,
            consumers: list[str] = CONSUMER_MODELS, budget: int = BUDGET_N) -> list[dict]:
    contexts = build_contexts(task, profile, messages, counter, budget)
    results = []
    for consumer in consumers:
        for arm, ctx in contexts.items():
            system = SYSTEM_BASE + (f"\n\n{ctx}" if ctx else "")
            answer = complete(model=consumer, system=system, user=task,
                              temperature=CONSUME_TEMPERATURE)
            results.append({
                "consumer": consumer, "arm": arm,
                "context_tokens": counter.count(ctx),
                "answer": answer,
            })
    return results
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/test_harness.py -v`
Expected: PASS (1 passed)

- [ ] **Step 5: Commit**

```bash
git add compound_spike/harness.py tests/test_harness.py
git commit -m "feat(spike): harness runs consumer x arm under budget parity"
```

---

## Task 11: Judge — blind table (`judge.py`)

**Files:**
- Create: `experiments/compound-memory-spike/compound_spike/judge.py`
- Test: `experiments/compound-memory-spike/tests/test_judge.py`

Produces a blind, randomized (seeded) DataFrame hiding which arm is which, plus the unblind mapping.

- [ ] **Step 1: Write the failing test**

```python
# tests/test_judge.py
from compound_spike.judge import build_blind_table

def _results():
    out = []
    for consumer in ["openai/gpt-5.5", "gemini/gemini-3.1-pro"]:
        for arm in ["cold", "raw", "compound"]:
            out.append({"consumer": consumer, "arm": arm,
                        "context_tokens": 0 if arm == "cold" else 100,
                        "answer": f"{consumer}-{arm}-answer"})
    return out

def test_blind_table_hides_arm_and_mapping_recovers_it():
    df, mapping = build_blind_table(_results(), seed=7)
    assert "arm" not in df.columns           # arm hidden from the judge
    assert "label" in df.columns and "answer" in df.columns
    assert len(df) == 6 and len(mapping) == 6
    # every label maps back to a real (consumer, arm)
    for label, meta in mapping.items():
        assert meta["arm"] in {"cold", "raw", "compound"}

def test_blind_table_is_deterministic_for_a_seed():
    df1, _ = build_blind_table(_results(), seed=7)
    df2, _ = build_blind_table(_results(), seed=7)
    assert list(df1["label"]) == list(df2["label"])
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/test_judge.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'compound_spike.judge'`

- [ ] **Step 3: Write minimal implementation**

```python
# compound_spike/judge.py
import random
import pandas as pd

def build_blind_table(results: list[dict], seed: int = 0):
    rng = random.Random(seed)
    shuffled = results[:]
    rng.shuffle(shuffled)
    rows, mapping = [], {}
    for i, r in enumerate(shuffled):
        label = f"OUT-{i+1:02d}"
        mapping[label] = {"consumer": r["consumer"], "arm": r["arm"],
                          "context_tokens": r["context_tokens"]}
        rows.append({"label": label, "answer": r["answer"], "score": ""})
    df = pd.DataFrame(rows, columns=["label", "answer", "score"])
    return df, mapping
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/test_judge.py -v`
Expected: PASS (2 passed)

- [ ] **Step 5: Commit**

```bash
git add compound_spike/judge.py tests/test_judge.py
git commit -m "feat(spike): seeded blind judging table + unblind mapping"
```

---

## Task 12: CLI wiring (`cli.py`)

**Files:**
- Create: `experiments/compound-memory-spike/compound_spike/cli.py`

Wires the four commands. No new logic; persists intermediate artifacts to `out/`.

- [ ] **Step 1: Write the implementation**

```python
# compound_spike/cli.py
import argparse, json
from pathlib import Path
from compound_spike.ingest import load_all
from compound_spike.distill import distill
from compound_spike.harness import run_all
from compound_spike.judge import build_blind_table
from compound_spike.models import Profile
from compound_spike.tokens import TokenCounter
from config import OUT_DIR, DATA_DIR

OUT_DIR.mkdir(exist_ok=True)

def _cmd_ingest(args):
    msgs = load_all(chatgpt_path=args.chatgpt, claude_path=args.claude)
    out = OUT_DIR / "messages.json"
    out.write_text(json.dumps([m.__dict__ for m in msgs], indent=2))
    print(f"ingested {len(msgs)} messages -> {out}")

def _cmd_distill(args):
    from compound_spike.models import Message
    msgs = [Message(**d) for d in json.loads((OUT_DIR / "messages.json").read_text())]
    profile = distill(msgs, TokenCounter())
    (OUT_DIR / "profile.json").write_text(json.dumps(profile.to_dict(), indent=2))
    print(f"distilled {len(profile.active())} active facts -> {OUT_DIR/'profile.json'}")

def _cmd_run(args):
    from compound_spike.models import Message
    msgs = [Message(**d) for d in json.loads((OUT_DIR / "messages.json").read_text())]
    profile = Profile.from_dict(json.loads((OUT_DIR / "profile.json").read_text()))
    results = run_all(task=args.task, profile=profile, messages=msgs, counter=TokenCounter())
    (OUT_DIR / "results.json").write_text(json.dumps({"task": args.task, "results": results}, indent=2))
    print(f"ran {len(results)} arm-runs -> {OUT_DIR/'results.json'}")

def _cmd_judge(args):
    payload = json.loads((OUT_DIR / "results.json").read_text())
    df, mapping = build_blind_table(payload["results"], seed=args.seed)
    df.to_csv(OUT_DIR / "blind_table.csv", index=False)
    (OUT_DIR / "blind_mapping.json").write_text(json.dumps(mapping, indent=2))
    print(f"task: {payload['task']}")
    print(f"score blind_table.csv (fill 1-5), then reveal with blind_mapping.json")

def main():
    p = argparse.ArgumentParser(prog="compound_spike")
    sub = p.add_subparsers(required=True)
    pi = sub.add_parser("ingest"); pi.add_argument("--chatgpt", type=Path); pi.add_argument("--claude", type=Path); pi.set_defaults(func=_cmd_ingest)
    pd_ = sub.add_parser("distill"); pd_.set_defaults(func=_cmd_distill)
    pr = sub.add_parser("run"); pr.add_argument("--task", required=True); pr.set_defaults(func=_cmd_run)
    pj = sub.add_parser("judge"); pj.add_argument("--seed", type=int, default=0); pj.set_defaults(func=_cmd_judge)
    args = p.parse_args(); args.func(args)

if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Verify the CLI loads**

Run: `python -m compound_spike.cli --help`
Expected: usage text listing `ingest`, `distill`, `run`, `judge`

- [ ] **Step 3: Full mock-free unit suite green**

Run: `pytest -v`
Expected: all tests pass (models, tokens, ingest, reconcile, distill, inject, harness, judge)

- [ ] **Step 4: Commit**

```bash
git add compound_spike/cli.py
git commit -m "feat(spike): CLI wiring for ingest/distill/run/judge"
```

---

## Task 13: End-to-end dry run on fixtures (no keys)

**Files:** none (verification task)

- [ ] **Step 1: Ingest the fixtures**

Run:
```bash
python -m compound_spike.cli ingest \
  --chatgpt tests/fixtures/chatgpt_export_sample.json \
  --claude tests/fixtures/claude_export_sample.json
```
Expected: `ingested 4 messages -> out/messages.json`

- [ ] **Step 2: Confirm normalized log looks right**

Run: `python -c "import json; print(len(json.load(open('out/messages.json'))))"`
Expected: `4`

This confirms the deterministic spine (parse → persist) end-to-end. `distill`/`run` need API keys and are exercised in Task 14.

- [ ] **Step 3: Commit (nothing to commit — artifacts are gitignored)**

Run: `git status --short`
Expected: clean (out/ is gitignored)

---

## Task 14: Real run + verdict (manual, needs API keys)

**Files:**
- Create: `experiments/compound-memory-spike/out/VERDICT.md` (the deliverable; gitignored dir, so copy the verdict into the PR description or a tracked location when done)

> This task is the actual experiment. It requires `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`,
> `GEMINI_API_KEY` and confirmed-live model ids in `config.py`. Do it WITH the user.

- [ ] **Step 1: Confirm model availability**

Run: `python -c "import litellm; litellm.completion(model='anthropic/claude-opus-4-8', messages=[{'role':'user','content':'ping'}], max_tokens=5)"`
Expected: a completion (no `NotFoundError`). If a model id is wrong, fix `config.py` and re-run.

- [ ] **Step 2: Drop real exports into `data/` and ingest**

Run:
```bash
python -m compound_spike.cli ingest --chatgpt data/chatgpt.json --claude data/claude.json
```
Expected: `ingested <N> messages` with N in the hundreds/thousands.

- [ ] **Step 3: Distill and audit the profile for hallucinations**

Run: `python -m compound_spike.cli distill`
Then open `out/profile.json` and spot-check ~10 facts: follow each `provenance.msg_id` back into `out/messages.json` and confirm the source actually supports the statement. Record the fabrication rate.

- [ ] **Step 4: Pick the real task WITH the user and run**

Run: `python -m compound_spike.cli run --task "<the real task chosen at runtime>"`
Expected: `ran 6 arm-runs -> out/results.json`

- [ ] **Step 5: Judge blind**

Run: `python -m compound_spike.cli judge --seed 7`
Open `out/blind_table.csv`, score each output 1–5 on: usefulness from msg one · specificity · correct assumptions · absence of hallucinations. Then reveal arms via `out/blind_mapping.json`.

- [ ] **Step 6: Write the verdict**

In `out/VERDICT.md` record, per consumer: mean score for cold/raw/compound, whether **compound > raw** held at parity, the profile fabrication rate, and a one-paragraph call: **confirmed / weak / killed**. Copy this into the PR description (since `out/` is gitignored).

- [ ] **Step 7: Commit the plan-completion marker**

```bash
git commit --allow-empty -m "chore(spike): compound-memory value experiment complete (see PR for verdict)"
```

---

## Self-Review

**Spec coverage:**
- Hypothesis + decision rule (spec §2) → Tasks 10, 14 (parity run + verdict). ✓
- Three arms cold/raw/compound (spec §2-3) → `harness.build_contexts` (Task 10). ✓
- Consumer outer loop, two non-Claude consumers (spec §3) → `config.CONSUMER_MODELS` + `run_all` (Tasks 1, 10). ✓
- Claude distills (spec §3) → `config.DISTILL_MODEL` + `distill` (Tasks 1, 8). ✓
- Budget parity counted in shared tokenizer (spec §3, §9) → `tokens` + `assert_budget_parity` (Tasks 3, 10). ✓
- Ingest ChatGPT + Claude (spec §5) → Task 5. ✓
- Map-reduce distill + provenance + recency reconcile (spec §6-7) → Tasks 6, 8. ✓
- Inject whole-profile-if-fits as bullets, strip metadata; naive raw excerpts (spec §7, §9) → Task 9. ✓
- Blind randomized judging + rubric (spec §8) → Tasks 11, 14. ✓
- Provenance hallucination audit (spec §8) → Task 14 Step 3. ✓
- Deferred items (embeddings, NLI, bi-temporal) correctly NOT built (spec §7). ✓
- Definition of done = verdict artifact (spec §13) → Task 14 Step 6. ✓

**Placeholder scan:** No TBD/TODO/"handle edge cases". The one `<the real task chosen at runtime>` is intentional per spec §12 (task picked with user) and lives in a manual step, not code. ✓

**Type consistency:** `Message`, `Fact(id,category,statement,status,superseded_by,first_seen,last_seen,provenance)`, `Provenance(source,conv_id,msg_id,ts)`, `Profile.active()` used identically across Tasks 2,5,6,8,9,10. `complete(model,system,user,temperature)` signature consistent in Tasks 7,8,10 and mocked with matching kwargs in tests. `TokenCounter.count` / `assert_budget_parity` / `fit_to_budget` consistent across Tasks 3,9,10. `build_blind_table(results, seed)` consistent Tasks 11,12. ✓

One known nit carried intentionally: `extract_facts` passes conflict keys via a function attribute (`_last_keys`) consumed immediately in `distill`'s loop — a deliberate shortcut acceptable for throwaway code; flagged here so a reviewer doesn't "fix" it into a behavior change.
