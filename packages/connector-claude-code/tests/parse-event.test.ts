import { describe, it, expect } from "vitest";
import {
  parseClaudeEvent,
  extractTextContent,
  extractToolUses,
  type ClaudeSystemEvent,
  type ClaudeAssistantEvent,
  type ClaudeResultEvent,
  type ClaudeCodeEvent,
} from "../src/parse-event.js";

// ── Fixture data matching real CLI output (v2.1.110) ──────────────────────────

const SYSTEM_INIT_LINE = JSON.stringify({
  type: "system",
  subtype: "init",
  cwd: "/path/to/project",
  session_id: "55058237-83e5-492c-aea4-69b46794d5f7",
  tools: ["Bash", "Edit", "Read"],
  mcp_servers: [],
  model: "claude-sonnet-4-6",
  permissionMode: "default",
  slash_commands: [],
  apiKeySource: "none",
  claude_code_version: "2.1.110",
  output_style: "default",
  agents: [],
  skills: [],
  plugins: [],
  uuid: "some-uuid",
  fast_mode_state: "off",
});

const ASSISTANT_TEXT_LINE = JSON.stringify({
  type: "assistant",
  message: {
    id: "6fd5cf1f-9523-49ab-8107-0fd4f3f6a89a",
    container: null,
    model: "<synthetic>",
    role: "assistant",
    stop_reason: "stop_sequence",
    stop_sequence: "",
    type: "message",
    usage: { input_tokens: 0, output_tokens: 0 },
    content: [{ type: "text", text: "Hello world" }],
    context_management: null,
  },
  parent_tool_use_id: null,
  session_id: "55058237-83e5-492c-aea4-69b46794d5f7",
  uuid: "another-uuid",
  error: "authentication_failed",
});

const ASSISTANT_TOOL_USE_LINE = JSON.stringify({
  type: "assistant",
  message: {
    id: "msg-tool-123",
    role: "assistant",
    content: [
      {
        type: "tool_use",
        id: "toolu_123",
        name: "Bash",
        input: { command: "ls -la" },
      },
    ],
  },
  session_id: "55058237-83e5-492c-aea4-69b46794d5f7",
});

const ASSISTANT_MIXED_LINE = JSON.stringify({
  type: "assistant",
  message: {
    id: "msg-mixed-456",
    role: "assistant",
    content: [
      { type: "text", text: "I'll run a command for you." },
      {
        type: "tool_use",
        id: "toolu_456",
        name: "Edit",
        input: { file_path: "/foo/bar.ts", old_string: "old", new_string: "new" },
      },
      { type: "text", text: " Done." },
    ],
  },
  session_id: "55058237-83e5-492c-aea4-69b46794d5f7",
});

const RESULT_SUCCESS_LINE = JSON.stringify({
  type: "result",
  subtype: "success",
  is_error: false,
  api_error_status: null,
  duration_ms: 58,
  duration_api_ms: 0,
  num_turns: 1,
  result: "Hello world",
  stop_reason: "stop_sequence",
  session_id: "55058237-83e5-492c-aea4-69b46794d5f7",
  total_cost_usd: 0.01,
  usage: { input_tokens: 100, output_tokens: 50 },
});

const RESULT_ERROR_LINE = JSON.stringify({
  type: "result",
  subtype: "success",
  is_error: true,
  result: "Not logged in",
  session_id: "55058237-83e5-492c-aea4-69b46794d5f7",
  total_cost_usd: 0,
});

// ── parseClaudeEvent ──────────────────────────────────────────────────────────

describe("parseClaudeEvent", () => {
  describe("system/init events", () => {
    it("parses a system init event and extracts session_id, model, tools, cwd", () => {
      const result = parseClaudeEvent(SYSTEM_INIT_LINE);
      expect(result).not.toBeNull();
      expect(result!.type).toBe("system");
      const ev = result as ClaudeSystemEvent;
      expect(ev.subtype).toBe("init");
      expect(ev.session_id).toBe("55058237-83e5-492c-aea4-69b46794d5f7");
      expect(ev.model).toBe("claude-sonnet-4-6");
      expect(ev.tools).toEqual(["Bash", "Edit", "Read"]);
      expect(ev.cwd).toBe("/path/to/project");
    });
  });

  describe("assistant events with text content", () => {
    it("parses an assistant event with text content block", () => {
      const result = parseClaudeEvent(ASSISTANT_TEXT_LINE);
      expect(result).not.toBeNull();
      expect(result!.type).toBe("assistant");
      const ev = result as ClaudeAssistantEvent;
      expect(ev.session_id).toBe("55058237-83e5-492c-aea4-69b46794d5f7");
      expect(ev.message.id).toBe("6fd5cf1f-9523-49ab-8107-0fd4f3f6a89a");
      expect(ev.message.content).toHaveLength(1);
      expect(ev.message.content[0].type).toBe("text");
      expect(ev.message.content[0].text).toBe("Hello world");
    });
  });

  describe("assistant events with tool_use", () => {
    it("parses an assistant event with a single tool_use block", () => {
      const result = parseClaudeEvent(ASSISTANT_TOOL_USE_LINE);
      expect(result).not.toBeNull();
      expect(result!.type).toBe("assistant");
      const ev = result as ClaudeAssistantEvent;
      expect(ev.message.content).toHaveLength(1);
      const block = ev.message.content[0];
      expect(block.type).toBe("tool_use");
      expect(block.id).toBe("toolu_123");
      expect(block.name).toBe("Bash");
      expect(block.input).toEqual({ command: "ls -la" });
    });
  });

  describe("assistant events with mixed content", () => {
    it("parses an assistant event with both text and tool_use blocks", () => {
      const result = parseClaudeEvent(ASSISTANT_MIXED_LINE);
      expect(result).not.toBeNull();
      const ev = result as ClaudeAssistantEvent;
      expect(ev.message.content).toHaveLength(3);
      expect(ev.message.content[0].type).toBe("text");
      expect(ev.message.content[1].type).toBe("tool_use");
      expect(ev.message.content[2].type).toBe("text");
    });
  });

  describe("result/success events", () => {
    it("parses a result success event and extracts session_id, result text, cost", () => {
      const result = parseClaudeEvent(RESULT_SUCCESS_LINE);
      expect(result).not.toBeNull();
      expect(result!.type).toBe("result");
      const ev = result as ClaudeResultEvent;
      expect(ev.subtype).toBe("success");
      expect(ev.is_error).toBe(false);
      expect(ev.result).toBe("Hello world");
      expect(ev.session_id).toBe("55058237-83e5-492c-aea4-69b46794d5f7");
      expect(ev.total_cost_usd).toBeCloseTo(0.01);
    });
  });

  describe("result with is_error: true", () => {
    it("parses an error result and flags is_error as true", () => {
      const result = parseClaudeEvent(RESULT_ERROR_LINE);
      expect(result).not.toBeNull();
      const ev = result as ClaudeResultEvent;
      expect(ev.type).toBe("result");
      expect(ev.is_error).toBe(true);
      expect(ev.result).toBe("Not logged in");
      expect(ev.total_cost_usd).toBe(0);
    });
  });

  describe("null / invalid inputs", () => {
    it("returns null for an empty string", () => {
      expect(parseClaudeEvent("")).toBeNull();
    });

    it("returns null for a whitespace-only string", () => {
      expect(parseClaudeEvent("   ")).toBeNull();
    });

    it("returns null for non-JSON input", () => {
      expect(parseClaudeEvent("not json at all")).toBeNull();
    });

    it("returns null for JSON that is not an object", () => {
      expect(parseClaudeEvent("42")).toBeNull();
      expect(parseClaudeEvent('"hello"')).toBeNull();
      expect(parseClaudeEvent("null")).toBeNull();
    });

    it("returns null for an unknown event type", () => {
      expect(
        parseClaudeEvent(JSON.stringify({ type: "unknown_event_xyz" }))
      ).toBeNull();
    });

    it("returns null for a system event missing required fields", () => {
      // Missing session_id, model, tools
      expect(
        parseClaudeEvent(JSON.stringify({ type: "system", subtype: "init" }))
      ).toBeNull();
    });

    it("returns null for an assistant event missing message", () => {
      expect(
        parseClaudeEvent(
          JSON.stringify({ type: "assistant", session_id: "abc" })
        )
      ).toBeNull();
    });

    it("returns null for a result event missing required fields", () => {
      expect(
        parseClaudeEvent(JSON.stringify({ type: "result" }))
      ).toBeNull();
    });
  });
});

// ── extractTextContent ────────────────────────────────────────────────────────

describe("extractTextContent", () => {
  it("returns text from a single text block", () => {
    const ev = parseClaudeEvent(ASSISTANT_TEXT_LINE) as ClaudeAssistantEvent;
    expect(extractTextContent(ev)).toBe("Hello world");
  });

  it("joins multiple text blocks separated by nothing (concatenated)", () => {
    const ev = parseClaudeEvent(ASSISTANT_MIXED_LINE) as ClaudeAssistantEvent;
    // two text blocks: "I'll run a command for you." and " Done."
    expect(extractTextContent(ev)).toBe("I'll run a command for you. Done.");
  });

  it("returns empty string when there are no text blocks", () => {
    const ev = parseClaudeEvent(ASSISTANT_TOOL_USE_LINE) as ClaudeAssistantEvent;
    expect(extractTextContent(ev)).toBe("");
  });
});

// ── extractToolUses ───────────────────────────────────────────────────────────

describe("extractToolUses", () => {
  it("returns an empty array when there are no tool_use blocks", () => {
    const ev = parseClaudeEvent(ASSISTANT_TEXT_LINE) as ClaudeAssistantEvent;
    expect(extractToolUses(ev)).toEqual([]);
  });

  it("extracts a single tool_use with id, name, and input", () => {
    const ev = parseClaudeEvent(ASSISTANT_TOOL_USE_LINE) as ClaudeAssistantEvent;
    const tools = extractToolUses(ev);
    expect(tools).toHaveLength(1);
    expect(tools[0]).toEqual({
      id: "toolu_123",
      name: "Bash",
      input: { command: "ls -la" },
    });
  });

  it("extracts tool_use blocks from a mixed content event", () => {
    const ev = parseClaudeEvent(ASSISTANT_MIXED_LINE) as ClaudeAssistantEvent;
    const tools = extractToolUses(ev);
    expect(tools).toHaveLength(1);
    expect(tools[0].id).toBe("toolu_456");
    expect(tools[0].name).toBe("Edit");
    expect(tools[0].input).toEqual({
      file_path: "/foo/bar.ts",
      old_string: "old",
      new_string: "new",
    });
  });
});
