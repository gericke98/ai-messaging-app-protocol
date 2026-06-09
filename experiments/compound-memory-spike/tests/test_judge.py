import json

from compound_spike import judge
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


# ── LLM judge (gate protocol) ─────────────────────────────────────────────

ANSWERS = {"cold": "ANSWER_COLD_X", "raw": "ANSWER_RAW_Y", "compound": "ANSWER_COMPOUND_Z"}


def _scoring_complete(scores_by_marker):
    """Fake judge: finds which blind label each marker answer got, scores accordingly."""
    def fake(model, system, user, temperature):
        out = {}
        for line in user.splitlines():
            for marker, score in scores_by_marker.items():
                if marker in line and "OUT-" in line:
                    label = line[line.index("OUT-"):line.index("OUT-") + 6]
                    out[label.rstrip(":")] = score
        return json.dumps(out)
    return fake


def test_judge_probe_is_blind_contains_rubric_and_maps_back(monkeypatch):
    prompts = []

    def fake(model, system, user, temperature):
        prompts.append((system, user))
        return _scoring_complete(
            {"ANSWER_COLD_X": 1, "ANSWER_RAW_Y": 3, "ANSWER_COMPOUND_Z": 5}
        )(model, system, user, temperature)

    monkeypatch.setattr(judge, "complete", fake)
    result = judge.judge_probe("plan my week", ANSWERS, seed=4)

    assert result == {"cold": 1, "raw": 3, "compound": 5}
    system, user = prompts[0]
    full = system + user
    assert judge.RUBRIC in full                      # pinned rubric present
    assert "plan my week" in user                    # the probe
    for arm_name in ("cold", "raw", "compound"):     # arm identities hidden
        assert arm_name not in user


def test_judge_probe_same_seed_same_prompt(monkeypatch):
    prompts = []

    def fake(model, system, user, temperature):
        prompts.append(user)
        return _scoring_complete(
            {"ANSWER_COLD_X": 2, "ANSWER_RAW_Y": 2, "ANSWER_COMPOUND_Z": 2}
        )(model, system, user, temperature)

    monkeypatch.setattr(judge, "complete", fake)
    judge.judge_probe("t", ANSWERS, seed=9)
    judge.judge_probe("t", ANSWERS, seed=9)
    assert prompts[0] == prompts[1]


def test_judge_probe_retries_once_then_raises(monkeypatch):
    calls = []

    def fake(model, system, user, temperature):
        calls.append(1)
        return "this is not json at all"

    monkeypatch.setattr(judge, "complete", fake)
    try:
        judge.judge_probe("t", ANSWERS, seed=0)
        raise AssertionError("expected ValueError")
    except ValueError:
        pass
    assert len(calls) == 2


def test_judge_probe_accepts_fenced_json(monkeypatch):
    def fake(model, system, user, temperature):
        inner = _scoring_complete(
            {"ANSWER_COLD_X": 2, "ANSWER_RAW_Y": 4, "ANSWER_COMPOUND_Z": 4}
        )(model, system, user, temperature)
        return f"```json\n{inner}\n```"

    monkeypatch.setattr(judge, "complete", fake)
    result = judge.judge_probe("t", ANSWERS, seed=1)
    assert result == {"cold": 2, "raw": 4, "compound": 4}
