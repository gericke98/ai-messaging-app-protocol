import random
import pandas as pd

def build_blind_table(results: list[dict], seed: int = 0):
    rng = random.Random(seed)
    shuffled = results[:]
    rng.shuffle(shuffled)
    rows, mapping = [], {}
    for i, r in enumerate(shuffled):
        label = f"OUT-{i+1:02d}"
        mapping[label] = {"consumer": r["consumer"], "arm": r["arm"],
                          "context_size": r.get("context_tokens", r.get("context_chars"))}
        rows.append({"label": label, "answer": r["answer"], "score": ""})
    df = pd.DataFrame(rows, columns=["label", "answer", "score"])
    return df, mapping
