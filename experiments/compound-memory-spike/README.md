# Compound Memory Value Spike + M0 Gate Harness

Tests whether a distilled cross-agent memory profile beats no-memory and raw-transcript
arms at equal token budget. Now carries the **Task-14 / M0 go-no-go gate** protocol:
≥20 probes, ≥3 independent runs, blind per-probe LLM judging against a pinned rubric,
verdict on cross-run means. See
`docs/superpowers/specs/2026-05-30-compound-memory-value-spike-design.md` and
`docs/superpowers/specs/2026-06-09-task14-gate-harness.md`.

## Setup
```bash
cd experiments/compound-memory-spike
python -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
cp .env.example .env   # then fill in keys
```

`.env` needs: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`.
Confirm the model ids in `config.py` are live before running.

## Run — M0 gate (the Task-14 go/no-go)
```bash
# 0. verify keys/models route before spending anything
python -m compound_spike.cli preflight

# 1. drop ChatGPT + Claude exports into data/
python -m compound_spike.cli ingest   --chatgpt data/chatgpt.json --claude data/claude.json
python -m compound_spike.cli distill                      # -> out/profile.json

# 2. generate >=20 probe tasks from your raw messages
#    (--extra tasks.txt to lead with your own real tasks, one per line)
python -m compound_spike.cli probes --n 20 --extra my_tasks.txt

# 3. the gate: 3 independent runs x 20 probes x consumers x 3 arms,
#    LLM-judged blind per probe (judge model distinct from the distiller),
#    PASS iff cross-run mean(C) >= 4.2 AND C - B >= 0.5
python -m compound_spike.cli gate                         # -> out/gate/report.{json,md}
```

## Run — single-task exploration (legacy flow)
```bash
python -m compound_spike.cli run      --task "<your real task>"   # -> out/results.json
python -m compound_spike.cli judge                        # -> out/blind_table.csv + scoring
```

## Tests
```bash
pytest -v
```
