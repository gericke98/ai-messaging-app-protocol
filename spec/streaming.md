# Streaming Specification

**AI-Native Messaging Protocol (AINM) -- Version 0.1.0**

## 1. Overview

Streaming enables agents to deliver responses token-by-token in real-time, giving human participants immediate feedback as content is generated. Without streaming, agent responses would arrive only after full generation completes -- introducing seconds or minutes of silence that break the conversational flow. Streaming transforms agent interaction from request-response into a live, progressive experience.

The streaming lifecycle follows a **Start-Delta-End triplet**. A stream begins with a start event that declares the full block layout (types and positions) upfront. Content deltas then carry incremental tokens, addressed to specific blocks by index. The stream ends with a completion or cancellation event that triggers **collapse** -- the assembly of accumulated deltas into a permanent, cryptographically signed message.

Streaming events are **ephemeral JSON-RPC notifications** delivered over the WebSocket connection established during `ainm.initialize` (see `transport.md` Section 4.2). They are NOT persistent signed messages. The distinction is critical for performance: signing every delta (potentially hundreds per second) would introduce prohibitive CPU overhead and latency. Instead, streaming deltas are authenticated by the WebSocket session, and only the collapsed result at stream-end is signed with Ed25519 per `security.md` Section 4.

This specification defines five JSON-RPC methods:

| Method | Type | Direction | Purpose |
|--------|------|-----------|---------|
| `ainm.stream.start` | Notification | Server -> Client | Declare stream and block layout |
| `ainm.stream.delta` | Notification | Server -> Client | Deliver incremental content tokens |
| `ainm.stream.end` | Notification | Server -> Client | Signal stream completion or cancellation |
| `ainm.stream.cancel` | Request | Client -> Server | Request stream cancellation |
| `ainm.stream.error` | Notification | Server -> Client | Report streaming errors |

Streaming requires both client and server to declare `streaming: true` in their capability negotiation during `ainm.initialize` (see `transport.md` Section 4.3). If either side does not support streaming, agents MUST deliver complete messages via `ainm.message.deliver` instead.

## 2. Terminology

| Term | Definition |
|------|-----------|
| **Stream** | A sequence of ephemeral events that progressively deliver the content of a single agent message. A stream begins with `ainm.stream.start` and ends with `ainm.stream.end` or `ainm.stream.error`. |
| **Stream lifecycle** | The three-phase progression of a stream: start (layout declaration), deltas (incremental content), end (collapse into permanent message). |
| **Stream-start** | The `ainm.stream.start` event that initiates a stream. Declares the stream identifier, target message identifier, sender, thread, and the complete block layout. |
| **Content delta** | An `ainm.stream.delta` event that carries one or a few tokens of incremental content, addressed to a specific block within the declared layout. |
| **Stream-end** | The `ainm.stream.end` event that terminates a stream. Carries a status indicating whether the stream completed successfully or was cancelled. |
| **Block layout** | The ordered array of content block declarations in the stream-start event. Each entry specifies the block type, version, and source. The layout is immutable for the duration of the stream. |
| **Block index** | A zero-based integer that identifies a block's position within the block layout. Content deltas reference blocks by their index. |
| **Collapse** | The process of assembling accumulated streaming deltas into a permanent, signed message envelope. Collapse occurs when a stream ends with status `"completed"` or `"cancelled"`. |
| **Ephemeral event** | A JSON-RPC notification that exists only during the streaming session. Ephemeral events are authenticated by the WebSocket session but are not individually signed with Ed25519. Streaming deltas are ephemeral events. |
| **stream_id** | A UUIDv7 identifier assigned to a stream at creation. Used to correlate all events (start, deltas, end, cancel, error) belonging to the same stream. |
| **Parallel streaming** | The ability of an agent to stream content into multiple blocks simultaneously within a single stream. Deltas for different block indices can arrive interleaved. |

## 3. Streaming Lifecycle

Every streaming agent response follows a three-phase lifecycle. The server initiates the stream on behalf of the generating agent, delivers deltas as tokens are produced, and terminates the stream when generation completes, is cancelled, or encounters an error.

### 3.1 Stream Start

**Method:** `ainm.stream.start`
**Type:** JSON-RPC notification (no `id` field, no response expected)
**Direction:** Server -> Client

The stream-start event initiates a new stream and declares the complete block layout upfront. Clients MUST render placeholders for all declared blocks immediately upon receiving this event.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `stream_id` | string | REQUIRED | UUIDv7 identifier for this stream. All subsequent events for this stream reference this identifier. |
| `message_id` | string | REQUIRED | UUIDv7 identifier that the collapsed permanent message will use. Pre-assigned so clients can associate the streaming placeholder with the eventual permanent message. |
| `from` | string | REQUIRED | The sender's AINM address (the generating agent). Same format as the `from` field in the message envelope (see `security.md` Section 3.2). |
| `thread_id` | string | REQUIRED | The UUIDv7 of the target thread. All stream events are scoped to this thread. |
| `timestamp` | string | REQUIRED | RFC 3339 timestamp with millisecond precision and UTC timezone. The time the stream was initiated. |
| `blocks` | array | REQUIRED | Ordered array of block layout declarations. Each entry declares the type, version, and source of a content block that will be populated by subsequent deltas. |

**Block layout entry fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | REQUIRED | Content block type identifier. MUST be a valid block type as defined in `content.md` Section 3. In v1, only `"text"` and `"code"` blocks are streamable. |
| `version` | string | REQUIRED | Block type schema version in `MAJOR.MINOR` format. |
| `source` | string | REQUIRED | Trust annotation: `"user"`, `"agent"`, or `"system"`. See `content.md` Section 3 and `security.md` for semantics. |

**Example:**

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.start",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "message_id": "019580b0-bbbb-7000-8000-000000000002",
    "from": "code-bot@agents.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T12:40:00.000Z",
    "blocks": [
      { "type": "text", "version": "1.0", "source": "agent" },
      { "type": "code", "version": "1.0", "source": "agent" }
    ]
  }
}
```

**Constraints:**

1. The `blocks` array MUST contain at least one entry.
2. The block layout is **immutable** for the duration of the stream. The server MUST NOT add, remove, or reorder blocks after stream-start. If an agent needs additional blocks beyond the declared layout, it MUST end the current stream and either start a new stream or send a separate message via `ainm.message.deliver`.
3. In v1, only `"text"` and `"code"` block types are streamable. Other block types (e.g., `"image"`, `"file"`) MAY appear in the block layout as non-streamable blocks -- they are populated in the collapsed message but do not receive deltas during streaming.
4. Each stream MUST have a unique `stream_id`. A server MUST NOT reuse `stream_id` values.
5. Each stream MUST have a unique `message_id` that is not already in use. This identifier becomes the `id` of the collapsed permanent message.

**Client behavior on stream-start:**

- Render a streaming message placeholder in the thread at the position determined by the `timestamp`.
- Create visual placeholders (e.g., empty block areas, typing indicators) for each block declared in the `blocks` array.
- Display the sender identity from `from`.
- Provide a cancel affordance (e.g., "Stop generating" button) that triggers `ainm.stream.cancel`.

### 3.2 Content Delta

**Method:** `ainm.stream.delta`
**Type:** JSON-RPC notification (no `id` field, no response expected)
**Direction:** Server -> Client

Content deltas deliver incremental tokens to a specific block within the stream's declared layout. Each delta carries a small amount of text content -- typically one to a few tokens as generated by the agent.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `stream_id` | string | REQUIRED | The `stream_id` from the corresponding `ainm.stream.start` event. |
| `block_index` | integer | REQUIRED | Zero-based index into the `blocks` array from stream-start. Identifies which block this delta appends to. |
| `delta` | string | REQUIRED | The incremental content to append. Typically 1 to a few tokens. MUST NOT be empty. |
| `language` | string | OPTIONAL | Programming language identifier for code blocks. Included on the first delta for a `"code"` block to allow clients to initialize syntax highlighting. Subsequent deltas for the same block MAY omit this field. |

**Example -- text delta:**

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.delta",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "block_index": 0,
    "delta": "Here is a function that"
  }
}
```

**Example -- code delta with language:**

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.delta",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "block_index": 1,
    "delta": "fn fibonacci(n: u64) -> u64 {\n",
    "language": "rust"
  }
}
```

**Constraints:**

1. The `block_index` MUST be a valid index within the `blocks` array declared in the corresponding `ainm.stream.start`. A delta with an out-of-range `block_index` is a protocol error (see Section 6, error code `-32041`).
2. The `block_index` MUST reference a streamable block type. In v1, only `"text"` and `"code"` are streamable. A delta targeting a non-streamable block type is a protocol error.
3. Deltas for a given `block_index` MUST be applied in the order received. Clients MUST append each delta's content to the accumulated content for that block.
4. Deltas for different `block_index` values MAY arrive interleaved (see Section 7: Parallel Multi-Block Streaming).
5. A delta MUST NOT be sent after `ainm.stream.end` or `ainm.stream.error` for the same `stream_id`. A delta received after stream termination is a protocol error (see Section 6, error code `-32042`).
6. The `delta` field MUST contain at least one character. Empty deltas are not permitted.

**Client behavior on delta:**

- Append the `delta` content to the rendered output for the block at `block_index`.
- If `language` is provided and the block is a code block, initialize or update syntax highlighting for the specified language.
- Render the updated content immediately without waiting for additional deltas.

### 3.3 Stream End

**Method:** `ainm.stream.end`
**Type:** JSON-RPC notification (no `id` field, no response expected)
**Direction:** Server -> Client

The stream-end event terminates a stream and triggers the collapse process.

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `stream_id` | string | REQUIRED | The `stream_id` from the corresponding `ainm.stream.start` event. |
| `status` | string | REQUIRED | Terminal status of the stream. One of `"completed"` or `"cancelled"`. |

**Example -- completed:**

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.end",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "status": "completed"
  }
}
```

**Example -- cancelled:**

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.end",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "status": "cancelled"
  }
}
```

**Constraints:**

1. After `ainm.stream.end`, no further `ainm.stream.delta` events MUST be sent for the same `stream_id`.
2. Each stream MUST terminate with exactly one `ainm.stream.end` or one `ainm.stream.error`. A stream MUST NOT receive both.
3. The `status` field MUST be either `"completed"` or `"cancelled"`. Any other value is a protocol error.

**Client behavior on stream-end:**

- Stop rendering the streaming state (typing indicators, animation).
- If `status` is `"completed"`: await the collapsed permanent message (delivered via `ainm.message.deliver`) and replace the streaming placeholder with it.
- If `status` is `"cancelled"`: await the collapsed partial message (delivered via `ainm.message.deliver` with `metadata.cancelled: true`) and render it with a visual cancelled indicator.
- Remove the cancel affordance.

See Section 4 for full collapse semantics.

## 4. Collapse Semantics

Collapse is the process by which ephemeral streaming events are consolidated into a permanent, cryptographically signed message. The collapsed message is the authoritative record -- streaming deltas are transient and are not persisted.

### 4.1 Completed Streams

When a stream ends with `status: "completed"`:

1. The server assembles all accumulated deltas into complete content blocks. For each streamable block, the server concatenates all delta content in order to produce the final block content.
2. Non-streamable blocks in the layout (e.g., `"image"`, `"file"`) are populated with their complete content by the server.
3. The server constructs a permanent message envelope using the same format as `ainm.message.deliver` (see `transport.md` Section 4.4):
   - `id`: The `message_id` from stream-start.
   - `from`: The `from` identity from stream-start.
   - `thread_id`: The `thread_id` from stream-start.
   - `timestamp`: The `timestamp` from stream-start.
   - `type`: `"message"`.
   - `content`: The fully assembled content blocks array, where each block includes all fields required by its type definition in `content.md` (including `fallback`).
   - `metadata`: An object containing `stream_id` for audit trail linkage.
   - `signature`: Ed25519 signature computed over the canonical form of the message per `security.md` Section 4.
4. The server delivers the permanent message via `ainm.message.deliver` to all thread participants (including the streaming recipient, who replaces their placeholder).

**Collapsed message example:**

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.message.deliver",
  "params": {
    "id": "019580b0-bbbb-7000-8000-000000000002",
    "from": "code-bot@agents.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T12:40:00.000Z",
    "type": "message",
    "content": [
      {
        "type": "text",
        "version": "1.0",
        "source": "agent",
        "text": "Here is a function that calculates the Fibonacci sequence using iterative approach for optimal performance:",
        "fallback": "Here is a function that calculates the Fibonacci sequence using iterative approach for optimal performance:"
      },
      {
        "type": "code",
        "version": "1.0",
        "source": "agent",
        "language": "rust",
        "code": "fn fibonacci(n: u64) -> u64 {\n    if n <= 1 {\n        return n;\n    }\n    let mut a: u64 = 0;\n    let mut b: u64 = 1;\n    for _ in 2..=n {\n        let temp = a + b;\n        a = b;\n        b = temp;\n    }\n    b\n}",
        "fallback": "```rust\nfn fibonacci(n: u64) -> u64 {\n    if n <= 1 {\n        return n;\n    }\n    let mut a: u64 = 0;\n    let mut b: u64 = 1;\n    for _ in 2..=n {\n        let temp = a + b;\n        a = b;\n        b = temp;\n    }\n    b\n}\n```"
      }
    ],
    "metadata": {
      "stream_id": "019580b0-aaaa-7000-8000-000000000001"
    },
    "signature": "Y29sbGFwc2VkLW1lc3NhZ2Utc2lnbmF0dXJlLWJhc2U2NHVybA..."
  }
}
```

### 4.2 Cancelled Streams

When a stream ends with `status: "cancelled"` (following client-initiated cancellation per Section 5):

1. The server assembles all deltas received up to the point of cancellation into partial content blocks.
2. Blocks that received no deltas are included as empty blocks in the content array.
3. The server constructs a permanent message envelope with:
   - All fields as in Section 4.1.
   - `metadata.stream_id`: The stream identifier.
   - `metadata.cancelled`: `true`.
4. The server signs the partial message and delivers it via `ainm.message.deliver`.

Cancelled messages are permanent records. They appear in thread history and are retrievable. Clients SHOULD render cancelled messages with a visual indicator (e.g., "Generation stopped") to distinguish them from complete messages.

**Cancelled message metadata:**

```json
{
  "metadata": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "cancelled": true
  }
}
```

### 4.3 Collapse Timing

The collapsed permanent message SHOULD be delivered immediately after `ainm.stream.end`. Clients MUST NOT assume the collapsed message will arrive in the same WebSocket frame as the stream-end event. There MAY be a brief delay while the server assembles, signs, and delivers the collapsed message.

If a client disconnects between `ainm.stream.end` and receipt of the collapsed message, the collapsed message is queued for store-and-forward delivery (see `transport.md` Section 4.6) and delivered upon reconnection.

### 4.4 Offline Participants

Thread participants who are not connected when a stream occurs do not receive streaming events. They receive only the collapsed permanent message via store-and-forward delivery. This ensures that streaming is purely an optimization for connected clients -- the permanent message is the canonical representation for all participants regardless of their connection state.

## 5. Client-Initiated Cancellation

A client MAY cancel an active stream by sending `ainm.stream.cancel`. This is the only mechanism for stopping a stream in progress. There is no server-initiated cancellation -- if the server encounters an error, it sends `ainm.stream.error` (Section 6) instead.

**Method:** `ainm.stream.cancel`
**Type:** JSON-RPC request (has `id` field, expects response)
**Direction:** Client -> Server

**Request:**

```json
{
  "jsonrpc": "2.0",
  "id": 42,
  "method": "ainm.stream.cancel",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001"
  }
}
```

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `stream_id` | string | REQUIRED | The `stream_id` of the active stream to cancel. |

**Response (success):**

```json
{
  "jsonrpc": "2.0",
  "id": 42,
  "result": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "cancelled": true
  }
}
```

**Response (error -- stream not found):**

```json
{
  "jsonrpc": "2.0",
  "id": 42,
  "error": {
    "code": -32040,
    "message": "Stream not found",
    "data": {
      "stream_id": "019580b0-aaaa-7000-8000-000000000001"
    }
  }
}
```

**Cancellation flow:**

1. Client sends `ainm.stream.cancel` with the target `stream_id`.
2. Server acknowledges with a success response.
3. Server signals the generating agent to stop.
4. Server sends `ainm.stream.end` with `status: "cancelled"` to all streaming recipients.
5. Server assembles partial content into a cancelled permanent message per Section 4.2.
6. Server delivers the collapsed partial message via `ainm.message.deliver`.

**Constraints:**

1. `ainm.stream.cancel` is a request (has `id` field) because the client needs confirmation that the server received and processed the cancellation.
2. Only the client receiving the stream (or any thread participant) MAY cancel it. The server MUST verify that the sender is a participant in the stream's thread.
3. If the stream has already ended (via `ainm.stream.end` or `ainm.stream.error`), the server MUST return error `-32042` (Stream already ended).
4. Some deltas MAY arrive between the cancel request and the stream-end event due to in-flight processing. Clients MUST handle late-arriving deltas gracefully by appending them to the accumulated content.

**Client behavior:**

- Provide a visible cancel affordance (e.g., "Stop generating" button) during active streaming.
- On receiving the success response, disable the cancel affordance.
- Continue accepting deltas until `ainm.stream.end` arrives.
- Replace the streaming placeholder with the collapsed partial message when it arrives.

## 6. Streaming Errors

When an error occurs during streaming, the server sends `ainm.stream.error` to terminate the stream and report the failure.

**Method:** `ainm.stream.error`
**Type:** JSON-RPC notification (no `id` field, no response expected)
**Direction:** Server -> Client

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `stream_id` | string | REQUIRED | The `stream_id` of the affected stream. |
| `code` | integer | REQUIRED | Error code from the streaming error range (-32040 to -32049). |
| `message` | string | REQUIRED | Human-readable error description. |
| `partial_preserved` | boolean | REQUIRED | Whether partial content accumulated before the error is preserved as a permanent message. |

**Example:**

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.error",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "code": -32043,
    "message": "Agent generation failed: model returned an error during token generation",
    "partial_preserved": true
  }
}
```

### 6.1 Error Codes

Streaming error codes occupy the range `-32040` to `-32049`.

| Code | Name | Description | Partial Preserved |
|------|------|-------------|-------------------|
| `-32040` | Stream not found | The `stream_id` does not reference an active or known stream. This error is returned for `ainm.stream.cancel` requests and MAY be sent as a notification if a delta references an unknown stream. | No |
| `-32041` | Invalid block index | A `ainm.stream.delta` event referenced a `block_index` outside the range declared in `ainm.stream.start`. | Depends -- partial content for valid blocks MAY be preserved. |
| `-32042` | Stream already ended | A `ainm.stream.delta` or `ainm.stream.cancel` was received after the stream has already terminated (via `ainm.stream.end` or `ainm.stream.error`). | No (stream was already terminated). |
| `-32043` | Agent generation error | The generating agent or its underlying model encountered a failure during token generation. This is the most common streaming error. | Yes -- partial content generated before the error is preserved. |
| `-32044` | Stream timeout | The server enforced a maximum stream duration and the stream exceeded it. Servers MAY configure a maximum stream duration to prevent resource exhaustion from runaway generation. | Yes -- partial content generated before timeout is preserved. |

### 6.2 Error Recovery

**When `partial_preserved` is `true`:**

1. The server assembles accumulated deltas into a partial permanent message.
2. The permanent message includes `metadata.stream_id`, `metadata.error_code`, and `metadata.error_message` for diagnostics.
3. The server signs and delivers the partial message via `ainm.message.deliver`.
4. Clients replace the streaming placeholder with the partial message and render an error indicator (e.g., "Generation interrupted -- partial response shown").

**When `partial_preserved` is `false`:**

1. No permanent message is created for this stream.
2. Clients remove the streaming placeholder and display an error state (e.g., "Generation failed").
3. The `message_id` from stream-start is not used and MUST NOT be reused for future messages.

**Client recovery flow:**

```
stream.error received
  |
  +-- partial_preserved: true
  |     |
  |     +-- Wait for collapsed partial message via ainm.message.deliver
  |     +-- Render partial content with error indicator
  |
  +-- partial_preserved: false
        |
        +-- Remove streaming placeholder
        +-- Display error message to user
```

## 7. Parallel Multi-Block Streaming

Agents MAY stream content into multiple blocks simultaneously within a single stream. This enables rich, multi-part responses where different content types are generated in parallel -- for example, an explanatory text block and a code snippet being streamed concurrently.

### 7.1 How It Works

1. The `ainm.stream.start` event declares the full `blocks[]` array with all blocks the agent intends to populate.
2. During streaming, `ainm.stream.delta` events can arrive for any `block_index` in any order. Deltas are not required to arrive sequentially by block index.
3. Clients MUST buffer and render deltas for each block independently. A delta for `block_index: 1` does not depend on any delta for `block_index: 0`.

### 7.2 Interleaved Delta Example

The following sequence shows an agent streaming a text explanation (block 0) and a Rust code snippet (block 1) simultaneously:

```json
// Stream start: 2 blocks declared
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.start",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "message_id": "019580b0-bbbb-7000-8000-000000000002",
    "from": "code-bot@agents.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T12:40:00.000Z",
    "blocks": [
      { "type": "text", "version": "1.0", "source": "agent" },
      { "type": "code", "version": "1.0", "source": "agent" }
    ]
  }
}

// Delta 1: text block starts
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.delta",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "block_index": 0,
    "delta": "Here is a function that calculates"
  }
}

// Delta 2: code block starts with language hint
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.delta",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "block_index": 1,
    "delta": "fn fibonacci(n: u64) -> u64 {\n",
    "language": "rust"
  }
}

// Delta 3: more text
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.delta",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "block_index": 0,
    "delta": " the Fibonacci sequence"
  }
}

// Delta 4: more code
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.delta",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "block_index": 1,
    "delta": "    if n <= 1 {\n        return n;\n    }\n"
  }
}

// Delta 5: text continues
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.delta",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "block_index": 0,
    "delta": " using an iterative approach"
  }
}

// Delta 6: code continues
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.delta",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "block_index": 1,
    "delta": "    let mut a: u64 = 0;\n    let mut b: u64 = 1;\n"
  }
}

// Delta 7: text finishes
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.delta",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "block_index": 0,
    "delta": " for optimal performance:"
  }
}

// Delta 8: code finishes
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.delta",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "block_index": 1,
    "delta": "    for _ in 2..=n {\n        let temp = a + b;\n        a = b;\n        b = temp;\n    }\n    b\n}"
  }
}

// Stream end: completed
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.end",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "status": "completed"
  }
}
```

After stream-end, the server delivers the collapsed permanent message containing the fully assembled text and code blocks.

### 7.3 Single-Block Streaming

Single-block streaming is the common case. The stream-start declares one block, and all deltas target `block_index: 0`. This is functionally identical to parallel streaming with a single block, so no separate mechanism is needed.

### 7.4 Client Capability

Clients declare their support for parallel multi-block streaming via the `parallelStreaming` capability flag during `ainm.initialize` (see Section 8). If a client does not support parallel streaming, agents SHOULD limit streams to a single block. The server MAY still deliver multi-block streams to clients that do not support parallel streaming -- in this case, the client MUST render the blocks in layout order, processing deltas for each block even if they arrive interleaved.

## 8. Capability Negotiation

Streaming capabilities are negotiated during the `ainm.initialize` handshake defined in `transport.md` Section 4.3.

### 8.1 Capability Flags

The following capability flags relate to streaming:

| Flag | Type | Side | Description |
|------|------|------|-------------|
| `streaming` | boolean | Both | Whether streaming is supported at all. If `false`, agents MUST deliver complete messages via `ainm.message.deliver` instead of streaming. Defined in `transport.md` Section 4.3. |
| `parallelStreaming` | boolean | Client | Whether the client supports rendering multiple blocks streaming simultaneously. If `false`, agents SHOULD limit streams to a single block. Servers include this flag to indicate whether they can generate parallel streams. |

**Client capabilities example:**

```json
{
  "capabilities": {
    "streaming": true,
    "parallelStreaming": true,
    "subThreads": true,
    "contentTypes": ["text", "code", "image", "file"]
  }
}
```

**Server capabilities example:**

```json
{
  "capabilities": {
    "streaming": true,
    "parallelStreaming": true,
    "subThreads": true,
    "storeAndForward": true,
    "maxContentBlocks": 50,
    "maxMessageSize": 1048576
  }
}
```

### 8.2 Negotiation Rules

1. **Streaming disabled:** If either client or server declares `streaming: false`, streaming is unavailable for the session. Agents MUST use `ainm.message.deliver` for all responses.
2. **Parallel streaming disabled:** If the client declares `parallelStreaming: false` (or omits it, which defaults to `false`), agents SHOULD send single-block streams only. The server MAY still send multi-block streams, and the client MUST handle them gracefully.
3. **Content types:** The `contentTypes` array in client capabilities indicates which block types the client can render. Agents SHOULD only include block types that the client supports in their stream-start `blocks[]` array. Unsupported block types will render using the `fallback` field in the collapsed message.

## 9. Security Considerations

Streaming introduces a deliberate split between ephemeral and persistent authentication. This section defines the security model for streaming events and the trust implications for clients.

### 9.1 Ephemeral Event Authentication

Streaming delta events (`ainm.stream.start`, `ainm.stream.delta`, `ainm.stream.end`, `ainm.stream.error`) are **ephemeral** -- they are authenticated by the WebSocket session, NOT individually signed with Ed25519.

The WebSocket session was established during `ainm.initialize` (see `transport.md` Section 4.3), which includes identity verification via Ed25519 signature. All events delivered over this authenticated session inherit the session's trust properties.

**Rationale:** Agent streaming can produce hundreds of delta events per second. Requiring Ed25519 signing on each delta would introduce:
- **CPU overhead:** Ed25519 signing is fast (~60 microseconds) but at 100+ deltas/second, signing overhead accumulates and competes with token generation for CPU time.
- **Latency:** Each delta would need to be signed before delivery, adding serialization and signing time to every token.
- **Diminished security value:** The delta content is transient -- the permanent signed message at stream-end is what matters for long-term integrity.

### 9.2 Permanent Message Signing

Only the collapsed message produced at stream-end is signed with Ed25519 per `security.md` Section 4. This message is the authoritative, persistent record of the agent's response. The signing procedure is identical to any other message in the protocol:

1. Construct the message envelope with all fields (id, from, thread_id, timestamp, type, content, metadata).
2. Canonicalize using RFC 8785 (JSON Canonicalization Scheme).
3. Sign with the sender's Ed25519 private key.
4. Include the signature in the `signature` field.

### 9.3 Client Trust Model

Clients MUST treat streaming delta content as **unverified** until the signed collapsed message arrives.

**During streaming:**
- Content is displayed progressively for user experience purposes.
- Clients SHOULD indicate visually that the content is being streamed (e.g., typing indicator, animation).
- Clients MUST NOT treat streamed content as cryptographically verified.

**After collapse:**
- The collapsed message arrives with a valid Ed25519 signature.
- Clients verify the signature per `security.md` Section 5.
- Clients replace the streaming placeholder with the verified permanent message.
- If signature verification fails, clients MUST discard the content and display an error.

**On disconnect before collapse:**
- If the WebSocket connection is lost after streaming starts but before the collapsed message arrives, clients SHOULD mark the streamed content as **unverified**.
- Upon reconnection, the collapsed message (which was queued for store-and-forward delivery) arrives and clients can verify it normally.
- If the collapsed message never arrives (e.g., server failure during collapse), clients SHOULD discard the unverified streaming content or display it with a prominent "unverified" indicator.

### 9.4 Trust Annotations in Streaming

The `source` field in the stream-start `blocks[]` array establishes the trust annotation for each block. This value is carried through to the collapsed message. Agents streaming their own responses MUST set `source: "agent"`. The `source` field is covered by the signature on the collapsed message, making it tamper-evident.

## 10. Error Handling Summary

This section consolidates all error codes and behaviors related to streaming.

### 10.1 Error Code Table

| Code | Name | Triggered By | Behavior |
|------|------|-------------|----------|
| `-32040` | Stream not found | `ainm.stream.cancel` referencing unknown `stream_id` | Return error response to cancel request. |
| `-32041` | Invalid block index | `ainm.stream.delta` with `block_index` outside declared layout range | Server sends `ainm.stream.error`. Partial content for valid blocks MAY be preserved. |
| `-32042` | Stream already ended | `ainm.stream.delta` or `ainm.stream.cancel` after stream termination | For cancel: return error response. For delta: server logs warning, client ignores. |
| `-32043` | Agent generation error | LLM failure, agent crash, or generation pipeline error mid-stream | Server sends `ainm.stream.error` with `partial_preserved: true`. Partial content delivered as cancelled message with error metadata. |
| `-32044` | Stream timeout | Server-enforced maximum stream duration exceeded | Server sends `ainm.stream.error` with `partial_preserved: true`. Partial content delivered. |

### 10.2 Error Behavior Matrix

| Scenario | Server Action | Client Action |
|----------|--------------|---------------|
| Agent model returns error mid-generation | Send `ainm.stream.error` (-32043, partial_preserved: true). Assemble partial content. Deliver as permanent message with error metadata. | Display partial content with error indicator. |
| Stream exceeds server timeout | Send `ainm.stream.error` (-32044, partial_preserved: true). Assemble partial content. Deliver as permanent message with error metadata. | Display partial content with timeout indicator. |
| Client sends cancel for unknown stream | Return error response (-32040). | Display error to user or ignore silently. |
| Delta arrives after stream-end | Log warning. Do not send error notification (stream is already terminated). | Ignore the late delta. |
| Delta references invalid block_index | Send `ainm.stream.error` (-32041). Optionally preserve valid blocks. | Remove streaming placeholder or display partial content with error. |
| Client disconnects mid-stream | Terminate the stream. Assemble partial content. Queue collapsed message for store-and-forward. | On reconnection, receive collapsed message. |
| Server crashes mid-stream | No stream-end or error sent. | After reconnection timeout, mark content as unverified. Discard or display with warning. |

## 11. Examples

### 11.1 Complete Streaming Session

This example shows a full streaming lifecycle: an agent responding with a text explanation and code snippet, streamed in parallel, collapsing into a signed permanent message.

**Step 1: Stream start**

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.start",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "message_id": "019580b0-bbbb-7000-8000-000000000002",
    "from": "code-bot@agents.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T12:40:00.000Z",
    "blocks": [
      { "type": "text", "version": "1.0", "source": "agent" },
      { "type": "code", "version": "1.0", "source": "agent" }
    ]
  }
}
```

Client renders: Two empty placeholder blocks with typing indicator.

**Step 2: Interleaved deltas**

```json
{"jsonrpc":"2.0","method":"ainm.stream.delta","params":{"stream_id":"019580b0-aaaa-7000-8000-000000000001","block_index":0,"delta":"Here is the "}}
{"jsonrpc":"2.0","method":"ainm.stream.delta","params":{"stream_id":"019580b0-aaaa-7000-8000-000000000001","block_index":1,"delta":"fn add(a: i32, b: i32)","language":"rust"}}
{"jsonrpc":"2.0","method":"ainm.stream.delta","params":{"stream_id":"019580b0-aaaa-7000-8000-000000000001","block_index":0,"delta":"implementation you requested:"}}
{"jsonrpc":"2.0","method":"ainm.stream.delta","params":{"stream_id":"019580b0-aaaa-7000-8000-000000000001","block_index":1,"delta":" -> i32 {\n    a + b\n}"}}
```

Client state after deltas:
- Block 0 (text): "Here is the implementation you requested:"
- Block 1 (code, rust): "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}"

**Step 3: Stream end**

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.end",
  "params": {
    "stream_id": "019580b0-aaaa-7000-8000-000000000001",
    "status": "completed"
  }
}
```

**Step 4: Collapsed permanent message delivered**

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.message.deliver",
  "params": {
    "id": "019580b0-bbbb-7000-8000-000000000002",
    "from": "code-bot@agents.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T12:40:00.000Z",
    "type": "message",
    "content": [
      {
        "type": "text",
        "version": "1.0",
        "source": "agent",
        "text": "Here is the implementation you requested:",
        "fallback": "Here is the implementation you requested:"
      },
      {
        "type": "code",
        "version": "1.0",
        "source": "agent",
        "language": "rust",
        "code": "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}",
        "fallback": "```rust\nfn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n```"
      }
    ],
    "metadata": {
      "stream_id": "019580b0-aaaa-7000-8000-000000000001"
    },
    "signature": "Y29sbGFwc2VkLXNpZ25hdHVyZS1iYXNlNjR1cmw..."
  }
}
```

Client replaces streaming placeholder with verified permanent message.

### 11.2 Cancellation Example

**Step 1:** Stream starts with a single text block.

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.start",
  "params": {
    "stream_id": "019580c0-1111-7000-8000-000000000001",
    "message_id": "019580c0-2222-7000-8000-000000000002",
    "from": "writer-agent@agents.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T12:42:00.000Z",
    "blocks": [
      { "type": "text", "version": "1.0", "source": "agent" }
    ]
  }
}
```

**Step 2:** Several deltas arrive.

```json
{"jsonrpc":"2.0","method":"ainm.stream.delta","params":{"stream_id":"019580c0-1111-7000-8000-000000000001","block_index":0,"delta":"The history of distributed computing begins with"}}
{"jsonrpc":"2.0","method":"ainm.stream.delta","params":{"stream_id":"019580c0-1111-7000-8000-000000000001","block_index":0,"delta":" the early work on ARPANET in the 1960s."}}
```

**Step 3:** User clicks "Stop generating". Client sends cancel request.

```json
{
  "jsonrpc": "2.0",
  "id": 50,
  "method": "ainm.stream.cancel",
  "params": {
    "stream_id": "019580c0-1111-7000-8000-000000000001"
  }
}
```

**Step 4:** Server acknowledges cancellation.

```json
{
  "jsonrpc": "2.0",
  "id": 50,
  "result": {
    "stream_id": "019580c0-1111-7000-8000-000000000001",
    "cancelled": true
  }
}
```

**Step 5:** Server sends stream-end with cancelled status.

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.end",
  "params": {
    "stream_id": "019580c0-1111-7000-8000-000000000001",
    "status": "cancelled"
  }
}
```

**Step 6:** Server delivers collapsed partial message.

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.message.deliver",
  "params": {
    "id": "019580c0-2222-7000-8000-000000000002",
    "from": "writer-agent@agents.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T12:42:00.000Z",
    "type": "message",
    "content": [
      {
        "type": "text",
        "version": "1.0",
        "source": "agent",
        "text": "The history of distributed computing begins with the early work on ARPANET in the 1960s.",
        "fallback": "The history of distributed computing begins with the early work on ARPANET in the 1960s."
      }
    ],
    "metadata": {
      "stream_id": "019580c0-1111-7000-8000-000000000001",
      "cancelled": true
    },
    "signature": "Y2FuY2VsbGVkLW1lc3NhZ2Utc2lnbmF0dXJl..."
  }
}
```

Client renders the partial text with a "Generation stopped" indicator.

### 11.3 Error Mid-Stream Example

**Step 1:** Stream starts.

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.start",
  "params": {
    "stream_id": "019580d0-3333-7000-8000-000000000001",
    "message_id": "019580d0-4444-7000-8000-000000000002",
    "from": "research-agent@agents.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T12:44:00.000Z",
    "blocks": [
      { "type": "text", "version": "1.0", "source": "agent" }
    ]
  }
}
```

**Step 2:** Some deltas are delivered.

```json
{"jsonrpc":"2.0","method":"ainm.stream.delta","params":{"stream_id":"019580d0-3333-7000-8000-000000000001","block_index":0,"delta":"Based on my analysis of the dataset,"}}
{"jsonrpc":"2.0","method":"ainm.stream.delta","params":{"stream_id":"019580d0-3333-7000-8000-000000000001","block_index":0,"delta":" the key findings are:"}}
```

**Step 3:** Agent model encounters an error.

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.stream.error",
  "params": {
    "stream_id": "019580d0-3333-7000-8000-000000000001",
    "code": -32043,
    "message": "Agent generation error: model context window exceeded during generation",
    "partial_preserved": true
  }
}
```

**Step 4:** Server delivers collapsed partial message with error metadata.

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.message.deliver",
  "params": {
    "id": "019580d0-4444-7000-8000-000000000002",
    "from": "research-agent@agents.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T12:44:00.000Z",
    "type": "message",
    "content": [
      {
        "type": "text",
        "version": "1.0",
        "source": "agent",
        "text": "Based on my analysis of the dataset, the key findings are:",
        "fallback": "Based on my analysis of the dataset, the key findings are:"
      }
    ],
    "metadata": {
      "stream_id": "019580d0-3333-7000-8000-000000000001",
      "error_code": -32043,
      "error_message": "Agent generation error: model context window exceeded during generation"
    },
    "signature": "ZXJyb3ItcGFydGlhbC1tZXNzYWdlLXNpZ25hdHVyZQ..."
  }
}
```

Client renders the partial text with an error indicator: "Generation interrupted -- partial response shown."

## 12. References

- `transport.md` -- WebSocket transport binding, `ainm.initialize` handshake, capability negotiation, `ainm.message.deliver` notification format, connection lifecycle, store-and-forward delivery.
- `security.md` -- Message envelope structure (Section 3), Ed25519 signing procedure (Section 4), signature verification (Section 5), trust annotations (`source` field).
- `content.md` -- Content block base schema (Section 3: type, version, source, fallback), core block types (Section 4: TextBlock, CodeBlock, ImageBlock, FileBlock), extension type rules (Section 5).
- JSON-RPC 2.0 Specification -- Framing format for all streaming events. Notifications (no `id`) for server-pushed events; requests (with `id`) for `ainm.stream.cancel`.
- RFC 9562 -- UUIDv7 format for `stream_id` and `message_id`.
- RFC 8785 -- JSON Canonicalization Scheme used for producing signing input from collapsed messages.
- RFC 3339 -- Timestamp format for the `timestamp` field in stream-start events.
