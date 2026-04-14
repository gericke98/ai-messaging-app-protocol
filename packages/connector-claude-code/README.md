# @ainm/connector-claude-code

AINM connector for Claude Code -- message your coding session from your phone.

## Prerequisites

- [Claude Code](https://claude.ai/code) installed and authenticated
- An AINM server running

## Usage

```bash
# Start in current project directory
npx @ainm/connector-claude-code --server wss://localhost:8080/ws

# Specify working directory
npx @ainm/connector-claude-code \
  --server wss://localhost:8080/ws \
  --working-dir /path/to/project

# Resume an existing session
npx @ainm/connector-claude-code \
  --server wss://localhost:8080/ws \
  --session-id abc123
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--server` | `wss://localhost:8080/ws` | AINM server URL |
| `--identity` | `claude-code@localhost` | Agent identity |
| `--name` | `Claude Code` | Display name |
| `--working-dir` | Current directory | Project directory |
| `--claude-binary` | `claude` | Path to claude CLI |
| `--session-id` | (none) | Resume an existing session |

## How It Works

The connector spawns `claude -p` in headless mode for each message, parses the NDJSON stream output, and relays tokens to the AINM server as streaming events. Tool calls (file edits, bash commands) are forwarded as AINM tool visibility events so you can see what Claude Code is doing from your phone.

Compound memory briefings are automatically injected as context -- Claude Code knows your preferences, tech stack, and recent work without you re-explaining.
