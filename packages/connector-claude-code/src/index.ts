#!/usr/bin/env node

import { startClaudeCodeConnector } from "./claude-code-agent.js";

function printHelp(): void {
  console.log(`
Usage: connector-claude-code [options]

Connect a Claude Code agent to an AI-Native Messaging server.

Options:
  --server <url>            WebSocket server URL (default: wss://localhost:8080/ws)
  --identity <id>           Agent identity string (default: claude-code@localhost)
  --name <name>             Display name for the agent (default: Claude Code)
  --working-dir <path>      Working directory for Claude Code (default: cwd)
  --cwd <path>              Alias for --working-dir
  --claude-binary <path>    Path to claude binary (default: claude)
  --session-id <id>         Resume an existing Claude Code session
  --resume <id>             Alias for --session-id
  --permission-mode <mode>  Claude Code permission mode (default: auto)
  --max-budget-usd <amt>    Maximum spend budget in USD (e.g. 5.00)
  --bare                    Run in bare mode (no interactive prompts)
  --help                    Show this help message and exit
`);
}

function parseArgs(): Record<string, string> {
  const args: Record<string, string> = {};
  const argv = process.argv.slice(2);

  for (let i = 0; i < argv.length; i++) {
    if (argv[i].startsWith("--")) {
      const key = argv[i].slice(2);
      const value =
        argv[i + 1] && !argv[i + 1].startsWith("--") ? argv[++i] : "true";
      args[key] = value;
    }
  }

  return args;
}

const args = parseArgs();

if (args["help"] === "true") {
  printHelp();
  process.exit(0);
}

const maxBudgetRaw = args["max-budget-usd"];
const maxBudgetUsd =
  maxBudgetRaw !== undefined ? parseFloat(maxBudgetRaw) : undefined;

startClaudeCodeConnector({
  serverUrl: args["server"] ?? "wss://localhost:8080/ws",
  identity: args["identity"] ?? "claude-code@localhost",
  name: args["name"] ?? "Claude Code",
  workingDir: args["working-dir"] ?? args["cwd"],
  claudeBinary: args["claude-binary"] ?? "claude",
  sessionId: args["session-id"] ?? args["resume"],
  permissionMode: args["permission-mode"],
  maxBudgetUsd,
  bare: args["bare"] === "true",
}).catch((err: unknown) => {
  const message = err instanceof Error ? err.message : String(err);
  console.error(`Error: ${message}`);
  process.exit(1);
});
