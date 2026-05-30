# Compound Memory Value Spike (THROWAWAY)

Tests whether a distilled cross-agent memory profile beats no-memory and raw-transcript
arms on a real task, at equal token budget. See
`docs/superpowers/specs/2026-05-30-compound-memory-value-spike-design.md`.

## Setup
```bash
cd experiments/compound-memory-spike
python -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
cp .env.example .env   # then fill in keys
```

`.env` needs: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`.
Confirm the model ids in `config.py` are live before running.

## Run
```bash
# 1. drop ChatGPT + Claude exports into data/
python -m compound_spike.cli ingest   --chatgpt data/chatgpt.json --claude data/claude.json
python -m compound_spike.cli distill                      # -> out/profile.json
python -m compound_spike.cli run      --task "<your real task>"   # -> out/results.json
python -m compound_spike.cli judge                        # -> out/blind_table.csv + scoring
```

## Tests
```bash
pytest -v
```
