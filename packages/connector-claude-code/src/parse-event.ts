/**
 * Type definitions and parser for Claude Code CLI stream-json output.
 *
 * The CLI is invoked as: claude -p <prompt> --output-format stream-json --verbose
 * It emits NDJSON lines. This module defines accurate discriminated union types
 * matching the real v2.1.110 output format and provides lenient parsing helpers.
 */

// ── Content block types ───────────────────────────────────────────────────────

export interface TextContentBlock {
  type: "text";
  text: string;
}

export interface ToolUseContentBlock {
  type: "tool_use";
  id: string;
  name: string;
  input: Record<string, unknown>;
}

export type ContentBlock = TextContentBlock | ToolUseContentBlock | { type: string; text?: string; name?: string; id?: string; input?: Record<string, unknown> };

// ── Event types ───────────────────────────────────────────────────────────────

export type ClaudeSystemEvent = {
  type: "system";
  subtype: "init";
  session_id: string;
  model: string;
  tools: string[];
  cwd: string;
};

export type ClaudeAssistantEvent = {
  type: "assistant";
  message: {
    id: string;
    content: Array<{
      type: string;
      text?: string;
      name?: string;
      id?: string;
      input?: Record<string, unknown>;
    }>;
  };
  session_id: string;
};

export type ClaudeResultEvent = {
  type: "result";
  subtype: string;
  is_error: boolean;
  result: string;
  session_id: string;
  total_cost_usd: number;
};

export type ClaudeCodeEvent =
  | ClaudeSystemEvent
  | ClaudeAssistantEvent
  | ClaudeResultEvent;

// ── Parsing ───────────────────────────────────────────────────────────────────

/**
 * Parse a single NDJSON line from the Claude Code CLI stream output.
 *
 * Returns null for:
 * - Empty or whitespace-only strings
 * - Non-JSON or non-object JSON
 * - Unknown event types
 * - Events missing required fields for their type
 *
 * Never throws.
 */
export function parseClaudeEvent(line: string): ClaudeCodeEvent | null {
  const trimmed = line.trim();
  if (!trimmed) return null;

  let raw: unknown;
  try {
    raw = JSON.parse(trimmed);
  } catch {
    return null;
  }

  if (raw === null || typeof raw !== "object" || Array.isArray(raw)) {
    return null;
  }

  const obj = raw as Record<string, unknown>;
  const type = obj["type"];

  if (type === "system") {
    return parseSystemEvent(obj);
  }
  if (type === "assistant") {
    return parseAssistantEvent(obj);
  }
  if (type === "result") {
    return parseResultEvent(obj);
  }

  return null;
}

function parseSystemEvent(obj: Record<string, unknown>): ClaudeSystemEvent | null {
  const { subtype, session_id, model, tools, cwd } = obj;

  if (
    subtype !== "init" ||
    typeof session_id !== "string" ||
    typeof model !== "string" ||
    typeof cwd !== "string" ||
    !Array.isArray(tools) ||
    !tools.every((t): t is string => typeof t === "string")
  ) {
    return null;
  }

  return {
    type: "system",
    subtype: "init",
    session_id,
    model,
    tools,
    cwd,
  };
}

function parseAssistantEvent(obj: Record<string, unknown>): ClaudeAssistantEvent | null {
  const { session_id, message } = obj;

  if (typeof session_id !== "string") return null;
  if (message === null || typeof message !== "object" || Array.isArray(message)) return null;

  const msg = message as Record<string, unknown>;
  const { id, content } = msg;

  if (typeof id !== "string") return null;
  if (!Array.isArray(content)) return null;

  // Validate each content block has at least a type string
  const typedContent = content.filter(
    (block): block is Record<string, unknown> =>
      block !== null && typeof block === "object" && !Array.isArray(block) && typeof (block as Record<string, unknown>)["type"] === "string"
  );

  if (typedContent.length !== content.length) return null;

  return {
    type: "assistant",
    message: {
      id,
      content: typedContent.map((block) => ({
        type: block["type"] as string,
        ...(typeof block["text"] === "string" ? { text: block["text"] } : {}),
        ...(typeof block["name"] === "string" ? { name: block["name"] } : {}),
        ...(typeof block["id"] === "string" ? { id: block["id"] } : {}),
        ...(block["input"] !== undefined &&
        block["input"] !== null &&
        typeof block["input"] === "object" &&
        !Array.isArray(block["input"])
          ? { input: block["input"] as Record<string, unknown> }
          : {}),
      })),
    },
    session_id,
  };
}

function parseResultEvent(obj: Record<string, unknown>): ClaudeResultEvent | null {
  const { subtype, is_error, result, session_id, total_cost_usd } = obj;

  if (
    typeof subtype !== "string" ||
    typeof is_error !== "boolean" ||
    typeof result !== "string" ||
    typeof session_id !== "string" ||
    typeof total_cost_usd !== "number"
  ) {
    return null;
  }

  return {
    type: "result",
    subtype,
    is_error,
    result,
    session_id,
    total_cost_usd,
  };
}

// ── Extraction helpers ────────────────────────────────────────────────────────

/**
 * Join all text-type content blocks from an assistant event into a single string.
 * Multiple text blocks are concatenated directly (no separator added).
 */
export function extractTextContent(event: ClaudeAssistantEvent): string {
  return event.message.content
    .filter((block) => block.type === "text" && typeof block.text === "string")
    .map((block) => block.text as string)
    .join("");
}

/**
 * Extract all tool_use content blocks from an assistant event.
 * Blocks missing id, name, or input are skipped.
 */
export function extractToolUses(
  event: ClaudeAssistantEvent
): Array<{ id: string; name: string; input: Record<string, unknown> }> {
  return event.message.content
    .filter(
      (block): block is { type: "tool_use"; id: string; name: string; input: Record<string, unknown> } =>
        block.type === "tool_use" &&
        typeof block.id === "string" &&
        typeof block.name === "string" &&
        block.input !== undefined
    )
    .map((block) => ({
      id: block.id,
      name: block.name,
      input: block.input,
    }));
}
