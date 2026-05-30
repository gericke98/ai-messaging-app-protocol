from dotenv import load_dotenv
import litellm

load_dotenv()

def complete(model: str, system: str, user: str, temperature: float = 0.0) -> str:
    """Single chat completion via litellm. The one seam tests mock."""
    resp = litellm.completion(
        model=model,
        messages=[
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
        temperature=temperature,
    )
    return resp["choices"][0]["message"]["content"]
