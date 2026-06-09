from compound_spike.models import Message
from compound_spike import probes


def _msgs(n=60):
    return [
        Message("claude", f"c{i//6}", f"m{i}", "user" if i % 2 == 0 else "assistant",
                f"Message {i} about project alpha, rust backends, and topic {i % 7}",
                f"2026-01-{(i % 27) + 1:02d}")
        for i in range(n)
    ]


def test_sample_windows_is_deterministic_and_covers_messages():
    w1 = probes.sample_windows(_msgs(), n_windows=5, seed=3)
    w2 = probes.sample_windows(_msgs(), n_windows=5, seed=3)
    assert w1 == w2
    assert len(w1) == 5
    assert all(isinstance(w, str) and w for w in w1)


def test_generate_probes_dedupes_and_reaches_n(monkeypatch):
    calls = []

    def fake_complete(model, system, user, temperature):
        calls.append(user)
        # Collides every 4th window -> forces dedupe to keep generating
        return f"Plan something about topic {len(calls) % 4} and {len(calls) // 4}"

    monkeypatch.setattr(probes, "complete", fake_complete)
    out = probes.generate_probes(_msgs(), n=8, seed=0)
    assert len(out) == 8
    assert len({probes.normalize(p) for p in out}) == 8


def test_generate_probes_deterministic_for_seed(monkeypatch):
    def fake_complete(model, system, user, temperature):
        return f"Task derived from: {user[:40]}"

    monkeypatch.setattr(probes, "complete", fake_complete)
    a = probes.generate_probes(_msgs(), n=5, seed=7)
    b = probes.generate_probes(_msgs(), n=5, seed=7)
    assert a == b


def test_extra_tasks_lead_and_count_toward_n(monkeypatch):
    def fake_complete(model, system, user, temperature):
        return f"Generated {user[:30]}"

    monkeypatch.setattr(probes, "complete", fake_complete)
    out = probes.generate_probes(_msgs(), n=4, seed=0,
                                 extra=["Review my Rust server architecture"])
    assert out[0] == "Review my Rust server architecture"
    assert len(out) == 4


def test_generate_probes_raises_when_not_enough_unique(monkeypatch):
    monkeypatch.setattr(probes, "complete",
                        lambda model, system, user, temperature: "Always the same task")
    try:
        probes.generate_probes(_msgs(12), n=5, seed=0)
        raise AssertionError("expected ValueError")
    except ValueError:
        pass
