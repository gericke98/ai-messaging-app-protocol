from compound_spike.models import Fact, Profile

def _norm(s: str) -> str:
    return " ".join(s.lower().split())

def reconcile(facts: list[Fact], conflict_keys: dict[str, str]) -> Profile:
    """conflict_keys maps fact.id -> a semantic key. Facts sharing a (category, key)
    are reconciled: identical statements merge; differing statements => recency wins,
    older marked superseded. Facts with no shared key are kept as-is.
    Order-independent: we sort by last_seen so the newest wins deterministically."""
    ordered = sorted(facts, key=lambda f: f.last_seen)
    winners: dict[tuple[str, str], Fact] = {}
    result: list[Fact] = []

    for f in ordered:
        key = (f.category, conflict_keys.get(f.id, _norm(f.statement)))
        if key not in winners:
            winners[key] = f
            result.append(f)
            continue
        prev = winners[key]
        if _norm(prev.statement) == _norm(f.statement):
            # duplicate: merge provenance into the kept fact, extend last_seen
            prev.provenance.extend(f.provenance)
            prev.last_seen = max(prev.last_seen, f.last_seen)
            prev.first_seen = min(prev.first_seen, f.first_seen)
        else:
            # contradiction: newer (f) wins, older (prev) superseded but kept
            prev.status = "superseded"
            prev.superseded_by = f.id
            winners[key] = f
            result.append(f)
    return Profile(subject="user", facts=result + [
        f for f in ordered if f.status == "superseded" and f not in result
    ])
