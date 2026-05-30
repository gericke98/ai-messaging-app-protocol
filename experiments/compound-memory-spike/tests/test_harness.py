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
