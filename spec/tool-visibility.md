# Tool Visibility Specification

**AI-Native Messaging Protocol (AINM) -- Version 0.1.0**

## 1. Overview

Tool visibility makes agent tool calls transparent to all thread participants. When an agent invokes a tool during message processing, the AINM protocol emits three types of events into the thread: **ToolCallStarted**, **ToolCallProgress**, and **ToolCallCompleted**. These events are persistent, signed messages in the thread -- part of the cryptographic audit trail, not ephemeral notifications that vanish after delivery.

This is the core transparency principle of AINM: **there are no opaque tool calls**. Every tool invocation that an agent performs within the protocol is visible to every participant in the thread. If Alice asks an agent to search the codebase, every participant in the thread sees that the agent is running `code_search`, what parameters it passed, and what the result was. Tool events are thread messages like any other -- they flow through the same delivery mechanism, are signed with the same Ed25519 procedure, and are stored in the same message history.

Clients render tool events as **collapsible activity indicators** -- a compact, non-disruptive representation that shows tool progress without overwhelming the conversation. By default, a tool event appears as a collapsed single line showing the tool name and status. Users can expand it to see full details: input parameters, progress, result summary, and execution duration.

Tool events also carry a `content[]` array with TextBlock fallback on every event type. This ensures that clients which do not support the collapsible rendering pattern can still display a readable text representation of the tool activity.

## 2. Terminology

| Term | Definition |
|------|-----------|
| **Tool call** | A single invocation of a named tool by an agent. A tool call has a defined lifecycle: started, zero or more progress updates, and a completion. |
| **Tool call lifecycle** | The sequence of events for a single tool invocation: ToolCallStarted, zero or more ToolCallProgress events, and exactly one ToolCallCompleted event. |
| **call_id** | A unique identifier for a tool call (UUIDv7, optionally prefixed with `tc-`). All events in a tool call lifecycle share the same `call_id`, linking them together. |
| **Collapsible indicator** | The default client rendering for tool events: a compact, single-line display showing tool name and current status, expandable to show full details. |
| **Determinate progress** | Progress with known bounds -- the `current` and `total` fields are both set, enabling a progress bar or percentage display. |
| **Indeterminate progress** | Progress without known bounds -- the `total` field is null or absent. Only the `detail` text describes current state. Clients render this as a spinner or pulsing indicator. |
| **Tool event** | Any of the three event types: `tool.call.started`, `tool.call.progress`, `tool.call.completed`. Tool events are persistent signed messages in the thread. |

## 3. Tool Call Lifecycle

A tool call lifecycle consists of three event types. Each is a signed message in the thread, following the message envelope structure from security.md Section 3.1. All three event types share a common `tool` object that carries the tool-specific fields.

### 3.1 ToolCallStarted

**Message type:** `"tool.call.started"`

Emitted by the agent when it begins invoking a tool. This is the first event in the lifecycle. It tells participants what tool is being called and what input is being passed.

#### Fields

In addition to the standard message envelope fields (`id`, `from`, `thread_id`, `timestamp`, `type`, `content`, `signature`), a ToolCallStarted message includes:

**`content`** (array, REQUIRED)

Array with at least one TextBlock providing a human-readable description of what the tool is doing. This serves as the fallback for clients that do not support collapsible rendering.

**`tool`** (object, REQUIRED)

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `call_id` | string | REQUIRED | Unique identifier for this tool call. MUST be a UUIDv7 (RFC 9562), optionally prefixed with `tc-` (e.g., `"tc-019580c0-eeee-7000-8000-000000000001"`). All subsequent progress and completed events MUST reference this same `call_id`. |
| `name` | string | REQUIRED | Tool name. MUST match the tool's `name` in the agent's Agent Card `ainm.tools[]` array (agent-card.md Section 6.2). |
| `description` | string | OPTIONAL | Human-readable description of what this specific tool invocation is doing. More specific than the tool's general description in the Agent Card -- this describes the intent of this particular call. |
| `input` | object | REQUIRED | The input parameters passed to the tool. This object is visible to all thread participants. The structure SHOULD conform to the tool's `inputSchema` from the Agent Card. |

#### Example

```json
{
  "id": "019580c0-dddd-7000-8000-000000000004",
  "from": "code-bot@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:41:00.000Z",
  "type": "tool.call.started",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Searching codebase for authentication patterns...",
      "fallback": "Searching codebase for authentication patterns..."
    }
  ],
  "tool": {
    "call_id": "tc-019580c0-eeee-7000-8000-000000000001",
    "name": "code_search",
    "description": "Searching for authentication middleware implementations",
    "input": {
      "query": "authentication middleware",
      "scope": "src/",
      "file_types": [".ts", ".js"]
    }
  },
  "signature": "dG9vbC1jYWxsLXN0YXJ0ZWQtc2lnbmF0dXJl..."
}
```

### 3.2 ToolCallProgress

**Message type:** `"tool.call.progress"`

Emitted by the agent during tool execution to report intermediate status. **Zero or more** progress events may occur between the started and completed events. Agents SHOULD send progress events for long-running tools (estimated execution time > 5 seconds).

#### Fields

**`content`** (array, REQUIRED)

Array with at least one TextBlock providing an updated human-readable progress description.

**`tool`** (object, REQUIRED)

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `call_id` | string | REQUIRED | References the `call_id` from the ToolCallStarted event. MUST exactly match. |
| `progress` | object | REQUIRED | Progress details. See sub-fields below. |

**`tool.progress` sub-fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `current` | integer or null | OPTIONAL | Current progress count. For determinate progress, this is the number of items processed so far. `null` or absent for indeterminate progress. |
| `total` | integer or null | OPTIONAL | Total expected count. When both `current` and `total` are set, clients can display a progress bar or percentage. `null` or absent for indeterminate progress (unknown total). |
| `detail` | string | OPTIONAL | Human-readable text description of the current progress state. Provides context beyond the numeric progress (e.g., `"Processing file 3 of 12: auth.ts"`). |

#### Determinate vs Indeterminate Progress

- **Determinate progress**: Both `current` and `total` are non-null integers. Clients SHOULD render a progress bar or percentage.
- **Indeterminate progress**: `total` is null or absent. Clients SHOULD render a spinner or pulsing indicator with the `detail` text (if provided).

#### Example (Determinate Progress)

```json
{
  "id": "019580c1-aaaa-7000-8000-000000000005",
  "from": "code-bot@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:41:02.500Z",
  "type": "tool.call.progress",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Searching codebase: 7 of 12 files scanned...",
      "fallback": "Searching codebase: 7 of 12 files scanned..."
    }
  ],
  "tool": {
    "call_id": "tc-019580c0-eeee-7000-8000-000000000001",
    "progress": {
      "current": 7,
      "total": 12,
      "detail": "Scanning src/middleware/auth.ts"
    }
  },
  "signature": "dG9vbC1jYWxsLXByb2dyZXNzLXNpZ25hdHVyZQ..."
}
```

#### Example (Indeterminate Progress)

```json
{
  "id": "019580c1-bbbb-7000-8000-000000000006",
  "from": "code-bot@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:41:03.000Z",
  "type": "tool.call.progress",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Analyzing search results...",
      "fallback": "Analyzing search results..."
    }
  ],
  "tool": {
    "call_id": "tc-019580c0-eeee-7000-8000-000000000001",
    "progress": {
      "current": null,
      "total": null,
      "detail": "Ranking results by relevance"
    }
  },
  "signature": "dG9vbC1jYWxsLXByb2dyZXNzLWluZGV0LXNpZw..."
}
```

### 3.3 ToolCallCompleted

**Message type:** `"tool.call.completed"`

Emitted by the agent when the tool invocation finishes, whether successfully or with an error. This is the final event in the lifecycle. Exactly one completed event MUST be sent for every started event.

#### Fields

**`content`** (array, REQUIRED)

Array with at least one TextBlock providing a human-readable result summary.

**`tool`** (object, REQUIRED)

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `call_id` | string | REQUIRED | References the `call_id` from the ToolCallStarted event. MUST exactly match. |
| `name` | string | REQUIRED | Tool name. Repeated from the started event for standalone readability -- a recipient viewing only the completed event can identify the tool without referencing the started event. |
| `result_summary` | string | REQUIRED | Brief text summary of the tool's result. Human-readable, suitable for display in a collapsed indicator. For errors, this SHOULD describe what went wrong. |
| `duration_ms` | integer | REQUIRED | Wall-clock execution time in milliseconds, measured from the time the tool was invoked to the time the result was received. |
| `status` | string | REQUIRED | Tool execution outcome. One of: `"success"` (tool completed without errors) or `"error"` (tool failed). |
| `error_message` | string | OPTIONAL | Detailed error description. Present only when `status` is `"error"`. Provides diagnostic information for debugging. |

#### Example (Success)

```json
{
  "id": "019580c2-cccc-7000-8000-000000000007",
  "from": "code-bot@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:41:05.400Z",
  "type": "tool.call.completed",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Found 3 authentication middleware files in src/",
      "fallback": "Found 3 authentication middleware files in src/"
    }
  ],
  "tool": {
    "call_id": "tc-019580c0-eeee-7000-8000-000000000001",
    "name": "code_search",
    "result_summary": "Found 3 matches: auth.ts, jwt-middleware.ts, session-handler.ts",
    "duration_ms": 5400,
    "status": "success"
  },
  "signature": "dG9vbC1jYWxsLWNvbXBsZXRlZC1zdWNjZXNz..."
}
```

#### Example (Error)

```json
{
  "id": "019580c3-dddd-7000-8000-000000000008",
  "from": "code-bot@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:41:02.100Z",
  "type": "tool.call.completed",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "code_search failed: permission denied for directory src/secrets/",
      "fallback": "code_search failed: permission denied for directory src/secrets/"
    }
  ],
  "tool": {
    "call_id": "tc-019580c0-ffff-7000-8000-000000000002",
    "name": "code_search",
    "result_summary": "Permission denied: cannot read src/secrets/",
    "duration_ms": 120,
    "status": "error",
    "error_message": "EACCES: permission denied, scandir '/project/src/secrets/'. The tool does not have read access to this directory."
  },
  "signature": "dG9vbC1jYWxsLWNvbXBsZXRlZC1lcnJvcg..."
}
```

## 4. Thread Visibility

Tool events are delivered to the thread like any other message. The server routes them using the same thread-addressed delivery mechanism defined in threading.md Section 3.2 -- the message is sent to a `thread_id`, and all participants in that thread receive it.

### 4.1 No Private Tool Calls

There are no private tool calls in AINM. All tool events are visible to every participant in the thread. If an agent invokes a tool, every human and every other agent in the thread sees:

- What tool was called
- What input parameters were passed
- What progress the tool reports
- What the result was (or what error occurred)
- How long the tool took

### 4.2 Out-of-Protocol Tool Use

If an agent needs to invoke a tool without thread visibility (e.g., for internal reasoning or private data retrieval), it MUST do so outside the AINM protocol. Such tool use is not covered by this specification and does not produce tool events. The agent is responsible for any privacy, security, or compliance implications of out-of-protocol tool use.

### 4.3 Tool Events and Message Ordering

Tool events are ordered within the thread's chronological message stream. A ToolCallStarted event appears at the point in the conversation where the tool was invoked. Progress and completed events appear at their respective timestamps. Multiple concurrent tool calls by the same or different agents are interleaved chronologically.

Clients SHOULD visually group events with the same `call_id` together, even if other messages appear between them in the chronological stream. This grouping is a client-side rendering optimization, not a protocol requirement -- the underlying message order is strictly chronological.

## 5. Rendering Guidance

This section defines normative rendering behavior for conforming clients. These are requirements on how clients present tool events, not suggestions.

### 5.1 Default State: Collapsed

By default, tool events MUST be rendered in a collapsed state. The collapsed view shows:

- **Tool name** -- the `tool.name` value (e.g., `"code_search"`)
- **Current status** -- derived from the event type:
  - ToolCallStarted: `"running"` (or equivalent activity indicator)
  - ToolCallProgress: `"running"` with optional progress text from `tool.progress.detail`
  - ToolCallCompleted with `status: "success"`: `"completed"` (or equivalent success indicator)
  - ToolCallCompleted with `status: "error"`: `"failed"` (or equivalent error indicator)

**Collapsed rendering examples (text representation):**

```
[>] code_search: running...
[>] code_search: 7/12 files scanned
[v] code_search: completed (5.4s)
[!] code_search: failed
```

### 5.2 Expanded State

When the user expands a tool event (e.g., by clicking/tapping the collapsed indicator), the expanded view SHOULD show:

- **Tool name and description** (from the ToolCallStarted event)
- **Input parameters** -- the `tool.input` object, rendered as formatted JSON or as a labeled key-value list
- **Progress details** -- if progress events exist, the most recent `tool.progress.detail` text and any `current`/`total` counts
- **Result summary** -- the `tool.result_summary` from the completed event
- **Duration** -- the `tool.duration_ms` from the completed event, displayed as a human-readable duration
- **Error message** -- if `status` is `"error"`, the `tool.error_message` text

### 5.3 Status Indicators

Clients SHOULD use semantic visual indicators for tool completion status:

- **Success** (`status: "success"`): Positive/green indicator. The tool completed normally.
- **Error** (`status: "error"`): Negative/red indicator. The tool failed.

The protocol does not prescribe specific colors, icons, or animations. The semantic style names `"primary"` and `"danger"` from the protocol's semantic style system (UI-SPEC) map naturally to success and error states.

### 5.4 Duration Display

Clients SHOULD display the `duration_ms` value as a human-readable duration string:

- Values < 1000: display as milliseconds (e.g., `"120ms"`)
- Values >= 1000 and < 60000: display as seconds with one decimal (e.g., `"5.4s"`)
- Values >= 60000: display as minutes and seconds (e.g., `"2m 15s"`)

### 5.5 Fallback Rendering

Every tool event carries a `content[]` array with at least one TextBlock. Clients that do not support the collapsible rendering pattern MUST render the `content[]` array as they would render any other message's content blocks. The TextBlock `fallback` field provides a plain-text representation.

This ensures backward compatibility: a client that only understands Phase 1 message types will display tool events as regular text messages, with content like "Searching codebase for authentication patterns..." -- less structured, but still informative.

### 5.6 Lifecycle Grouping

When rendering a complete tool call lifecycle (started + progress + completed), clients SHOULD:

1. **Group** all events with the same `call_id` into a single visual unit.
2. **Update** the visual unit in place as new events arrive (progress updates replace the previous progress display; completed replaces the "running" state).
3. **Display** the final state (completed) as the primary view, with the option to expand and see the full lifecycle history.

This prevents the conversation view from being cluttered with multiple separate entries for a single tool call.

## 6. Error Handling

### 6.1 No New Protocol Error Codes

Tool visibility does not introduce new JSON-RPC error codes. Tool events are standard signed messages delivered through the existing transport layer. Transport-level delivery errors (connection failures, authentication errors, message too large) use the existing error codes from transport.md.

### 6.2 Application-Level Errors

When a tool call fails, the agent sends a ToolCallCompleted event with `status: "error"` and an optional `error_message`. This is an **application-level** error, not a protocol error:

- The protocol successfully delivered the tool event (no transport error).
- The tool itself reported a failure (application error).
- Other participants see the error as a normal tool event with error status.

### 6.3 Orphaned Started Events

If an agent sends a ToolCallStarted event but never sends a corresponding ToolCallCompleted event (e.g., the agent disconnects), clients SHOULD display the tool as "timed out" or "unknown status" after a client-configurable timeout (recommended: 5 minutes). The protocol does not enforce completion -- it is the agent's responsibility to send a completed event for every started event.

### 6.4 Out-of-Order Events

If a client receives a ToolCallProgress or ToolCallCompleted event before receiving the corresponding ToolCallStarted event (due to message reordering in transit), the client SHOULD:

1. Buffer the event and wait for the started event (recommended: up to 10 seconds).
2. If the started event arrives within the buffer window, render the lifecycle normally.
3. If the started event does not arrive, render the event as a standalone tool event using the information available (the `call_id`, `name` on completed events, and the `content[]` fallback).

## 7. Security Considerations

### 7.1 Signed Events

Tool events are signed by the agent that initiated the tool call, using the same Ed25519 signing procedure defined in security.md Section 4. The signature covers the entire message envelope including the `tool` object. This means:

- Tool events cannot be forged -- producing a valid signature requires the agent's private key.
- Tool events cannot be tampered with -- modifying any field (including `tool.input` or `tool.result_summary`) invalidates the signature.
- Tool event authorship is verifiable -- recipients can verify that the event came from the claimed sender by resolving the sender's public key via identity resolution.

### 7.2 Input Visibility

The `tool.input` field is visible to **all** thread participants. Agents MUST NOT pass sensitive data as tool input parameters when operating within the AINM protocol. Sensitive data includes:

- Passwords and authentication credentials
- API keys and tokens
- Private keys or key material
- Personally identifiable information (PII) that should not be shared with all participants

If an agent needs to pass sensitive data to a tool, the agent SHOULD:

1. **Redact** the sensitive portions of the input in the tool event. For example, replace an API key with `"[REDACTED]"` in the `tool.input` field.
2. **Handle** the actual sensitive data through a secure, out-of-protocol channel.
3. **Document** in the tool's `description` field (Agent Card) that the tool handles sensitive data.

### 7.3 Result Visibility

Similarly, the `tool.result_summary` field is visible to all participants. Agents SHOULD ensure that result summaries do not leak sensitive information. The `result_summary` is a brief text summary, not the full tool output -- agents can (and should) summarize results without including raw sensitive data.

### 7.4 Denial of Service via Tool Events

A malicious or malfunctioning agent could flood the thread with rapid tool events (e.g., thousands of progress updates per second). Servers MAY enforce rate limits on tool events per agent per thread. The protocol does not define specific rate limits -- these are server policy decisions. Servers that implement rate limiting SHOULD use the existing rate limiting mechanisms from transport.md.

## 8. Examples

### 8.1 Full Lifecycle: Success

A complete tool call lifecycle showing a code search that succeeds:

**1. ToolCallStarted**

```json
{
  "id": "019580c0-dddd-7000-8000-000000000004",
  "from": "code-bot@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:41:00.000Z",
  "type": "tool.call.started",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Searching codebase for authentication patterns...",
      "fallback": "Searching codebase for authentication patterns..."
    }
  ],
  "tool": {
    "call_id": "tc-019580c0-eeee-7000-8000-000000000001",
    "name": "code_search",
    "description": "Searching for authentication middleware implementations",
    "input": {
      "query": "authentication middleware",
      "scope": "src/"
    }
  },
  "signature": "c3RhcnRlZC1zaWduYXR1cmU..."
}
```

**2. ToolCallProgress (determinate)**

```json
{
  "id": "019580c1-aaaa-7000-8000-000000000005",
  "from": "code-bot@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:41:02.500Z",
  "type": "tool.call.progress",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Scanning files: 4 of 12 complete...",
      "fallback": "Scanning files: 4 of 12 complete..."
    }
  ],
  "tool": {
    "call_id": "tc-019580c0-eeee-7000-8000-000000000001",
    "progress": {
      "current": 4,
      "total": 12,
      "detail": "Scanning src/middleware/session.ts"
    }
  },
  "signature": "cHJvZ3Jlc3MtMS1zaWduYXR1cmU..."
}
```

**3. ToolCallProgress (later update)**

```json
{
  "id": "019580c1-bbbb-7000-8000-000000000006",
  "from": "code-bot@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:41:04.000Z",
  "type": "tool.call.progress",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Scanning files: 11 of 12 complete...",
      "fallback": "Scanning files: 11 of 12 complete..."
    }
  ],
  "tool": {
    "call_id": "tc-019580c0-eeee-7000-8000-000000000001",
    "progress": {
      "current": 11,
      "total": 12,
      "detail": "Scanning src/routes/protected.ts"
    }
  },
  "signature": "cHJvZ3Jlc3MtMi1zaWduYXR1cmU..."
}
```

**4. ToolCallCompleted (success)**

```json
{
  "id": "019580c2-cccc-7000-8000-000000000007",
  "from": "code-bot@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:41:05.400Z",
  "type": "tool.call.completed",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Search complete: found 3 authentication middleware files.",
      "fallback": "Search complete: found 3 authentication middleware files."
    }
  ],
  "tool": {
    "call_id": "tc-019580c0-eeee-7000-8000-000000000001",
    "name": "code_search",
    "result_summary": "Found 3 matches: auth.ts, jwt-middleware.ts, session-handler.ts",
    "duration_ms": 5400,
    "status": "success"
  },
  "signature": "Y29tcGxldGVkLXN1Y2Nlc3Mtc2lnbmF0dXJl..."
}
```

**Client rendering of this lifecycle (collapsed):**

```
[v] code_search: completed (5.4s) -- 3 matches found
```

**Client rendering (expanded):**

```
+--------------------------------------------------+
| code_search                          completed    |
|                                                   |
| Input:                                            |
|   query: "authentication middleware"              |
|   scope: "src/"                                   |
|                                                   |
| Progress: 12/12 files scanned                     |
|                                                   |
| Result: Found 3 matches: auth.ts,                 |
|         jwt-middleware.ts, session-handler.ts      |
|                                                   |
| Duration: 5.4s                                    |
+--------------------------------------------------+
```

### 8.2 Error Lifecycle

A tool call that fails:

**1. ToolCallStarted**

```json
{
  "id": "019580d0-aaaa-7000-8000-000000000010",
  "from": "data-agent@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:45:00.000Z",
  "type": "tool.call.started",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Querying database for user statistics...",
      "fallback": "Querying database for user statistics..."
    }
  ],
  "tool": {
    "call_id": "tc-019580d0-bbbb-7000-8000-000000000002",
    "name": "db_query",
    "description": "Executing SQL query for user signup statistics",
    "input": {
      "query": "SELECT COUNT(*) FROM users WHERE created_at > '2026-03-01'",
      "database": "analytics"
    }
  },
  "signature": "ZXJyb3Itc3RhcnRlZC1zaWduYXR1cmU..."
}
```

**2. ToolCallCompleted (error, no progress events)**

```json
{
  "id": "019580d0-cccc-7000-8000-000000000011",
  "from": "data-agent@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:45:00.800Z",
  "type": "tool.call.completed",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Database query failed: connection timeout.",
      "fallback": "Database query failed: connection timeout."
    }
  ],
  "tool": {
    "call_id": "tc-019580d0-bbbb-7000-8000-000000000002",
    "name": "db_query",
    "result_summary": "Connection timeout after 800ms",
    "duration_ms": 800,
    "status": "error",
    "error_message": "ETIMEDOUT: Connection to analytics database timed out after 800ms. The database may be under heavy load or unreachable."
  },
  "signature": "ZXJyb3ItY29tcGxldGVkLXNpZ25hdHVyZQ..."
}
```

**Client rendering (collapsed):**

```
[!] db_query: failed (0.8s) -- connection timeout
```

## References

- **threading.md** -- Thread model and message delivery (Section 3.2: thread-addressed delivery)
- **security.md** -- Message envelope and signing procedure (Section 3.1: envelope structure, Section 4: signing)
- **content.md** -- Content block base schema (Section 3: TextBlock for fallback rendering)
- **agent-card.md** -- Agent Card tool listing (Section 6.2: `ainm.tools` array)
- **transport.md** -- JSON-RPC methods and WebSocket binding
- **RFC 9562** -- UUIDv7 (used for `call_id` generation)

---

*AI-Native Messaging Protocol -- Tool Visibility Specification v0.1.0*
*Cross-references: threading.md (message delivery), security.md (signing), content.md (TextBlock fallback), agent-card.md (tool listing)*
