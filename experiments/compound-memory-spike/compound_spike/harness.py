from compound_spike.models import Message, Profile
from compound_spike.tokens import TokenCounter, assert_budget_parity
from compound_spike.inject import render_profile, select_raw_excerpts
from compound_spike.llm import complete
from config import CONSUMER_MODELS, BUDGET_N, CONSUME_TEMPERATURE

SYSTEM_BASE = ("You are a helpful assistant. If context about the user is provided, "
               "use it to be specific and immediately useful. Do not ask for information "
               "already given.")

def build_contexts(task: str, profile: Profile, messages: list[Message],
                   counter: TokenCounter, budget: int = BUDGET_N) -> dict[str, str]:
    raw = select_raw_excerpts(messages, task, budget, counter)
    raw_tokens = counter.count(raw)
    compound = render_profile(profile, min(budget, raw_tokens), counter)
    assert_budget_parity(raw=raw, compound=compound, counter=counter)
    return {"cold": "", "raw": raw, "compound": compound}

def run_all(task: str, profile: Profile, messages: list[Message], counter: TokenCounter,
            consumers: list[str] = CONSUMER_MODELS, budget: int = BUDGET_N) -> list[dict]:
    contexts = build_contexts(task, profile, messages, counter, budget)
    results = []
    for consumer in consumers:
        for arm, ctx in contexts.items():
            system = SYSTEM_BASE + (f"\n\n{ctx}" if ctx else "")
            answer = complete(model=consumer, system=system, user=task,
                              temperature=CONSUME_TEMPERATURE)
            results.append({
                "consumer": consumer, "arm": arm,
                "context_tokens": counter.count(ctx),
                "answer": answer,
            })
    return results
