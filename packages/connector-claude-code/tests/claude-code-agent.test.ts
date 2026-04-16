/**
 * Integration tests for the Claude Code agent event-processing logic.
 *
 * Rather than testing `startClaudeCodeConnector` end-to-end (which requires
 * mocking AINMAgent, WebSocket, checkClaudeAvailable, and spawn), these tests
 * extract and exercise the core line-processing loop in isolation by:
 *
 *   1. Building a minimal mock MessageContext with spied stream/tool methods.
 *   2. Running NDJSON lines through the same parseClaudeEvent + delta-tracking
 *      logic used inside the rl.on("line") handler.
 *   3. Asserting that stream.delta / stream.end / emitToolStarted are called
 *      with the correct arguments.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  parseClaudeEvent,
  extractTextContent,
  extractToolUses,
  type ClaudeAssistantEvent,
  type ClaudeResultEvent,
  type ClaudeSystemEvent,
} from "../src/parse-event.js";

// ── Minimal mock context ──────────────────────────────────────────────────────

interface MockStream {
  delta: ReturnType<typeof vi.fn>;
  end: ReturnType<typeof vi.fn>;
}

interface MockCtx {
  stream: () => MockStream;
  emitToolStarted: ReturnType<typeof vi.fn>;
  emitToolCompleted: ReturnType<typeof vi.fn>;
}

function makeMockCtx(): { ctx: MockCtx; mockStream: MockStream } {
  const mockStream: MockStream = {
    delta: vi.fn(),
    end: vi.fn(),
  };
  const ctx: MockCtx = {
    stream: () => mockStream,
    emitToolStarted: vi.fn(),
    emitToolCompleted: vi.fn(),
  };
  return { ctx, mockStream };
}

// ── Core processor ────────────────────────────────────────────────────────────

/**
 * Simulates the rl.on("line") handler from claude-code-agent.ts.
 *
 * Returns a summary of what happened so tests can assert on session IDs and
 * the terminal state without reaching into mutable closure variables.
 */
function processLines(
  lines: string[],
  ctx: MockCtx
): { sessionId: string | undefined; resolvedWith: void | null; rejectedWith: Error | null } {
  const stream = ctx.stream();
  let fullText = "";
  let lastTextLength = 0;
  let currentSessionId: string | undefined;
  let resolvedWith: void | null = null;
  let rejectedWith: Error | null = null;

  for (const line of lines) {
    const event = parseClaudeEvent(line);
    if (event === null) continue;

    if (event.type === "system") {
      const sysEv = event as ClaudeSystemEvent;
      currentSessionId = sysEv.session_id;
      continue;
    }

    if (event.type === "assistant") {
      const assistantEv = event as ClaudeAssistantEvent;
      const currentText = extractTextContent(assistantEv);
      if (currentText.length > lastTextLength) {
        const delta = currentText.slice(lastTextLength);
        stream.delta(delta);
        fullText = currentText;
        lastTextLength = currentText.length;
      }
      const toolUses = extractToolUses(assistantEv);
      for (const tool of toolUses) {
        ctx.emitToolStarted(tool.name, tool.input);
      }
      continue;
    }

    if (event.type === "result") {
      const resultEv = event as ClaudeResultEvent;
      currentSessionId = resultEv.session_id;
      if (resultEv.is_error) {
        stream.end();
        rejectedWith = new Error(resultEv.result);
        break;
      }
      stream.end();
      resolvedWith = undefined;
      break;
    }
  }

  return { sessionId: currentSessionId, resolvedWith, rejectedWith };
}

// ── Test suites ───────────────────────────────────────────────────────────────

describe("claude-code-agent event processing", () => {
  let ctx: MockCtx;
  let mockStream: MockStream;

  beforeEach(() => {
    ({ ctx, mockStream } = makeMockCtx());
  });

  // ── 1. Happy path ───────────────────────────────────────────────────────────

  describe("happy path — incremental text accumulation", () => {
    const lines = [
      JSON.stringify({
        type: "system",
        subtype: "init",
        session_id: "sess-1",
        model: "claude-sonnet-4-6",
        tools: ["Bash", "Edit", "Read"],
        cwd: "/project",
      }),
      JSON.stringify({
        type: "assistant",
        message: { id: "msg-1", content: [{ type: "text", text: "Hello" }] },
        session_id: "sess-1",
      }),
      JSON.stringify({
        type: "assistant",
        message: { id: "msg-2", content: [{ type: "text", text: "Hello world" }] },
        session_id: "sess-1",
      }),
      JSON.stringify({
        type: "result",
        subtype: "success",
        is_error: false,
        result: "Hello world",
        session_id: "sess-1",
        total_cost_usd: 0.01,
      }),
    ];

    it("emits 'Hello' on the first assistant event", () => {
      processLines(lines, ctx);
      expect(mockStream.delta).toHaveBeenNthCalledWith(1, "Hello");
    });

    it("emits only the incremental delta ' world' on the second assistant event", () => {
      processLines(lines, ctx);
      expect(mockStream.delta).toHaveBeenNthCalledWith(2, " world");
    });

    it("calls stream.end() exactly once", () => {
      processLines(lines, ctx);
      expect(mockStream.end).toHaveBeenCalledTimes(1);
    });

    it("captures session_id from both the system event and result event", () => {
      const { sessionId } = processLines(lines, ctx);
      expect(sessionId).toBe("sess-1");
    });

    it("resolves (no error) for a successful result", () => {
      const { rejectedWith } = processLines(lines, ctx);
      expect(rejectedWith).toBeNull();
    });

    it("does NOT call emitToolStarted for a plain text response", () => {
      processLines(lines, ctx);
      expect(ctx.emitToolStarted).not.toHaveBeenCalled();
    });
  });

  // ── 2. Tool use ─────────────────────────────────────────────────────────────

  describe("tool use — emitToolStarted called with correct name and input", () => {
    const lines = [
      JSON.stringify({
        type: "system",
        subtype: "init",
        session_id: "sess-2",
        model: "claude-sonnet-4-6",
        tools: ["Bash"],
        cwd: "/project",
      }),
      JSON.stringify({
        type: "assistant",
        message: {
          id: "msg-1",
          content: [
            { type: "text", text: "Let me check." },
            { type: "tool_use", id: "toolu_1", name: "Bash", input: { command: "ls -la" } },
          ],
        },
        session_id: "sess-2",
      }),
      JSON.stringify({
        type: "result",
        subtype: "success",
        is_error: false,
        result: "Let me check.",
        session_id: "sess-2",
        total_cost_usd: 0.02,
      }),
    ];

    it("emits stream.delta for the text portion of a mixed-content event", () => {
      processLines(lines, ctx);
      expect(mockStream.delta).toHaveBeenCalledWith("Let me check.");
    });

    it("calls emitToolStarted with the correct tool name", () => {
      processLines(lines, ctx);
      expect(ctx.emitToolStarted).toHaveBeenCalledWith("Bash", { command: "ls -la" });
    });

    it("calls emitToolStarted exactly once", () => {
      processLines(lines, ctx);
      expect(ctx.emitToolStarted).toHaveBeenCalledTimes(1);
    });

    it("calls stream.end() after the result event", () => {
      processLines(lines, ctx);
      expect(mockStream.end).toHaveBeenCalledTimes(1);
    });

    it("correctly captures session_id from the tool-use sequence", () => {
      const { sessionId } = processLines(lines, ctx);
      expect(sessionId).toBe("sess-2");
    });
  });

  // ── 3. Error result (is_error: true) ────────────────────────────────────────

  describe("error result — process exits with is_error: true", () => {
    const lines = [
      JSON.stringify({
        type: "system",
        subtype: "init",
        session_id: "sess-3",
        model: "claude-sonnet-4-6",
        tools: [],
        cwd: "/project",
      }),
      JSON.stringify({
        type: "result",
        subtype: "success",
        is_error: true,
        result: "Authentication failed",
        session_id: "sess-3",
        total_cost_usd: 0,
      }),
    ];

    it("rejects with an Error whose message matches the result text", () => {
      const { rejectedWith } = processLines(lines, ctx);
      expect(rejectedWith).toBeInstanceOf(Error);
      expect(rejectedWith!.message).toBe("Authentication failed");
    });

    it("calls stream.end() even on an error result", () => {
      processLines(lines, ctx);
      expect(mockStream.end).toHaveBeenCalledTimes(1);
    });

    it("does NOT emit any stream.delta before the error", () => {
      processLines(lines, ctx);
      expect(mockStream.delta).not.toHaveBeenCalled();
    });

    it("does NOT call emitToolStarted on an error result", () => {
      processLines(lines, ctx);
      expect(ctx.emitToolStarted).not.toHaveBeenCalled();
    });
  });

  // ── 4. Malformed JSON mixed with valid events ────────────────────────────────

  describe("malformed JSON — bad lines are skipped, valid events processed", () => {
    const lines = [
      "not json at all",
      JSON.stringify({
        type: "system",
        subtype: "init",
        session_id: "sess-4",
        model: "claude-sonnet-4-6",
        tools: [],
        cwd: "/project",
      }),
      "{malformed",
      JSON.stringify({
        type: "assistant",
        message: { id: "msg-1", content: [{ type: "text", text: "OK" }] },
        session_id: "sess-4",
      }),
      "",
      JSON.stringify({
        type: "result",
        subtype: "success",
        is_error: false,
        result: "OK",
        session_id: "sess-4",
        total_cost_usd: 0.01,
      }),
    ];

    it("emits stream.delta('OK') from the valid assistant event", () => {
      processLines(lines, ctx);
      expect(mockStream.delta).toHaveBeenCalledWith("OK");
    });

    it("calls stream.end() at the valid result event", () => {
      processLines(lines, ctx);
      expect(mockStream.end).toHaveBeenCalledTimes(1);
    });

    it("does not reject (no error)", () => {
      const { rejectedWith } = processLines(lines, ctx);
      expect(rejectedWith).toBeNull();
    });

    it("still captures the correct session_id despite surrounding bad lines", () => {
      const { sessionId } = processLines(lines, ctx);
      expect(sessionId).toBe("sess-4");
    });

    it("calls stream.delta exactly once — no phantom calls from bad lines", () => {
      processLines(lines, ctx);
      expect(mockStream.delta).toHaveBeenCalledTimes(1);
    });
  });
});
