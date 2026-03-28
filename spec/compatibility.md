# Compatibility Bridge Specification

**AI-Native Messaging Protocol (AINM) -- Version 0.1.0**

## 1. Overview

AINM provides compatibility bridges to two major ecosystem protocols: **MCP** (Model Context Protocol) for tool integration, and **A2A** (Agent-to-Agent) for agent interoperability. These bridges enable existing MCP servers to be exposed as AINM thread participants, and A2A task lifecycle to be represented as AINM thread events. Bridges do not require MCP tools or A2A agents to be rewritten -- they operate as translation layers that map between protocol semantics.

Bridges are implemented as **proxy participants** -- AINM server components with their own identities that translate between protocols. A proxy participant has a full AINM identity (registered via identity.md), publishes an Agent Card (per agent-card.md), and participates in threads like any other agent. The difference is that it does not generate content itself -- it translates content between an external protocol and the AINM thread.

This specification defines:

1. **MCP Bridge** -- Exposes MCP server tools as AINM thread capabilities. MCP tool calls appear as tool visibility events (tool-visibility.md), and MCP tool discovery maps to the Agent Card `ainm.tools` array (agent-card.md Section 6.2).

2. **A2A Bridge** -- Maps A2A task lifecycle states to AINM thread events. A2A tasks appear as sub-threads, and A2A state transitions appear as presence updates, interactive blocks, and messages within those sub-threads.

Both bridges are OPTIONAL server features. Servers that support bridges MUST explicitly enable them through configuration -- bridges are NOT auto-discovered from the network.

## 2. Terminology

| Term | Definition |
|------|-----------|
| **Bridge** | An AINM server component that translates between AINM protocol operations and an external protocol (MCP or A2A). A bridge operates as a proxy participant in AINM threads. |
| **Proxy participant** | An AINM participant whose messages originate from or are destined for an external protocol system. The proxy has its own AINM identity, Ed25519 key pair, and Agent Card, but its behavior is driven by the external system it represents. |
| **MCP bridge** | A bridge that connects to one or more MCP servers and exposes their tools as AINM thread capabilities. Each MCP server connection is represented by a separate proxy participant. |
| **A2A bridge** | A bridge that translates between A2A task lifecycle and AINM thread events. Operates in two modes: outbound (AINM participant delegates to A2A agent) and inbound (A2A agent sends tasks that appear in AINM threads). |
| **Tool mapping** | The process of translating MCP tool definitions (name, description, inputSchema) into the `ainm.tools` array format defined in agent-card.md Section 6.2. |
| **Task mapping** | The process of translating A2A task lifecycle states (submitted, working, input-required, completed, failed, canceled) into AINM thread events (sub-thread creation, presence updates, interactive blocks, messages). |
| **Bridge identity** | The AINM address assigned to a bridge proxy participant. Bridge identities SHOULD follow a naming convention that indicates the bridge type: `mcp-{server-name}@{provider}` or `a2a-{agent-name}@{provider}`. |

## 3. MCP Bridge

The MCP bridge enables MCP servers to participate in AINM threads without any modification to the MCP server. The bridge connects to an MCP server as a standard MCP client, registers as an AINM proxy participant, and translates between the two protocols.

### 3.1 Architecture

The MCP bridge is a server-side component with three responsibilities:

1. **MCP client connection** -- The bridge connects to an MCP server using the MCP client protocol (JSON-RPC 2.0 over stdio, SSE, or Streamable HTTP). It maintains this connection for the lifetime of the bridge.

2. **AINM participant registration** -- The bridge registers as an AINM participant with its own identity (e.g., `mcp-code-tools@agents.example.com`). This identity is created via the standard registration process (identity.md Section 5) and has its own Ed25519 key pair.

3. **Protocol translation** -- The bridge translates between AINM thread messages and MCP tool operations. When a thread participant requests a tool call, the bridge invokes the MCP tool and delivers the result back to the thread.

**Architecture diagram:**

```
AINM Thread                        MCP Bridge                         MCP Server
                                   (proxy participant)
    |                                    |                                |
    |-- @mcp-code-tools (mention) ------>|                                |
    |                                    |-- tools/call ----------------->|
    |<-- tool.call.started --------------|                                |
    |                                    |<-- tool result ----------------|
    |<-- tool.call.completed ------------|                                |
    |                                    |                                |
```

Each MCP server connection requires a separate bridge instance and a separate AINM identity. A single AINM server MAY operate multiple MCP bridges, each with its own identity and Agent Card.

### 3.2 Tool Discovery Mapping

The MCP bridge maps MCP tool definitions to the AINM Agent Card format. This mapping occurs at bridge startup and whenever the MCP server notifies the bridge of tool list changes.

**Initial discovery:**

1. Bridge sends MCP `tools/list` request to the MCP server.
2. MCP server responds with an array of tool definitions, each containing `name`, `description`, and `inputSchema`.
3. Bridge populates the `ainm.tools` array in its Agent Card with the MCP tool definitions.

**MCP tool definition to AINM Agent Card mapping:**

| MCP Field | AINM Agent Card Field | Notes |
|-----------|----------------------|-------|
| `name` | `ainm.tools[].name` | Preserved verbatim. Tool names are NOT modified or prefixed by the bridge. |
| `description` | `ainm.tools[].description` | Preserved verbatim. |
| `inputSchema` | `ainm.tools[].inputSchema` | Preserved verbatim. The MCP input schema is a JSON Schema object, which is the same format used by AINM Agent Card tools (agent-card.md Section 6.2). |

**Example -- MCP `tools/list` response mapped to AINM Agent Card:**

MCP response:
```json
{
  "tools": [
    {
      "name": "code_search",
      "description": "Search codebase for patterns and definitions",
      "inputSchema": {
        "type": "object",
        "properties": {
          "query": { "type": "string", "description": "Search query" },
          "scope": { "type": "string", "description": "Directory scope" }
        },
        "required": ["query"]
      }
    }
  ]
}
```

Resulting AINM Agent Card `ainm.tools` entry:
```json
{
  "ainm": {
    "tools": [
      {
        "name": "code_search",
        "description": "Search codebase for patterns and definitions",
        "inputSchema": {
          "type": "object",
          "properties": {
            "query": { "type": "string", "description": "Search query" },
            "scope": { "type": "string", "description": "Directory scope" }
          },
          "required": ["query"]
        }
      }
    ]
  }
}
```

**Dynamic tool updates:**

When the MCP server sends a `notifications/tools/list_changed` notification, the bridge MUST:

1. Re-issue `tools/list` to get the updated tool list.
2. Update its Agent Card's `ainm.tools` array with the new tool definitions.
3. Update the Agent Card's `version` field (semver increment).
4. Serve the updated Agent Card with a new `ETag` header value.

Clients that have cached the Agent Card will detect the change on their next conditional request (agent-card.md Section 4) and receive the updated tool list.

### 3.3 Tool Invocation Mapping

When a thread participant requests a tool call from the MCP bridge, the bridge translates the request into an MCP `tools/call` operation and delivers the result as AINM tool visibility events.

**Invocation flow:**

1. **Trigger** -- A thread participant @-mentions the MCP bridge and indicates a tool call. The bridge detects the mention (content.md Section 4.1, threading.md Section 9) and parses the tool name and input parameters from the message content. The parsing strategy is implementation-defined -- bridges MAY use natural language understanding, structured JSON in the message, or a convention like `@bridge tool_name {"param": "value"}`.

2. **ToolCallStarted** -- The bridge sends a `tool.call.started` event to the thread (per tool-visibility.md Section 3.1). The event includes:
   - `tool.call_id`: A new UUIDv7 with `tc-` prefix.
   - `tool.name`: The MCP tool name (verbatim from the MCP server).
   - `tool.input`: The input parameters that will be sent to the MCP tool.
   - `content[]`: A TextBlock describing what the bridge is doing.

3. **MCP `tools/call`** -- The bridge calls the MCP server's `tools/call` method with the tool name and input parameters.

4. **ToolCallProgress** (optional) -- If the MCP server supports progress notifications or the tool call takes more than 5 seconds, the bridge MAY send `tool.call.progress` events to the thread (per tool-visibility.md Section 3.2).

5. **ToolCallCompleted** -- When the MCP server returns the tool result, the bridge sends a `tool.call.completed` event to the thread (per tool-visibility.md Section 3.3). The event includes:
   - `tool.status`: `"success"` or `"error"` based on the MCP response.
   - `tool.result_summary`: A brief text summary of the result.
   - `tool.duration_ms`: Wall-clock time for the MCP tool call.
   - `content[]`: A TextBlock with the result summary.

6. **Result message** (optional) -- The bridge MAY also send a regular message (`type: "message"`) with the full tool result as content blocks (TextBlock, CodeBlock, etc.). This provides the complete result in a format suitable for conversation context, while the ToolCallCompleted event provides the structured audit trail.

**Example -- complete invocation lifecycle:**

User message:
```json
{
  "type": "message",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "user",
      "text": "@mcp-code-tools search for authentication middleware in src/",
      "mentions": [
        { "address": "mcp-code-tools@agents.example.com", "offset": 0, "length": 15 }
      ],
      "fallback": "@mcp-code-tools search for authentication middleware in src/"
    }
  ]
}
```

Bridge sends ToolCallStarted:
```json
{
  "id": "019580e0-1111-7000-8000-000000000001",
  "from": "mcp-code-tools@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T14:00:00.000Z",
  "type": "tool.call.started",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Searching codebase for authentication middleware...",
      "fallback": "Searching codebase for authentication middleware..."
    }
  ],
  "tool": {
    "call_id": "tc-019580e0-2222-7000-8000-000000000001",
    "name": "code_search",
    "description": "Searching for authentication middleware in src/ directory",
    "input": {
      "query": "authentication middleware",
      "scope": "src/"
    }
  },
  "signature": "bWNwLWJyaWRnZS10b29sLXN0YXJ0ZWQtc2ln..."
}
```

Bridge sends ToolCallCompleted after MCP returns:
```json
{
  "id": "019580e0-3333-7000-8000-000000000002",
  "from": "mcp-code-tools@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T14:00:03.200Z",
  "type": "tool.call.completed",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Found 3 matches for authentication middleware in src/",
      "fallback": "Found 3 matches for authentication middleware in src/"
    }
  ],
  "tool": {
    "call_id": "tc-019580e0-2222-7000-8000-000000000001",
    "name": "code_search",
    "result_summary": "Found 3 matches: auth.ts, jwt-middleware.ts, session-handler.ts",
    "duration_ms": 3200,
    "status": "success"
  },
  "signature": "bWNwLWJyaWRnZS10b29sLWNvbXBsZXRlZC1zaWc..."
}
```

### 3.4 Error Mapping

MCP errors are mapped to AINM tool visibility events with error status:

| MCP Error Condition | AINM Equivalent | Behavior |
|---------------------|-----------------|----------|
| MCP `tools/call` returns `isError: true` | `tool.call.completed` with `status: "error"` | Bridge sends ToolCallCompleted with `error_message` containing the MCP error text. |
| MCP connection failure (transport error) | System message to thread | Bridge sends a `type: "message"` with `source: "system"` TextBlock describing the connection failure. |
| MCP tool timeout | `tool.call.completed` with `status: "error"` | Bridge sends ToolCallCompleted with `error_message` describing the timeout. `duration_ms` reflects the time waited before timeout. |
| MCP tool not found | `tool.call.completed` with `status: "error"` | Bridge sends ToolCallCompleted with `error_message`: "Tool '{name}' not found on MCP server." |
| MCP server disconnected during call | `tool.call.completed` with `status: "error"` | Bridge sends ToolCallCompleted with `error_message` describing the disconnection. If the bridge cannot recover the connection, it SHOULD also send a system message noting the MCP server is unavailable. |

**Error example:**

```json
{
  "id": "019580e1-aaaa-7000-8000-000000000010",
  "from": "mcp-code-tools@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T14:01:00.500Z",
  "type": "tool.call.completed",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "code_search failed: connection to MCP server timed out after 30 seconds.",
      "fallback": "code_search failed: connection to MCP server timed out after 30 seconds."
    }
  ],
  "tool": {
    "call_id": "tc-019580e1-bbbb-7000-8000-000000000003",
    "name": "code_search",
    "result_summary": "Connection timeout after 30s",
    "duration_ms": 30000,
    "status": "error",
    "error_message": "ETIMEDOUT: Connection to MCP server timed out after 30000ms. The MCP server may be under heavy load or unreachable."
  },
  "signature": "bWNwLWVycm9yLXRvb2wtY29tcGxldGVkLXNpZw..."
}
```

### 3.5 MCP Bridge Agent Card

The MCP bridge publishes an AINM Agent Card that reflects the MCP server's capabilities. The card follows the schema defined in agent-card.md with specific conventions for bridge participants.

**Required Agent Card fields for MCP bridges:**

- `name`: SHOULD include the MCP server name (e.g., `"Code Tools (MCP Bridge)"`).
- `description`: SHOULD describe the bridge nature and the MCP server's purpose.
- `capabilities.streaming`: Set based on whether the MCP server supports streaming results.
- `ainm.contentTypes`: At minimum `["text", "code"]` for tool result delivery.
- `ainm.tools`: Populated from MCP `tools/list` (see Section 3.2).
- `ainm.bridgeType`: A string field indicating the bridge type. MUST be `"mcp"` for MCP bridges. This field is an AINM extension within the `ainm` object.
- `ainm.bridgeTarget`: A string describing the external service. For MCP bridges, this SHOULD be the MCP server name or identifier.

**Example Agent Card for MCP bridge:**

```json
{
  "name": "Code Tools (MCP Bridge)",
  "description": "Bridge to a code analysis MCP server. Provides code search, file reading, and test execution tools via the MCP protocol.",
  "url": "https://agents.example.com/.well-known/ainm/mcp-code-tools",
  "version": "1.0.0",
  "protocolVersion": "0.2.1",
  "capabilities": {
    "streaming": false,
    "pushNotifications": false
  },
  "skills": [
    {
      "id": "code-analysis",
      "name": "Code Analysis",
      "description": "Searches, reads, and analyzes source code files.",
      "tags": ["code", "search", "analysis"]
    }
  ],
  "ainm": {
    "protocolVersion": "0.1.0",
    "contentTypes": ["text", "code"],
    "streaming": {
      "supported": false,
      "parallelBlocks": false,
      "preferredChunkSize": "token"
    },
    "threading": {
      "subThreads": false,
      "maxConcurrentThreads": null
    },
    "memory": {
      "archivalSupported": false,
      "contextFramePreferred": false
    },
    "tools": [
      {
        "name": "code_search",
        "description": "Search codebase for patterns and definitions",
        "inputSchema": {
          "type": "object",
          "properties": {
            "query": { "type": "string" },
            "scope": { "type": "string" }
          },
          "required": ["query"]
        }
      }
    ],
    "bridgeType": "mcp",
    "bridgeTarget": "code-analysis-server"
  }
}
```

### 3.6 Limitations

- **MCP resources** (read-only data exposed by the MCP server) are NOT directly mapped in v1. Agents MUST use MCP tools to access resources. A future protocol version MAY define a resource mapping.
- **MCP prompts** (predefined prompt templates) are NOT mapped. AINM has its own context management via the memory specification (memory.md). MCP prompts are an MCP-specific concept that does not have a natural AINM equivalent.
- **One bridge per MCP server.** The bridge operates as a single AINM participant. Each MCP server connection requires a separate bridge instance with its own AINM identity. A single bridge instance MUST NOT multiplex multiple MCP servers.
- **MCP sampling** (the ability for an MCP server to request LLM completions from the client) is NOT mapped. AINM agents generate their own responses -- there is no protocol-level mechanism for an external service to request completions through a bridge.

## 4. A2A Bridge

The A2A bridge maps the A2A (Agent-to-Agent) task lifecycle to AINM thread events. While the MCP bridge focuses on tool-level integration, the A2A bridge focuses on task-level interoperability -- enabling AINM participants to delegate work to A2A agents and receive structured results.

### 4.1 Architecture

The A2A bridge operates in two modes:

**Outbound mode** -- An AINM participant delegates work to an external A2A agent. The bridge translates AINM messages into A2A task operations and delivers A2A task results back to the AINM thread.

```
AINM Thread              A2A Bridge                    A2A Agent
                         (proxy participant)
    |                         |                            |
    |-- "delegate task" ----->|                            |
    |                         |-- POST /tasks (submit) --->|
    |<-- sub-thread created --|                            |
    |<-- presence: processing |<-- status: working --------|
    |<-- FormBlock ---------- |<-- input-required ---------|
    |-- ActionBlock --------->|-- send input -------------->|
    |<-- result message ------|<-- status: completed -------|
    |                         |                            |
```

**Inbound mode** -- An external A2A agent sends tasks that appear as AINM thread events. The bridge receives A2A task notifications and translates them into AINM thread messages.

```
A2A Agent                A2A Bridge                    AINM Thread
                         (proxy participant)
    |                         |                            |
    |-- POST /tasks --------->|                            |
    |                         |-- sub-thread created ------>|
    |-- status: working ----->|-- presence: processing ---->|
    |-- status: completed --->|-- result message ---------->|
    |                         |                            |
```

The bridge has its own AINM identity (e.g., `a2a-research-agent@agents.example.com`) and participates in threads as any other agent participant.

### 4.2 Task Lifecycle Mapping

The following table defines how A2A task states map to AINM protocol operations:

| A2A Task State | AINM Equivalent | How |
|----------------|-----------------|-----|
| `submitted` | `ainm.thread.create` (sub-thread) | Bridge creates a sub-thread for the task. The sub-thread title reflects the task description. Task metadata is included in the initial message. |
| `working` | `ainm.presence.update` with `status: "processing"` | Bridge updates its presence in the task sub-thread to `"processing"` with `detail` describing the current work. |
| `input-required` | Message with FormBlock or ApprovalBlock | Bridge sends a message containing interactive blocks (interactive.md) that collect the required input. The form fields are derived from the A2A task's input schema. |
| `completed` | Message with result content blocks | Bridge delivers the task result as TextBlock, CodeBlock, ImageBlock, or FileBlock content within the sub-thread. The result content type depends on the A2A task's output MIME type. |
| `failed` | Message with error TextBlock | Bridge sends a message with a TextBlock (`source: "system"`) describing the failure. The text includes the A2A error code and message if available. |
| `canceled` | Thread leave or `ainm.stream.cancel` | Bridge sends `ainm.thread.leave` for the task sub-thread. If the task was streaming results, the bridge sends `ainm.stream.cancel` first. |

### 4.3 Task Submission (Outbound)

When an AINM participant requests the A2A bridge to perform a task, the bridge:

1. **Creates a sub-thread** -- The bridge creates a sub-thread (threading.md Section 4) from the requesting message. The sub-thread contains the requesting participant and the bridge as participants.

2. **Submits the A2A task** -- The bridge translates the request into an A2A task submission (`POST /tasks` with `state: "submitted"`). The task description is derived from the AINM message content.

3. **Monitors task state** -- The bridge polls the A2A agent's task status (or receives push notifications if the A2A agent supports them) and translates state transitions into AINM events in the sub-thread.

**Example -- task submission flow:**

User sends message in the main thread:
```json
{
  "type": "message",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "user",
      "text": "@a2a-research-agent please research the latest developments in quantum computing and summarize the top 5 breakthroughs.",
      "mentions": [
        { "address": "a2a-research-agent@agents.example.com", "offset": 0, "length": 20 }
      ],
      "fallback": "@a2a-research-agent please research quantum computing breakthroughs."
    }
  ]
}
```

Bridge creates sub-thread:
```json
{
  "jsonrpc": "2.0",
  "id": 100,
  "method": "ainm.thread.create",
  "params": {
    "thread_id": "019580f0-aaaa-7000-8000-000000000001",
    "parent_thread_id": "019580a0-1234-7000-8000-000000000001",
    "parent_message_id": "019580e5-1111-7000-8000-000000000001",
    "title": "A2A Task: Research quantum computing breakthroughs",
    "participants": [
      "alice@messaging.example.com",
      "a2a-research-agent@agents.example.com"
    ]
  }
}
```

Bridge updates presence to processing:
```json
{
  "jsonrpc": "2.0",
  "method": "ainm.presence.update",
  "params": {
    "identity": "a2a-research-agent@agents.example.com",
    "status": "processing",
    "thread_id": "019580f0-aaaa-7000-8000-000000000001",
    "detail": "Researching quantum computing developments..."
  }
}
```

### 4.4 Input-Required Mapping

When the A2A agent transitions to `input-required`, the bridge translates the input request into AINM interactive blocks:

- **Simple input** (text or selection) maps to a **FormBlock** (interactive.md Section 5).
- **Approval gates** (proceed/cancel decisions) map to an **ApprovalBlock** (interactive.md Section 7).

The user's response (an ActionBlock per interactive.md Section 8) is captured by the bridge and translated into A2A task input.

**Example -- A2A input-required mapped to FormBlock:**

```json
{
  "id": "019580f1-bbbb-7000-8000-000000000010",
  "from": "a2a-research-agent@agents.example.com",
  "thread_id": "019580f0-aaaa-7000-8000-000000000001",
  "timestamp": "2026-03-28T14:05:00.000Z",
  "type": "message",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "I found several areas of quantum computing research. Please select which areas to focus on:",
      "fallback": "Please select research focus areas."
    },
    {
      "type": "form",
      "version": "1.0",
      "source": "agent",
      "action_id": "a2a-input-select-areas",
      "title": "Research Focus Areas",
      "submit_label": "Continue Research",
      "fields": [
        {
          "name": "areas",
          "label": "Focus Areas",
          "field_type": "select",
          "required": true,
          "options": ["Error Correction", "Quantum Advantage", "Topological Qubits", "Quantum Networking", "Hybrid Algorithms"]
        }
      ],
      "fallback": "[Form: Select research focus areas]"
    }
  ],
  "signature": "YTJhLWlucHV0LXJlcXVpcmVkLXNpZw..."
}
```

### 4.5 Agent Card Compatibility

Per decision D-09 and the Agent Card specification (agent-card.md), AINM Agent Cards are a superset of A2A Agent Cards. This creates a natural bidirectional compatibility:

**AINM Agent Card -> A2A discovery:**
An A2A client can discover an AINM agent by fetching its Agent Card at the well-known URL. The A2A client reads the top-level A2A-compatible fields (`name`, `description`, `capabilities`, `skills`, etc.) and ignores the `ainm` extension object. The AINM agent appears as a standard A2A agent.

**A2A Agent Card -> AINM discovery:**
The A2A bridge can wrap an external A2A agent's Agent Card with AINM extensions. The bridge fetches the A2A agent's card, copies the top-level fields, and adds an `ainm` object with:
- `ainm.protocolVersion`: The AINM version the bridge supports.
- `ainm.contentTypes`: Based on the A2A agent's output modes.
- `ainm.bridgeType`: `"a2a"`.
- `ainm.bridgeTarget`: The A2A agent's URL or identifier.

This wrapped card is published at the bridge's own Agent Card URL, allowing AINM clients to discover A2A agents through the bridge.

### 4.6 Artifact Mapping

A2A task artifacts (files produced by the task) are mapped to AINM content blocks:

| A2A Artifact Type | AINM Content Block | Notes |
|-------------------|-------------------|-------|
| Text content (`text/plain`) | TextBlock | Direct mapping. |
| JSON content (`application/json`) | CodeBlock with `language: "json"` | Rendered as formatted JSON. |
| Image content (`image/*`) | ImageBlock | Bridge downloads the artifact and serves it via HTTPS. |
| File content (any other MIME type) | FileBlock | Bridge downloads the artifact, computes SHA-256 digest, and serves via HTTPS. |

The bridge MUST download A2A artifacts and serve them through its own HTTPS infrastructure. AINM content blocks require HTTPS URLs (content.md Section 4.3, 4.4), and A2A artifact URLs may not meet AINM's URL requirements. By re-hosting artifacts, the bridge ensures URL scheme compliance and content integrity via SHA-256 digests on FileBlocks.

### 4.7 A2A Streaming Integration

When an A2A agent supports streaming (SSE-based), the bridge MAY map A2A streaming events to AINM streaming events (streaming.md):

1. Bridge receives A2A SSE event with task update containing streaming content.
2. Bridge initiates an AINM stream (`ainm.stream.start`) in the task sub-thread.
3. A2A content chunks are translated to `ainm.stream.delta` events.
4. When the A2A stream completes, the bridge sends `ainm.stream.end`.
5. The collapsed permanent message contains the complete streamed content.

This mapping is OPTIONAL. Bridges that do not support streaming deliver A2A results as complete messages after the A2A task reaches `completed` state.

### 4.8 Limitations

- **A2A push notifications** are NOT mapped to AINM in v1. AINM uses persistent WebSocket connections for real-time delivery. The A2A bridge polls for task status or uses A2A SSE streaming, but does not expose A2A push notification subscriptions to AINM participants.
- **A2A multi-turn tasks** work naturally via the sub-thread model. Each `input-required` state transition produces an interactive block in the sub-thread, and the user's ActionBlock response provides the next input. The sub-thread accumulates the full multi-turn interaction history.
- **A2A task cancellation** from the AINM side is supported: when the requesting participant leaves the task sub-thread or sends a cancel request, the bridge sends an A2A cancel request to the external agent.

## 5. Bridge Identity and Security

### 5.1 Identity Convention

Bridge proxy participants MUST have AINM identities registered via the standard registration process (identity.md Section 5). Bridge identities SHOULD follow a naming convention that indicates the bridge type:

- **MCP bridges:** `mcp-{server-name}@{provider}` (e.g., `mcp-code-tools@agents.example.com`)
- **A2A bridges:** `a2a-{agent-name}@{provider}` (e.g., `a2a-research-agent@agents.example.com`)

The `type` field in the identity record (identity.md Section 4.3) MUST be `"agent"` for all bridge participants. Bridges are agents from the protocol's perspective -- the fact that they translate to an external system is an implementation detail, not a protocol distinction.

### 5.2 Message Signing

Bridge messages are signed with the bridge's own Ed25519 private key, following the standard signing procedure (security.md Section 4). This means:

- Bridge messages are attributable to the bridge identity, not to the external MCP server or A2A agent.
- Recipients verify bridge messages against the bridge's public key (resolved via identity resolution).
- The bridge is responsible for the content it delivers -- if a bridge delivers incorrect or malicious content from an external source, the bridge's identity is the accountable party.

### 5.3 Content Source Attribution

The `source` field on content blocks from bridges is `"agent"` (security.md Section 6.1). The bridge acts on behalf of the external service and its generated content is AI-originated. Content blocks MUST NOT use `source: "user"` even if the external MCP tool or A2A agent claims to relay human-authored content -- the bridge cannot verify the external service's source claims.

System messages generated by the bridge itself (e.g., connection failure notifications) use `source: "system"`.

### 5.4 Agent Card Bridge Metadata

Bridge Agent Cards SHOULD include fields in the `ainm` extension object indicating the bridge type and external service:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `ainm.bridgeType` | string | RECOMMENDED | The bridge protocol. One of `"mcp"` or `"a2a"`. |
| `ainm.bridgeTarget` | string | RECOMMENDED | Human-readable identifier of the external service (e.g., MCP server name, A2A agent URL). |

These fields allow AINM clients to display bridge provenance information in the UI (e.g., "This agent is an MCP bridge to code-analysis-server").

## 6. Error Handling

Bridge-specific error scenarios are handled via existing AINM error mechanisms. No new error code ranges are defined for bridges.

### 6.1 Tool-Level Errors

Tool invocation errors from MCP bridges use the tool visibility event mechanism (tool-visibility.md Section 6). A `tool.call.completed` event with `status: "error"` communicates the failure. The `error_message` field carries diagnostic information from the MCP server.

### 6.2 Task-Level Errors

A2A task failures are communicated as messages in the task sub-thread. The bridge sends a message with:
- `type: "message"`
- A TextBlock with `source: "system"` describing the failure.
- If the A2A agent provided an error code, it is included in the text.

### 6.3 Protocol-Level Errors

Protocol-level errors (malformed bridge configuration, external service permanently unreachable, authentication failure with external service) result in system messages delivered to threads where the bridge is a participant:

```json
{
  "id": "019580f5-aaaa-7000-8000-000000000099",
  "from": "mcp-code-tools@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T14:10:00.000Z",
  "type": "message",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "system",
      "text": "MCP bridge disconnected: unable to reach MCP server 'code-analysis-server'. Tool calls to this bridge are temporarily unavailable.",
      "fallback": "MCP bridge disconnected: MCP server unreachable."
    }
  ],
  "signature": "YnJpZGdlLWVycm9yLW1lc3NhZ2Utc2ln..."
}
```

### 6.4 Error Code Reuse

Bridges use existing AINM error codes:

| Scenario | Error Code | Source |
|----------|-----------|--------|
| Bridge message signature invalid | `-32010` | security.md |
| Bridge identity cannot be resolved | `-32011` | security.md |
| Bridge sends malformed message | `-32013` | security.md |
| Thread not found for bridge operation | `-32024` | transport.md |
| Bridge not a thread participant | `-32025` | transport.md |

## 7. Security Considerations

### 7.1 Trust Boundary

Bridges are **trust boundaries** in the AINM protocol. An AINM server that operates a bridge implicitly trusts the external MCP or A2A service that the bridge connects to. This trust relationship has several implications:

- **Data exposure**: Tool inputs sent through the MCP bridge are transmitted to the external MCP server. The bridge MUST follow the same input visibility rules as tool-visibility.md Section 7.2 -- sensitive data SHOULD be redacted from tool events before delivery to the thread, and the actual sensitive data handled through the bridge's connection to the MCP server.

- **Result integrity**: The bridge cannot cryptographically verify that MCP tool results or A2A task outputs are authentic. The bridge trusts the external service's responses and delivers them to the thread under the bridge's own signature.

- **Availability dependency**: The bridge's functionality depends on the external service's availability. If the MCP server or A2A agent is unreachable, the bridge cannot perform its translation function.

### 7.2 Explicit Configuration

Servers MUST require explicit configuration to enable bridges. Bridges MUST NOT be auto-discovered from the network or auto-enabled based on Agent Card content. An administrator must:

1. Configure the bridge with the external service's connection details.
2. Register the bridge's AINM identity.
3. Generate and install the bridge's Ed25519 key pair.
4. Publish the bridge's Agent Card.

This explicit setup prevents rogue bridges from being introduced into the network.

### 7.3 Input Visibility

MCP tool inputs visible in tool events (tool-visibility.md) may contain data that the external MCP server processes. Bridge operators SHOULD be aware that:

- All thread participants can see tool inputs (tool-visibility.md Section 4.1).
- The external MCP server receives the full tool input, even if the thread-visible input is redacted.
- Bridge operators are responsible for ensuring that tool inputs comply with the external service's data handling policies.

### 7.4 Bridge Participant in Memory

Bridge participants can store and query archival memory (memory.md) like any other participant. Bridge archival memory is private to the bridge identity. Servers MAY impose separate storage quotas on bridge participants.

Bridge participants receive context frames (memory.md Section 4) when joining threads. The context frame includes active context and conversation summary, which the bridge MAY use to provide context to the external service. Bridge operators SHOULD consider the privacy implications of forwarding AINM conversation context to external services.

## 8. Examples

### 8.1 MCP Bridge: Code Search Tool Call

A user asks the MCP bridge to search for a pattern, and the bridge delivers results through tool visibility events.

**Thread participants:** `alice@messaging.example.com` (human), `mcp-code-tools@agents.example.com` (MCP bridge)

**Step 1:** Alice sends a message mentioning the bridge:

```json
{
  "id": "019580e0-0001-7000-8000-aaaaaaaaaaaa",
  "from": "alice@messaging.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T14:00:00.000Z",
  "type": "message",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "user",
      "text": "@mcp-code-tools can you search for all error handling patterns in the project?",
      "mentions": [
        { "address": "mcp-code-tools@agents.example.com", "offset": 0, "length": 15 }
      ],
      "fallback": "@mcp-code-tools can you search for all error handling patterns in the project?"
    }
  ],
  "signature": "YWxpY2UtbWNwLXJlcXVlc3Qtc2ln..."
}
```

**Step 2:** Bridge sends ToolCallStarted, calls MCP `tools/call`, then sends ToolCallCompleted:

```
Bridge -> Thread: tool.call.started (code_search, input: {query: "error handling"})
Bridge -> MCP Server: tools/call (code_search, {query: "error handling"})
MCP Server -> Bridge: result (5 matches found)
Bridge -> Thread: tool.call.completed (status: "success", duration: 2100ms)
```

**Step 3:** Bridge sends a follow-up message with the full results:

```json
{
  "id": "019580e0-0004-7000-8000-dddddddddddd",
  "from": "mcp-code-tools@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T14:00:02.200Z",
  "type": "message",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Found 5 error handling patterns across the project:",
      "fallback": "Found 5 error handling patterns."
    },
    {
      "type": "code",
      "version": "1.0",
      "source": "agent",
      "language": "plaintext",
      "code": "1. src/api/handler.rs:42    - Result<T, ApiError> pattern\n2. src/db/connection.rs:18  - anyhow::Context wrapping\n3. src/auth/validate.rs:55  - Custom AuthError enum\n4. src/middleware/log.rs:30  - Error logging middleware\n5. src/utils/retry.rs:12    - Retry with exponential backoff",
      "fallback": "5 error handling patterns found in src/"
    }
  ],
  "signature": "bWNwLWJyaWRnZS1yZXN1bHQtbWVzc2FnZS1zaWc..."
}
```

### 8.2 A2A Bridge: Research Task Lifecycle

A user delegates a research task to an external A2A agent through the bridge. The task goes through the full lifecycle: submitted, working, input-required, completed.

**Thread participants:** `alice@messaging.example.com` (human), `a2a-research-agent@agents.example.com` (A2A bridge)

**Step 1:** Alice mentions the A2A bridge in the main thread. Bridge creates a sub-thread for the task.

**Step 2:** Bridge submits the task to the external A2A agent. A2A agent transitions to `working`. Bridge updates presence to `processing` in the sub-thread.

**Step 3:** A2A agent transitions to `input-required`. Bridge sends a FormBlock in the sub-thread asking Alice to select focus areas.

**Step 4:** Alice submits an ActionBlock with her selections. Bridge forwards the input to the A2A agent. A2A agent transitions back to `working`.

**Step 5:** A2A agent transitions to `completed` with result artifacts. Bridge delivers results as content blocks in the sub-thread:

```json
{
  "id": "019580f3-cccc-7000-8000-000000000020",
  "from": "a2a-research-agent@agents.example.com",
  "thread_id": "019580f0-aaaa-7000-8000-000000000001",
  "timestamp": "2026-03-28T14:15:00.000Z",
  "type": "message",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Research complete. Here are the top 5 quantum computing breakthroughs in 2026:\n\n1. **Error-corrected logical qubits** -- Google achieved 10 logical qubits with surface code error correction.\n2. **Quantum advantage in optimization** -- IBM demonstrated practical advantage on supply chain optimization.\n3. **Topological qubit milestone** -- Microsoft created the first stable topological qubit.\n4. **Quantum networking** -- China established a 1000km quantum key distribution network.\n5. **Hybrid quantum-classical algorithms** -- New variational algorithms outperform classical on drug discovery tasks.",
      "fallback": "Research complete: 5 quantum computing breakthroughs summarized."
    },
    {
      "type": "file",
      "version": "1.0",
      "source": "agent",
      "url": "https://agents.example.com/artifacts/019580f3-report.pdf",
      "mime_type": "application/pdf",
      "filename": "quantum-computing-breakthroughs-2026.pdf",
      "size_bytes": 524288,
      "digest": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "fallback": "[File: quantum-computing-breakthroughs-2026.pdf (512 KB, PDF)]"
    }
  ],
  "signature": "YTJhLXJlc2VhcmNoLWNvbXBsZXRlZC1zaWc..."
}
```

**Step 6:** Bridge updates presence back to `available` and posts a summary in the parent thread:

```json
{
  "id": "019580f4-dddd-7000-8000-000000000021",
  "from": "a2a-research-agent@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T14:15:05.000Z",
  "type": "message",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "@alice Research task complete. I identified 5 key quantum computing breakthroughs for 2026. Full results and PDF report are in the task sub-thread.",
      "mentions": [
        { "address": "alice@messaging.example.com", "offset": 0, "length": 6 }
      ],
      "fallback": "Research task complete. Results in sub-thread."
    }
  ],
  "signature": "YTJhLXN1bW1hcnktcGFyZW50LXNpZw..."
}
```

## References

- **identity.md** -- Identity model, registration, address format (bridge identity convention)
- **security.md** -- Message signing, trust annotations (bridge messages signed by bridge identity)
- **content.md** -- Content block base schema, core block types (TextBlock, CodeBlock, FileBlock used by bridges)
- **transport.md** -- JSON-RPC methods, WebSocket binding (bridge uses standard methods)
- **threading.md** -- Thread model, sub-threads (A2A tasks map to sub-threads), @-mentions (trigger mechanism for bridges)
- **agent-card.md** -- Agent Card schema, `ainm.tools` array (MCP tools mapped here), A2A compatibility (Section 5)
- **tool-visibility.md** -- Tool call events (MCP tool calls produce tool.call.started/progress/completed)
- **streaming.md** -- Streaming lifecycle (A2A streaming maps to AINM streaming)
- **interactive.md** -- Interactive blocks (A2A input-required maps to FormBlock/ApprovalBlock)
- **memory.md** -- Memory tiers (bridge participants participate in memory operations)
- **MCP Specification** -- `tools/list`, `tools/call`, `notifications/tools/list_changed`
- **A2A Specification** -- Agent Cards, task lifecycle states, artifacts

---

*AI-Native Messaging Protocol -- Compatibility Bridge Specification v0.1.0*
*Cross-references: identity.md (bridge identity), security.md (signing), content.md (content blocks), transport.md (methods), threading.md (sub-threads), agent-card.md (tool discovery), tool-visibility.md (tool events), streaming.md (streaming), interactive.md (forms/approvals), memory.md (memory operations)*
