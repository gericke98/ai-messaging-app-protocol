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
