"""Answer-quality A/B over the REAL Rust briefing arms (out/dogfood_briefings.json).

Validates the production `generate_briefing` artifact directly: does a fresh agent
answer the task visibly better with arm C (new briefing) than arm B (dump) or arm A (none)?
Reuses the same seeded profile + task already emitted by the dogfood integration test,
so no real export / distill / Anthropic key is needed — only ONE consumer key.
"""
import json
from pathlib import Path
from compound_spike.llm import complete
from compound_spike.harness import SYSTEM_BASE
from compound_spike.judge import build_blind_table
from config import OUT_DIR, CONSUME_TEMPERATURE

CAT_ORDER = ["identity", "preferences", "projects", "decisions", "patterns", "relationships"]

def render_arm_c(arm_c: dict) -> str:
    """Render the briefing dict (peer card + ranked sections) into injectable text."""
    parts = []
    pc = arm_c.get("peer_card")
    if pc:
        parts.append(pc.strip())
    sections = arm_c.get("sections") or {}
    def _label(f):                                 # sections hold node dicts; inject the .label
        return f.get("label", "") if isinstance(f, dict) else str(f)
    body = []
    def _emit(cat, facts):
        labels = [_label(f) for f in facts if _label(f)]
        if labels:
            body.append(f"## {cat.capitalize()}")
            body.extend(f"- {l}" for l in labels)
    for cat in CAT_ORDER:
        if sections.get(cat):
            _emit(cat, sections[cat])
    for cat, facts in sections.items():            # any categories outside the canonical order
        if cat not in CAT_ORDER and facts:
            _emit(cat, facts)
    text = ""
    if parts:
        text += "\n".join(parts) + "\n\n"
    text += "What we know about the user:\n" + "\n".join(body)
    return text.strip()

def load_arms(path: Path) -> dict:
    d = json.load(open(path))
    return {
        "task": d["task"],
        "arms": {
            "cold": d["arm_a_none"] or "",
            "dump": d["arm_b_dump"],
            "briefing": render_arm_c(d["arm_c_new_briefing"]),
        },
    }

def run(consumer: str, brief_path: Path | None = None) -> dict:
    brief_path = brief_path or (OUT_DIR / "dogfood_briefings.json")
    loaded = load_arms(brief_path)
    task = loaded["task"]
    results = []
    for arm, ctx in loaded["arms"].items():
        system = SYSTEM_BASE + (f"\n\n{ctx}" if ctx else "")
        answer = complete(model=consumer, system=system, user=task,
                          temperature=CONSUME_TEMPERATURE)
        results.append({"consumer": consumer, "arm": arm,
                        "context_chars": len(ctx), "answer": answer})
    return {"task": task, "consumer": consumer, "results": results}

# Routed through Vercel AI Gateway: one VERCEL_AI_GATEWAY_API_KEY, slug = vercel_ai_gateway/<provider>/<model>
DEFAULT_CONSUMERS = [
    "vercel_ai_gateway/openai/gpt-5.5",
    "vercel_ai_gateway/anthropic/claude-opus-4-8",
    "vercel_ai_gateway/google/gemini-3.1-pro",
]

if __name__ == "__main__":
    import sys
    consumers = sys.argv[1:] or DEFAULT_CONSUMERS
    OUT_DIR.mkdir(exist_ok=True)
    all_results, task = [], None
    for consumer in consumers:
        try:
            out = run(consumer)
        except Exception as e:                     # one provider failing must not sink the rest
            print(f"!! {consumer} FAILED: {type(e).__name__}: {e}")
            continue
        task = out["task"]
        all_results.extend(out["results"])
        print(f"OK  {consumer}: {len(out['results'])} answers")
    combined = {"task": task, "consumers": consumers, "results": all_results}
    (OUT_DIR / "ab_results.json").write_text(json.dumps(combined, indent=2))
    if all_results:
        df, mapping = build_blind_table(all_results)
        df.to_csv(OUT_DIR / "ab_blind_table.csv", index=False)
        (OUT_DIR / "ab_mapping.json").write_text(json.dumps(mapping, indent=2))
    print(f"\nWrote {len(all_results)} answers -> out/ab_results.json")
    print("Blind table -> out/ab_blind_table.csv ; unblind map -> out/ab_mapping.json")
