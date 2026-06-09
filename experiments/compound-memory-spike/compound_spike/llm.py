import os

from dotenv import load_dotenv
import litellm

load_dotenv()

GATEWAY_PREFIX = "vercel_ai_gateway/"
_PROVIDER_KEYS = {
    "anthropic/": ("ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"),
    "openai/": ("OPENAI_API_KEY",),
    "gemini/": ("GEMINI_API_KEY", "GOOGLE_API_KEY"),
}


def _route(model: str) -> str:
    """Prefer a direct provider key when present; otherwise fall back to the
    Vercel AI Gateway (one key routes every provider)."""
    if model.startswith(GATEWAY_PREFIX):
        return model
    for prefix, env_keys in _PROVIDER_KEYS.items():
        if model.startswith(prefix):
            if any(os.getenv(k) for k in env_keys):
                return model
            break
    if os.getenv("VERCEL_AI_GATEWAY_API_KEY"):
        # The gateway catalogs Google models under google/, not litellm's gemini/.
        if model.startswith("gemini/"):
            model = "google/" + model.removeprefix("gemini/")
        return GATEWAY_PREFIX + model
    return model


def complete(model: str, system: str, user: str, temperature: float = 0.0) -> str:
    """Single chat completion via litellm. The one seam tests mock."""
    resp = litellm.completion(
        model=_route(model),
        messages=[
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
        temperature=temperature,
    )
    return resp["choices"][0]["message"]["content"]
