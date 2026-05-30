from compound_spike.models import Fact, Provenance
from compound_spike.reconcile import reconcile

def _fact(fid, statement, last_seen, key=None, category="preferences"):
    return Fact(id=fid, category=category, statement=statement,
                first_seen=last_seen, last_seen=last_seen,
                provenance=[Provenance("claude", "c", fid, last_seen)])

def test_duplicate_facts_merge_provenance_and_extend_last_seen():
    a = _fact("f1", "Prefers Rust", "2025-11-01")
    b = _fact("f2", "Prefers Rust", "2026-04-01")
    profile = reconcile([a, b], conflict_keys={"f1": "lang", "f2": "lang"})
    active = profile.active()
    assert len(active) == 1
    assert active[0].last_seen == "2026-04-01"
    assert len(active[0].provenance) == 2

def test_contradiction_supersedes_older_keeps_both():
    a = _fact("f1", "Lives in Madrid", "2025-01-01", category="identity")
    b = _fact("f2", "Lives in Lisbon", "2026-01-01", category="identity")
    profile = reconcile([a, b], conflict_keys={"f1": "city", "f2": "city"})
    active = profile.active()
    assert len(active) == 1 and active[0].statement == "Lives in Lisbon"
    superseded = [f for f in profile.facts if f.status == "superseded"]
    assert len(superseded) == 1 and superseded[0].superseded_by == "f2"
