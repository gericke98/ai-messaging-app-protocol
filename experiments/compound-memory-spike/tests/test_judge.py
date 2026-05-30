from compound_spike.judge import build_blind_table

def _results():
    out = []
    for consumer in ["openai/gpt-5.5", "gemini/gemini-3.1-pro"]:
        for arm in ["cold", "raw", "compound"]:
            out.append({"consumer": consumer, "arm": arm,
                        "context_tokens": 0 if arm == "cold" else 100,
                        "answer": f"{consumer}-{arm}-answer"})
    return out

def test_blind_table_hides_arm_and_mapping_recovers_it():
    df, mapping = build_blind_table(_results(), seed=7)
    assert "arm" not in df.columns           # arm hidden from the judge
    assert "label" in df.columns and "answer" in df.columns
    assert len(df) == 6 and len(mapping) == 6
    # every label maps back to a real (consumer, arm)
    for label, meta in mapping.items():
        assert meta["arm"] in {"cold", "raw", "compound"}

def test_blind_table_is_deterministic_for_a_seed():
    df1, _ = build_blind_table(_results(), seed=7)
    df2, _ = build_blind_table(_results(), seed=7)
    assert list(df1["label"]) == list(df2["label"])
