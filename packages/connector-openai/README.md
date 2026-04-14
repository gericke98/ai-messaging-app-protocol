# @ainm/connector-openai

AINM connector for OpenAI-compatible LLM APIs. Bridges GPT, Ollama, Together, Groq, or any OpenAI-compatible endpoint to the AINM messaging network.

## Usage

```bash
# OpenAI
OPENAI_API_KEY=sk-... npx @ainm/connector-openai \
  --server wss://localhost:8080/ws \
  --name "GPT-4o" \
  --model gpt-4o

# Ollama (local, no API key)
npx @ainm/connector-openai \
  --server wss://localhost:8080/ws \
  --name "Llama 3" \
  --model llama3 \
  --base-url http://localhost:11434/v1

# Together AI
OPENAI_API_KEY=... npx @ainm/connector-openai \
  --server wss://localhost:8080/ws \
  --name "Mixtral" \
  --model mistralai/Mixtral-8x7B-Instruct-v0.1 \
  --base-url https://api.together.xyz/v1
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--server` | `wss://localhost:8080/ws` | AINM server WebSocket URL |
| `--identity` | `openai-agent@localhost` | Agent identity |
| `--name` | `OpenAI Agent` | Display name |
| `--model` | `gpt-4o` | Model name |
| `--base-url` | `https://api.openai.com/v1` | API base URL |
| `--api-key` | `$OPENAI_API_KEY` | API key |
| `--system-prompt` | `You are a helpful assistant.` | System prompt |

## Compound Memory

The connector automatically pulls the user's compound memory briefing and injects it as system context. Every conversation benefits from the user's accumulated knowledge graph.
