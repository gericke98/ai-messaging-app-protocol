from compound_spike.tokens import TokenCounter, assert_budget_parity
import pytest

def test_counter_counts_more_tokens_for_longer_text():
    c = TokenCounter()
    assert c.count("hello world") < c.count("hello world " * 50)

def test_parity_passes_when_compound_not_larger_than_raw():
    c = TokenCounter()
    raw = "word " * 200
    compound = "word " * 100
    # should not raise
    assert_budget_parity(raw=raw, compound=compound, counter=c)

def test_parity_fails_when_compound_exceeds_raw():
    c = TokenCounter()
    raw = "word " * 100
    compound = "word " * 200
    with pytest.raises(ValueError, match="budget parity violated"):
        assert_budget_parity(raw=raw, compound=compound, counter=c)
