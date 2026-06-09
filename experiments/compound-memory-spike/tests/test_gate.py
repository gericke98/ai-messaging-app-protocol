import json

from compound_spike import gate


def _records():
    # 2 runs x 2 probes x 1 consumer; compound clearly ahead of raw
    rows = []
    scores = {
        # (run, probe): (cold, raw, compound)
        (0, "p1"): (2, 3, 5),
        (0, "p2"): (2, 4, 5),
        (1, "p1"): (1, 3, 4),
        (1, "p2"): (3, 4, 5),
    }
    for (run, probe), (cold, raw, compound) in scores.items():
        for arm, s in (("cold", cold), ("raw", raw), ("compound", compound)):
            rows.append({"run": run, "probe": probe, "consumer": "openai/gpt-5.5",
                         "arm": arm, "score": s})
    return rows


def test_aggregate_per_run_and_cross_run_means():
    agg = gate.aggregate(_records())
    assert agg["per_run"][0]["compound"] == 5.0
    assert agg["per_run"][1]["compound"] == 4.5
    assert agg["per_run"][0]["raw"] == 3.5
    assert agg["cross_run"]["compound"]["mean"] == 4.75
    assert agg["cross_run"]["raw"]["mean"] == 3.5
    assert agg["cross_run"]["compound"]["stdev"] > 0


def test_verdict_pass_at_exact_thresholds():
    v = gate.verdict({"compound": {"mean": 4.2, "stdev": 0.1},
                      "raw": {"mean": 3.7, "stdev": 0.1},
                      "cold": {"mean": 2.0, "stdev": 0.1}})
    assert v["pass"] is True


def test_verdict_fails_below_absolute_threshold():
    v = gate.verdict({"compound": {"mean": 4.19, "stdev": 0.1},
                      "raw": {"mean": 3.0, "stdev": 0.1},
                      "cold": {"mean": 2.0, "stdev": 0.1}})
    assert v["pass"] is False


def test_verdict_fails_below_margin():
    v = gate.verdict({"compound": {"mean": 4.5, "stdev": 0.1},
                      "raw": {"mean": 4.01, "stdev": 0.1},
                      "cold": {"mean": 2.0, "stdev": 0.1}})
    assert v["pass"] is False


def test_run_gate_orchestrates_and_writes_artifacts(monkeypatch, tmp_path):
    from compound_spike.models import Message, Profile, Fact

    probes_list = ["task one", "task two"]
    msgs = [Message("claude", "c", "m1", "user", "I love Rust backends today", "2026-01-01")]
    profile = Profile("user", [Fact("f1", "preferences", "Prefers Rust")])

    def fake_run_all(task, profile, messages, counter, consumers=None, budget=None):
        return [{"consumer": c, "arm": arm, "context_tokens": 0,
                 "answer": f"{c}-{arm}-{task}"}
                for c in ["openai/gpt-5.5"] for arm in ["cold", "raw", "compound"]]

    def fake_judge_probe(task, answers, seed=0):
        return {"cold": 2, "raw": 3, "compound": 5}

    monkeypatch.setattr(gate, "run_all", fake_run_all)
    monkeypatch.setattr(gate, "judge_probe", fake_judge_probe)

    report = gate.run_gate(probes_list, profile, msgs, runs=2, out_dir=tmp_path)

    assert report["verdict"]["pass"] is True
    # 2 runs x 2 probes x 1 consumer x 3 arms
    assert len(report["records"]) == 12
    assert (tmp_path / "report.json").exists()
    assert (tmp_path / "report.md").exists()
    on_disk = json.loads((tmp_path / "report.json").read_text())
    assert on_disk["verdict"]["pass"] is True
    assert on_disk["rubric_version"]
