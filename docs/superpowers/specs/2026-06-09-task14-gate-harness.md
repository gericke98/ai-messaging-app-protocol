# Task-14 Gate Harness — Measurement-Rigor Upgrade (M0 prep)

**Date:** 2026-06-09
**Status:** Approved scope (user: "yes please" to the upgrade offer); exports pending from user.
**Parent:** ship-roadmap M0 (§3 measurement-rigor addendum) · spike design `2026-05-30-compound-memory-value-spike-design.md`

## Problem

The existing spike harness runs ONE task through 3 arms and emits a blind CSV for a
human to score. The M0 gate requires: ≥20 probes per arm, ≥3 independent runs, an LLM
judge **distinct from the briefing model**, per-probe blind side-by-side scoring against
a **pinned rubric**, and the PASS rule (C ≥ 4.2/5 AND C−B ≥ 0.5) evaluated on
**cross-run means** — reproducibly logged.

## Design

New modules in `experiments/compound-memory-spike/compound_spike/`:

- **`probes.py`** — generate ≥N_PROBES realistic tasks from the user's ingested raw
  messages (deterministic window sampling, seeded; normalized dedupe). Probes derive
  from raw messages — the same source both arm B and arm C draw on — never from the
  distilled profile (which would bias toward arm C). User-supplied tasks merge in via
  `--extra tasks.txt`; the user's "real task" should be one of them.
- **`judge.py`** (upgraded; `build_blind_table` kept) — `RUBRIC` pinned versioned
  constant; `judge_probe()` shows all three answers side-by-side per probe, blind
  (OUT-1..3, deterministic shuffle by seed), scores each 1–5 in one call (within-probe
  calibration), strict-JSON parse with one retry. Judge = `JUDGE_MODEL`.
- **`gate.py`** — orchestration + pure aggregation: per-run per-arm means → cross-run
  mean ± stdev → verdict. PASS iff `mean(C) ≥ 4.2` and `mean(C) − mean(B) ≥ 0.5` on
  cross-run means. Writes `out/gate/records.json`, `report.json`, `report.md`.
- **`cli.py`** — new commands: `probes`, `gate` (one command once `data/` has the
  exports: probes → N_RUNS full sweeps → judge → verdict), `preflight` (1-token ping
  of every configured model so routing/key failures surface before the real run).

## Model assignments (config.py)

| Role | Model | Why |
|---|---|---|
| Distill (briefing) | `anthropic/claude-opus-4-8` | existing |
| Consumers | `openai/gpt-5.5`, `gemini/gemini-3.1-pro` | existing |
| **Judge + probe-gen** | `anthropic/claude-fable-5` | distinct from the briefing model (gate requirement) and never judges its own answers — consumers are different vendors entirely |

Tunables: `N_PROBES=20`, `N_RUNS=3`, `JUDGE_TEMPERATURE=0.0`, thresholds
`PASS_MEAN_C=4.2`, `PASS_MARGIN_CB=0.5`.

## Independence across runs

Consumer calls use `CONSUME_TEMPERATURE=0.3` (natural variance); probe order shuffles
per run; blind-label shuffle seeds vary per (run, probe). Judge stays at temperature 0
against the pinned rubric — variance we measure is the *system under test*, not the ruler.

## What stays manual

- `data/chatgpt.json` + `data/claude.json` — the user's exports (the gate's whole point).
- The verdict interpretation + founder go/no-go decision.

## Cost envelope

20 probes × 2 consumers × 3 arms × 3 runs = 360 consumer calls (+120 judge calls,
+distill once). Roughly low-tens of dollars at current prices — acceptable for the gate.
