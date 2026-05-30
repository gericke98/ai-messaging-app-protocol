import tiktoken
from config import TOKENIZER

class TokenCounter:
    """One shared tokenizer. Parity only needs B and C counted the SAME way."""
    def __init__(self, encoding_name: str = TOKENIZER):
        self._enc = tiktoken.get_encoding(encoding_name)

    def count(self, text: str) -> int:
        return len(self._enc.encode(text or ""))

def assert_budget_parity(raw: str, compound: str, counter: TokenCounter) -> None:
    r, c = counter.count(raw), counter.count(compound)
    if c > r:
        raise ValueError(
            f"budget parity violated: compound={c} tokens > raw={r} tokens"
        )

def fit_to_budget(text: str, budget: int, counter: TokenCounter) -> str:
    """Truncate text to <= budget tokens (token-accurate)."""
    enc = counter._enc
    toks = enc.encode(text or "")
    if len(toks) <= budget:
        return text
    return enc.decode(toks[:budget])
