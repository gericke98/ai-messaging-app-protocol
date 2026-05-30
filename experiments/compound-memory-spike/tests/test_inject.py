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
