from compound_spike import llm


def _capture(monkeypatch, captured):
    def fake_completion(model, messages, temperature):
        captured.append(model)
        return {"choices": [{"message": {"content": "ok"}}]}
    monkeypatch.setattr(llm.litellm, "completion", fake_completion)


def test_routes_through_vercel_gateway_when_only_gateway_key_present(monkeypatch):
    captured = []
    _capture(monkeypatch, captured)
    monkeypatch.setenv("VERCEL_AI_GATEWAY_API_KEY", "vck_test")
    monkeypatch.delenv("ANTHROPIC_API_KEY", raising=False)
    monkeypatch.delenv("OPENAI_API_KEY", raising=False)
    monkeypatch.delenv("GEMINI_API_KEY", raising=False)

    llm.complete(model="anthropic/claude-opus-4-8", system="s", user="u")
    assert captured == ["vercel_ai_gateway/anthropic/claude-opus-4-8"]


def test_uses_direct_provider_when_provider_keys_present(monkeypatch):
    captured = []
    _capture(monkeypatch, captured)
    monkeypatch.delenv("VERCEL_AI_GATEWAY_API_KEY", raising=False)
    monkeypatch.setenv("ANTHROPIC_API_KEY", "sk-test")

    llm.complete(model="anthropic/claude-opus-4-8", system="s", user="u")
    assert captured == ["anthropic/claude-opus-4-8"]


def test_gemini_translates_to_google_slug_on_gateway(monkeypatch):
    # The gateway catalogs Google models under google/, not litellm's gemini/.
    captured = []
    _capture(monkeypatch, captured)
    monkeypatch.setenv("VERCEL_AI_GATEWAY_API_KEY", "vck_test")
    monkeypatch.delenv("GEMINI_API_KEY", raising=False)
    monkeypatch.delenv("GOOGLE_API_KEY", raising=False)

    llm.complete(model="gemini/gemini-3.1-pro", system="s", user="u")
    assert captured == ["vercel_ai_gateway/google/gemini-3.1-pro"]


def test_never_double_prefixes(monkeypatch):
    captured = []
    _capture(monkeypatch, captured)
    monkeypatch.setenv("VERCEL_AI_GATEWAY_API_KEY", "vck_test")
    monkeypatch.delenv("ANTHROPIC_API_KEY", raising=False)
    monkeypatch.delenv("OPENAI_API_KEY", raising=False)
    monkeypatch.delenv("GEMINI_API_KEY", raising=False)

    llm.complete(model="vercel_ai_gateway/anthropic/claude-opus-4-8", system="s", user="u")
    assert captured == ["vercel_ai_gateway/anthropic/claude-opus-4-8"]
