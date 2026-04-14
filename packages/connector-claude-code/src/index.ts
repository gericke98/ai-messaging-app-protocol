#!/usr/bin/env node

import { startClaudeCodeConnector } from "./claude-code-agent.js";

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

startClaudeCodeConnector({
  serverUrl: args["server"] ?? "wss://localhost:8080/ws",
  identity: args["identity"] ?? "claude-code@localhost",
  name: args["name"] ?? "Claude Code",
  workingDir: args["working-dir"] ?? args["cwd"],
  claudeBinary: args["claude-binary"] ?? "claude",
  sessionId: args["session-id"] ?? args["resume"],
}).catch((err: unknown) => {
  console.error("Fatal:", err);
  process.exit(1);
});
