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
