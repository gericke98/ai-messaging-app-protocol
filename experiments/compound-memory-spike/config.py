"""Single source of truth for the spike's tunables."""
from pathlib import Path

ROOT = Path(__file__).parent
DATA_DIR = ROOT / "data"          # user's real exports (gitignored)
OUT_DIR = ROOT / "out"            # results (gitignored)

# Models — confirm availability when wiring keys (see README).
DISTILL_MODEL = "anthropic/claude-opus-4-8"
CONSUMER_MODELS = ["openai/gpt-5.5", "gemini/gemini-3.1-pro"]

# Gate (M0) protocol — judge must be DISTINCT from the briefing/distill model and
# never judges its own answers (consumers are different vendors).
JUDGE_MODEL = "anthropic/claude-fable-5"
PROBE_MODEL = JUDGE_MODEL
N_PROBES = 20
N_RUNS = 3
JUDGE_TEMPERATURE = 0.0
PASS_MEAN_C = 4.2          # arm C cross-run mean must reach this
PASS_MARGIN_CB = 0.5       # and beat arm B by at least this much

# Budget N: shared context-token ceiling for arms B and C (counted with TOKENIZER).
# Arm B fills up to N; arm C must use <= N.
BUDGET_N = 3500

# Single shared tokenizer for the parity gate. Parity only needs B and C counted
# with the SAME counter, so absolute per-vendor accuracy is irrelevant.
TOKENIZER = "o200k_base"

DISTILL_CHUNK_TOKENS = 6000       # window size for map-reduce extraction
EXTRACT_TEMPERATURE = 0.0
CONSUME_TEMPERATURE = 0.3
