#!/usr/bin/env node

import { startOpenAIConnector } from "./openai-agent.js";

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

const serverUrl = args["server"] ?? "wss://localhost:8080/ws";
const identity = args["identity"] ?? "openai-agent@localhost";
const name = args["name"] ?? "OpenAI Agent";
const model = args["model"] ?? "gpt-4o";
const baseUrl = args["base-url"] ?? "https://api.openai.com/v1";
const apiKey = args["api-key"] ?? process.env["OPENAI_API_KEY"] ?? "";
const systemPrompt = args["system-prompt"];

// Require an API key unless targeting a local endpoint (e.g., Ollama)
if (!apiKey && !baseUrl.includes("localhost")) {
  console.error(
    "Error: OPENAI_API_KEY environment variable or --api-key required"
  );
  process.exit(1);
}

startOpenAIConnector({
  serverUrl,
  identity,
  name,
  model,
  baseUrl,
  apiKey,
  systemPrompt,
}).catch((err: unknown) => {
  console.error("Fatal:", err);
  process.exit(1);
});
