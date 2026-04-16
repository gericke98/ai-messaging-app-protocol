/**
 * E2E test: Claude Code connector against live AINM server + real Claude Code CLI.
 *
 * Requires:
 *   - AINM server running at localhost:8080 with PostgreSQL
 *   - Claude Code CLI installed and authenticated
 *
 * Run: npx vitest run tests/e2e-server.test.ts
 *
 * Tests:
 *   1. Human sends prompt → connector spawns Claude Code → human receives streamed response
 *   2. Verifies the full streaming lifecycle: ainm.stream.start, delta, end
 */

import { describe, it, expect, beforeAll } from "vitest";
import WebSocket from "ws";
import { spawn } from "node:child_process";
import { startClaudeCodeConnector } from "../src/claude-code-agent.js";

const SERVER_URL_WS = "ws://localhost:8080/ws";
const HEALTH_URL = "http://localhost:8080/health/live";

// ── Availability checks ──────────────────────────────────────────────────────

async function serverAvailable(): Promise<boolean> {
  try {
    const res = await fetch(HEALTH_URL);
    return res.ok;
  } catch {
    return false;
  }
}

async function claudeCliAvailable(): Promise<boolean> {
  return new Promise((resolve) => {
    const child = spawn("claude", ["--version"], {
      stdio: ["ignore", "pipe", "pipe"],
    });
    const timer = setTimeout(() => {
      child.kill();
      resolve(false);
    }, 5000);
    child.on("error", () => {
      clearTimeout(timer);
      resolve(false);
    });
    child.on("close", (code: number | null) => {
      clearTimeout(timer);
      resolve(code === 0);
    });
  });
}

// ── Raw WebSocket helper for simulating a human client ───────────────────────

class HumanClient {
  private ws: WebSocket | null = null;
  private messages: string[] = [];
  private messageResolvers: Array<(msg: string) => void> = [];

  async connect(url: string, identity: string): Promise<void> {
    return new Promise((resolve, reject) => {
      this.ws = new WebSocket(url);
      this.ws.on("open", () => {
        this.ws!.send(
          JSON.stringify({
            jsonrpc: "2.0",
            id: 1,
            method: "ainm.initialize",
            params: {
              protocolVersion: "0.1.0",
              identity,
              signature: "test-sig",
              capabilities: {
                streaming: true,
                contentTypes: ["text"],
                participant_type: "human",
              },
            },
          })
        );
      });

      this.ws.on("message", (data) => {
        const msg = data.toString();
        try {
          const parsed = JSON.parse(msg);
          if (parsed.id === 1 && parsed.result) {
            this.ws!.send(
              JSON.stringify({
                jsonrpc: "2.0",
                method: "ainm.initialized",
                params: {},
              })
            );
            resolve();
            return;
          }
        } catch {
          // fall through to queue
        }
        if (this.messageResolvers.length > 0) {
          this.messageResolvers.shift()!(msg);
        } else {
          this.messages.push(msg);
        }
      });

      this.ws.on("error", reject);
    });
  }

  sendMessage(threadId: string, text: string, identity: string): void {
    this.ws!.send(
      JSON.stringify({
        jsonrpc: "2.0",
        id: Date.now(),
        method: "ainm.message.send",
        params: {
          id: `msg-${Date.now()}`,
          from: identity,
          thread_id: threadId,
          timestamp: new Date().toISOString(),
          type: "message",
          content: [
            {
              type: "text",
              version: "1.0",
              source: "user",
              text,
              fallback: text,
            },
          ],
          signature: "test-sig",
        },
      })
    );
  }

  async waitForMessage(timeoutMs = 5000): Promise<Record<string, unknown>> {
    if (this.messages.length > 0) {
      return JSON.parse(this.messages.shift()!);
    }
    return new Promise((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error("Timeout waiting for message")),
        timeoutMs
      );
      this.messageResolvers.push((msg) => {
        clearTimeout(timer);
        resolve(JSON.parse(msg));
      });
    });
  }

  /** Collect all messages received within the given time window. */
  async collectMessages(
    durationMs: number
  ): Promise<Array<Record<string, unknown>>> {
    const collected: Array<Record<string, unknown>> = [];
    const deadline = Date.now() + durationMs;
    while (Date.now() < deadline) {
      try {
        const remaining = deadline - Date.now();
        if (remaining <= 0) break;
        const msg = await this.waitForMessage(remaining);
        collected.push(msg);
      } catch {
        break; // timeout — stop collecting
      }
    }
    return collected;
  }

  close(): void {
    this.ws?.close();
  }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("E2E: Claude Code Connector ↔ AINM Server + Real Claude Code", () => {
  let serverOk = false;
  let cliOk = false;

  beforeAll(async () => {
    serverOk = await serverAvailable();
    cliOk = await claudeCliAvailable();
    if (!serverOk) {
      console.warn(
        "⚠ AINM server not available at localhost:8080, skipping E2E tests"
      );
    }
    if (!cliOk) {
      console.warn(
        "⚠ Claude Code CLI not available or not authenticated, skipping E2E tests"
      );
    }
  });

  it("human → connector → real Claude Code → streamed response", async () => {
    if (!serverOk || !cliOk) return;

    const handle = await startClaudeCodeConnector({
      serverUrl: SERVER_URL_WS,
      identity: "e2e-claude-code@server-a.local",
      name: "E2E Claude Code",
      workingDir: "/tmp",
      permissionMode: "auto",
      maxBudgetUsd: 0.5,
      bare: true,
    });
    console.log("[E2E] Claude Code connector started");

    try {
      const human = new HumanClient();
      await human.connect(SERVER_URL_WS, "e2e-human@server-a.local");
      console.log("[E2E] Human connected");

      const threadId = `e2e-claude-thread-${Date.now()}`;
      human.sendMessage(
        threadId,
        "What is 2+2? Reply with just the number.",
        "e2e-human@server-a.local"
      );
      console.log("[E2E] Human sent prompt");

      // Claude Code CLI startup is slow — give it 45s to complete the streamed response
      const messages = await human.collectMessages(45_000);
      console.log(`[E2E] Human received ${messages.length} messages`);

      const methods = messages
        .map((m) => m.method as string | undefined)
        .filter((m): m is string => typeof m === "string");

      expect(methods).toContain("ainm.stream.start");
      expect(methods).toContain("ainm.stream.delta");
      expect(methods).toContain("ainm.stream.end");

      // Collect all delta text to verify "4" appears somewhere
      const deltaText = messages
        .filter((m) => m.method === "ainm.stream.delta")
        .map((m) => {
          const params = m.params as Record<string, unknown> | undefined;
          return (params?.delta as string) ?? "";
        })
        .join("");

      // Also check the collapsed message in stream.end if available
      const endMsg = messages.find((m) => m.method === "ainm.stream.end");
      const collapsedText = (() => {
        const params = endMsg?.params as Record<string, unknown> | undefined;
        const collapsed = params?.collapsed_message as
          | Record<string, unknown>
          | undefined;
        const content = collapsed?.content as
          | Array<{ text?: string }>
          | undefined;
        return content?.map((c) => c.text ?? "").join("") ?? "";
      })();

      const fullText = deltaText + collapsedText;
      console.log(`[E2E] Full streamed text: "${fullText}"`);
      expect(fullText).toContain("4");

      human.close();
    } finally {
      handle.stop();
    }
  }, 60_000);
});
