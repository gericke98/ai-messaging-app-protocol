import re
from compound_spike.models import Message, Profile
from compound_spike.tokens import TokenCounter, fit_to_budget

def render_profile(profile: Profile, budget: int, counter: TokenCounter) -> str:
    groups: dict[str, list[str]] = {}
    for f in profile.active():
        groups.setdefault(f.category, []).append(f.statement)
    lines = []
    for cat in ["identity", "preferences", "projects", "decisions", "patterns", "relationships"]:
        if cat in groups:
            lines.append(f"## {cat.capitalize()}")
            lines += [f"- {s}" for s in groups[cat]]
    text = "What we know about the user:\n" + "\n".join(lines)
    return fit_to_budget(text, budget, counter)

def _tokens(s: str) -> set[str]:
    return set(re.findall(r"[a-z0-9]+", s.lower()))

def select_raw_excerpts(messages: list[Message], task: str, budget: int,
                        counter: TokenCounter) -> str:
    """Deliberately naive arm B: rank by keyword overlap with task, tie-break by recency."""
    task_toks = _tokens(task)
    scored = sorted(
        messages,
        key=lambda m: (len(_tokens(m.text) & task_toks), m.ts),
        reverse=True,
    )
    out, used = [], 0
    for m in scored:
        line = f"{m.role}: {m.text}"
        t = counter.count(line)
        if used + t > budget:
            continue
        out.append(line)
        used += t
    return "Relevant past messages:\n" + "\n".join(out)
