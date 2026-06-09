"""M0 gate orchestration: N independent runs x N probes x consumers x 3 arms,
LLM-judged blind per probe, aggregated to cross-run means, hard verdict.

PASS iff mean(compound) >= PASS_MEAN_C and
        mean(compound) - mean(raw) >= PASS_MARGIN_CB
evaluated on CROSS-RUN means (a single lucky run cannot pass the gate).
"""
import json
import random
import statistics
from pathlib import Path

from compound_spike.harness import run_all
from compound_spike.judge import RUBRIC_VERSION, judge_probe
from compound_spike.models import Message, Profile
from compound_spike.tokens import TokenCounter
from config import N_RUNS, PASS_MARGIN_CB, PASS_MEAN_C

ARMS = ("cold", "raw", "compound")


def aggregate(records: list[dict]) -> dict:
    """records: [{run, probe, consumer, arm, score}] -> per-run and cross-run stats."""
    per_run: dict[int, dict[str, float]] = {}
    runs = sorted({r["run"] for r in records})
    for run in runs:
        per_run[run] = {}
        for arm in ARMS:
            scores = [r["score"] for r in records if r["run"] == run and r["arm"] == arm]
            if scores:
                per_run[run][arm] = statistics.mean(scores)

    cross_run: dict[str, dict[str, float]] = {}
    for arm in ARMS:
        run_means = [per_run[run][arm] for run in runs if arm in per_run[run]]
        if run_means:
            cross_run[arm] = {
                "mean": statistics.mean(run_means),
                "stdev": statistics.stdev(run_means) if len(run_means) > 1 else 0.0,
            }
    return {"per_run": per_run, "cross_run": cross_run}


def verdict(cross_run: dict, pass_mean: float = PASS_MEAN_C,
            margin: float = PASS_MARGIN_CB) -> dict:
    mean_c = cross_run["compound"]["mean"]
    mean_b = cross_run["raw"]["mean"]
    return {
        "pass": bool(mean_c >= pass_mean and (mean_c - mean_b) >= margin),
        "mean_compound": mean_c,
        "mean_raw": mean_b,
        "mean_cold": cross_run.get("cold", {}).get("mean"),
        "margin_over_raw": mean_c - mean_b,
        "thresholds": {"mean_compound": pass_mean, "margin_over_raw": margin},
    }


def _report_md(report: dict) -> str:
    v = report["verdict"]
    lines = [
        "# Task-14 Gate Report",
        "",
        f"**Verdict: {'PASS' if v['pass'] else 'FAIL'}**",
        "",
        f"- compound (C): {v['mean_compound']:.3f} (need >= {v['thresholds']['mean_compound']})",
        f"- raw (B): {v['mean_raw']:.3f}",
        f"- margin C-B: {v['margin_over_raw']:.3f} (need >= {v['thresholds']['margin_over_raw']})",
        f"- cold (A): {v['mean_cold']:.3f}" if v["mean_cold"] is not None else "- cold (A): n/a",
        f"- rubric: {report['rubric_version']} · runs: {report['runs']} · probes: {len(report['probes'])}",
        "",
        "## Per-run arm means",
        "",
        "| run | cold | raw | compound |",
        "|---|---|---|---|",
    ]
    for run, arms in sorted(report["aggregate"]["per_run"].items()):
        lines.append(
            f"| {run} | {arms.get('cold', float('nan')):.3f} "
            f"| {arms.get('raw', float('nan')):.3f} "
            f"| {arms.get('compound', float('nan')):.3f} |"
        )
    cr = report["aggregate"]["cross_run"]
    lines += ["", "## Cross-run (mean ± stdev of run means)", ""]
    for arm in ARMS:
        if arm in cr:
            lines.append(f"- {arm}: {cr[arm]['mean']:.3f} ± {cr[arm]['stdev']:.3f}")
    return "\n".join(lines) + "\n"


def run_gate(probes: list[str], profile: Profile, messages: list[Message],
             runs: int = N_RUNS, out_dir: Path | None = None) -> dict:
    counter = TokenCounter()
    records: list[dict] = []

    for run in range(runs):
        order = probes[:]
        random.Random(run).shuffle(order)  # vary probe order per run
        for p_idx, task in enumerate(order):
            results = run_all(task=task, profile=profile, messages=messages,
                              counter=counter)
            by_consumer: dict[str, dict[str, str]] = {}
            for r in results:
                by_consumer.setdefault(r["consumer"], {})[r["arm"]] = r["answer"]
            for consumer, answers in by_consumer.items():
                scores = judge_probe(task, answers, seed=run * 1000 + p_idx)
                for arm, score in scores.items():
                    records.append({"run": run, "probe": task, "consumer": consumer,
                                    "arm": arm, "score": score})

    agg = aggregate(records)
    report = {
        "rubric_version": RUBRIC_VERSION,
        "runs": runs,
        "probes": probes,
        "records": records,
        "aggregate": agg,
        "verdict": verdict(agg["cross_run"]),
    }
    if out_dir is not None:
        out_dir = Path(out_dir)
        out_dir.mkdir(parents=True, exist_ok=True)
        (out_dir / "report.json").write_text(json.dumps(report, indent=2))
        (out_dir / "report.md").write_text(_report_md(report))
    return report
