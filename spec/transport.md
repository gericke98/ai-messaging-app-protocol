# Transport Specification

**AI-Native Messaging Protocol (AINM) -- Version 0.1.0**

## 1. Overview

Transport defines how messages move between participants via servers in the AI-Native Messaging Protocol. The transport layer sits between the application (messages, threads, presence) and the network, providing reliable delivery with two distinct communication patterns: real-time bidirectional messaging between clients and servers, and store-and-forward relay between federated servers.

AINM defines two transport bindings. The **WebSocket binding** handles client-to-server communication -- persistent, bidirectional connections that support real-time message delivery, streaming agent responses, and presence updates. The **HTTP binding** handles server-to-server federation relay -- stateless POST requests that carry messages between servers in a federated network. Both bindings use the same framing format: JSON-RPC 2.0 over TLS 1.3.

All transport in AINM is encrypted. TLS 1.3 is required for both bindings with no fallback to older versions. The WebSocket binding uses standard TLS via `wss://`. The HTTP binding uses mutual TLS (mTLS) for bidirectional server authentication. Messages transmitted over transport carry content blocks as defined in content.md and are signed per security.md.

## 2. Terminology

| Term | Definition |
|------|-----------|
| **Transport binding** | A specification of how AINM protocol messages are carried over a specific network protocol. AINM defines two bindings: WebSocket and HTTP. |
| **Client** | An endpoint that connects to a server on behalf of a participant (human or agent). A client initiates WebSocket connections and sends messages through its home server. |
| **Server** | An endpoint that accepts client connections and routes messages. Servers participate in federation by relaying messages to other servers via HTTP. |
| **Connection** | A persistent WebSocket connection between a client and server. One connection per client session. |
| **Session** | The logical state associated with an initialized connection. Created during the `ainm.initialize` handshake and destroyed on disconnection or `ainm.shutdown`. |
| **Handshake** | The `ainm.initialize` / `ainm.initialized` exchange that establishes a session. No protocol methods can be called before the handshake completes. |
| **Capability negotiation** | The process during handshake where client and server exchange their supported features. Only features declared by both sides are available for the session. |
| **Streaming mode** | Real-time message delivery to connected clients via WebSocket notifications. The default delivery mode for online participants. |
| **Store-and-forward mode** | Queued message delivery for offline participants. Messages are stored by the server and delivered when the participant reconnects. |
| **Notification** | A JSON-RPC 2.0 message without an `id` field. No response is expected. Used for events like message delivery and presence updates. |
| **Request** | A JSON-RPC 2.0 message with an `id` field. A response (success or error) is expected. |
| **Response** | A JSON-RPC 2.0 message with an `id` field matching a previous request. Contains either a `result` (success) or `error` (failure). |

## 3. JSON-RPC 2.0 Framing

All AINM protocol messages use JSON-RPC 2.0 as the envelope format, regardless of transport binding. This provides a standard, well-tooled framing with built-in request/response correlation, error handling, and notification support.

### 3.1 Request

A request expects a response. The `id` field correlates the response to the request.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "ainm.message.send",
  "params": {
    "id": "019580a1-7b3e-7000-8000-abcdef123456",
    "from": "alice@messaging.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T12:34:56.789Z",
    "type": "message",
    "content": [
      {
        "type": "text",
        "version": "1.0",
        "source": "user",
        "text": "Hello, world!",
        "fallback": "Hello, world!"
      }
    ],
    "signature": "dGhpcyBpcyBhIGJhc2U2NHVybCBlbmNvZGVkIHNpZ25hdHVyZQ..."
  }
}
```

### 3.2 Response (Success)

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "accepted": true,
    "server_timestamp": "2026-03-28T12:34:57.000Z"
  }
}
```

### 3.3 Response (Error)

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32010,
    "message": "Invalid signature",
    "data": {
      "field": "signature",
      "detail": "Ed25519 signature verification failed for sender alice@messaging.example.com"
    }
  }
}
```

### 3.4 Notification

A notification has no `id` field. No response is expected or sent.

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.message.deliver",
  "params": {
    "id": "019580a1-7b3e-7000-8000-abcdef123456",
    "from": "alice@messaging.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T12:34:56.789Z",
    "type": "message",
    "content": [
      {
        "type": "text",
        "version": "1.0",
        "source": "user",
        "text": "Hello, world!",
        "fallback": "Hello, world!"
      }
    ],
    "signature": "dGhpcyBpcyBhIGJhc2U2NHVybCBlbmNvZGVkIHNpZ25hdHVyZQ..."
  }
}
```

### 3.5 Method Namespace

All AINM methods use the `ainm.` prefix to avoid collisions with other JSON-RPC services that might share the same transport. Method names use dot-separated hierarchical naming: `ainm.{domain}.{action}` (e.g., `ainm.message.send`, `ainm.thread.create`, `ainm.presence.update`).

### 3.6 Error Codes

**Standard JSON-RPC error codes** (defined by the JSON-RPC 2.0 specification):

| Code | Name | Description |
|------|------|-------------|
| `-32700` | Parse error | Invalid JSON received. |
| `-32600` | Invalid Request | JSON is valid but not a valid JSON-RPC request. |
| `-32601` | Method not found | The requested method does not exist. |
| `-32602` | Invalid params | Method parameters are invalid. |
| `-32603` | Internal error | An internal server error occurred. |

**AINM security error codes** (defined in security.md):

| Code | Name | Description |
|------|------|-------------|
| `-32010` | Invalid signature | Ed25519 signature verification failed. |
| `-32011` | Unknown identity | Sender identity could not be resolved. |
| `-32012` | Expired key | The signing key has been revoked or expired. |
| `-32013` | Malformed message | Message structure is invalid (missing required fields, invalid content blocks). |

**AINM transport error codes:**

| Code | Name | Description |
|------|------|-------------|
| `-32020` | Unsupported protocol version | Client and server cannot agree on a protocol version. |
| `-32021` | Not initialized | A method was called before the initialization handshake completed. |
| `-32022` | Rate limited | Too many requests from this client or relay source. |
| `-32023` | Server unavailable | Relay target server is unreachable. |
| `-32024` | Thread not found | The specified thread does not exist. |
| `-32025` | Not a thread participant | The sender is not a member of the specified thread. |
| `-32026` | Replay detected | Relay payload failed replay protection (stale timestamp or duplicate relay_id). |

**AINM thread error codes** (normatively defined in threading.md):

| Code | Name | Description |
|------|------|-------------|
| `-32030` | Thread creation failed | Thread could not be created (e.g., invalid parent reference). |
| `-32031` | Already a participant | The identity is already a member of the thread. |

### 3.7 Batch Requests

JSON-RPC 2.0 batch format (an array of request objects) is supported but OPTIONAL for implementations. Servers that do not support batching MUST return a single error response with code `-32600` (Invalid Request) when they receive an array.

## 4. WebSocket Transport Binding

The WebSocket binding provides persistent, bidirectional communication between clients and servers. It is the primary transport for real-time messaging, presence updates, and streaming agent responses.

### 4.1 Connection Establishment

- **URL pattern:** `wss://{server}/ainm/v1`
  - The `{server}` value comes from the `server` field in the participant's identity resolution response (see identity.md).
  - The `/ainm/v1` path includes the major protocol version. Future incompatible protocol versions use different paths (e.g., `/ainm/v2`).
- **TLS 1.3 REQUIRED.** Connections using `ws://` (unencrypted WebSocket) MUST be rejected by both clients and servers. There is no fallback to older TLS versions.
- Standard WebSocket upgrade handshake per RFC 6455.

### 4.2 Connection Lifecycle

The connection lifecycle is modeled on MCP's initialize/initialized/shutdown pattern:

```
Client                          Server
  |                               |
  |--- WebSocket upgrade -------->|  (TLS 1.3 required)
  |<-- 101 Switching Protocols ---|
  |                               |
  |--- ainm.initialize ---------->|  JSON-RPC request
  |<-- result: capabilities ------|  JSON-RPC response
  |--- ainm.initialized --------->|  JSON-RPC notification
  |                               |
  |  [Normal operation phase]     |
  |--- ainm.message.send -------->|  Client sends message
  |<-- ainm.message.deliver ------|  Server delivers message (notification)
  |                               |
  |--- ainm.ping ---------------->|  Keepalive (request)
  |<-- ainm.pong -----------------|  Keepalive (response)
  |                               |
  |--- close frame / ainm.shutdown|  Graceful shutdown
  |<-- close frame ---------------|
```

**Phase 1: WebSocket Upgrade.** Standard RFC 6455 handshake over TLS 1.3. No AINM-specific headers are required during the upgrade.

**Phase 2: Initialization.** Client sends `ainm.initialize` as the first JSON-RPC message. Server responds with capabilities. Client confirms with `ainm.initialized`. No other methods are permitted until initialization completes -- the server MUST return error `-32021` (Not initialized) for any method called before `ainm.initialized` is received.

**Phase 3: Normal Operation.** Client and server exchange messages, presence updates, thread operations. Both sides can send requests and notifications.

**Phase 4: Shutdown.** Either side can initiate shutdown. Graceful shutdown uses `ainm.shutdown` notification followed by WebSocket close frame. Abrupt disconnection (network failure) is handled by the reconnection mechanism (Section 4.5).

### 4.3 Initialize

The initialization handshake establishes the session, authenticates the client, and negotiates capabilities.

**`ainm.initialize`** (client -> server, request)

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "ainm.initialize",
  "params": {
    "protocolVersion": "0.1.0",
    "identity": "alice@messaging.example.com",
    "signature": "aW5pdGlhbGl6YXRpb24tc2lnbmF0dXJlLWJhc2U2NHVybA...",
    "capabilities": {
      "streaming": true,
      "subThreads": true,
      "contentTypes": ["text", "code", "image", "file", "button", "form", "table", "approval", "action"],
      "parallelStreaming": false,
      "toolVisibility": true,
      "memory": {
        "conversation": true,
        "archival": true,
        "maxContextFrameSize": 32768
      }
    }
  }
}
```

**Parameters:**
- `protocolVersion` (string, REQUIRED): The protocol version the client supports. Format: semantic versioning (`MAJOR.MINOR.PATCH`).
- `identity` (string, REQUIRED): The client's AINM address (e.g., `alice@messaging.example.com`).
- `signature` (string, REQUIRED): Base64url-encoded Ed25519 signature of an initialization nonce. The nonce is derived from the connection parameters. See security.md for the signing procedure.
- `capabilities` (object, REQUIRED): Client capabilities.
  - `streaming` (boolean): Whether the client supports streaming message delivery.
  - `subThreads` (boolean): Whether the client supports sub-thread operations.
  - `contentTypes` (array of strings): Content block types the client can render. Phase 1 types: `"text"`, `"code"`, `"image"`, `"file"`. Phase 2 types: `"button"`, `"form"`, `"table"`, `"approval"`, `"action"` (see interactive.md).
  - `parallelStreaming` (boolean, Phase 2): Whether the client supports receiving multi-block streaming where deltas for different block indices arrive interleaved (see streaming.md). Default `false`.
  - `toolVisibility` (boolean, Phase 2): Whether the client supports rendering tool call events (`tool.call.started`, `tool.call.progress`, `tool.call.completed`) as collapsible activity indicators (see tool-visibility.md). Default `false`.
  - `memory` (object, Phase 2): Memory tier support capabilities.
    - `memory.conversation` (boolean): Client/server supports conversation memory queries via `ainm.memory.query` (see memory.md). Default `false`.
    - `memory.archival` (boolean): Client/server supports archival memory operations (`ainm.memory.store`, `ainm.memory.query`, `ainm.memory.evict`) (see memory.md). Default `false`.
    - `memory.maxContextFrameSize` (integer): Maximum context frame size in bytes that the client can accept. Recommended default: `32768` (32 KB). See memory.md Section 4.2 for budget allocation.

**Response (success):**

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "0.1.0",
    "serverInfo": {
      "name": "messaging.example.com",
      "version": "0.1.0"
    },
    "capabilities": {
      "streaming": true,
      "subThreads": true,
      "storeAndForward": true,
      "maxContentBlocks": 50,
      "maxMessageSize": 1048576
    },
    "nonce": "c2VydmVyLWdlbmVyYXRlZC1ub25jZS1mb3ItYXV0aA..."
  }
}
```

**Response fields:**
- `protocolVersion` (string): The version the server will use for this session. If the server cannot support the client's version, it returns error `-32020` (Unsupported protocol version) and closes the connection.
- `serverInfo` (object): Server identification.
  - `name` (string): Server hostname or display name.
  - `version` (string): Server software version.
- `capabilities` (object): Server capabilities. Only features declared by both client and server are available for the session.
  - `streaming` (boolean): Server supports streaming delivery.
  - `subThreads` (boolean): Server supports sub-thread operations.
  - `storeAndForward` (boolean): Server queues messages for offline recipients.
  - `maxContentBlocks` (integer): Maximum number of content blocks per message.
  - `maxMessageSize` (integer): Maximum message payload size in bytes.
- `nonce` (string): Server-generated nonce for client authentication verification.

**Confirmation notification:**

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.initialized"
}
```

The client sends `ainm.initialized` (a notification, no `id`) to confirm the handshake is complete. After this notification, both sides enter normal operation.

**Version negotiation:** The client sends its supported version. The server responds with the version it will use. If the versions are incompatible (different MAJOR version), the server returns error `-32020` and closes the connection. Servers SHOULD support the latest protocol version and MAY support older versions.

**Capability negotiation:** Features not declared by both sides are unavailable. For example, if the client declares `streaming: true` but the server responds `streaming: false`, streaming delivery is not used for this session.

### 4.4 Protocol Methods

#### ainm.message.send (client -> server, request)

Sends a message to a thread. The server validates the signature, checks thread membership, and queues the message for delivery to all thread participants.

**Request params:** The signed message envelope (as defined in security.md). The envelope contains the message `id`, `from`, `thread_id`, `timestamp`, `type`, `content` blocks (as defined in content.md), and `signature`.

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "accepted": true,
    "server_timestamp": "2026-03-28T12:34:57.000Z"
  }
}
```

- `accepted` (boolean): Whether the server accepted the message for delivery.
- `server_timestamp` (string): Server-assigned timestamp (RFC 3339). This is the authoritative ordering timestamp -- clients use their own timestamp for display but the server timestamp for ordering.

**Errors:** `-32010` (invalid signature), `-32013` (malformed message), `-32024` (thread not found), `-32025` (not a participant).

#### ainm.message.deliver (server -> client, notification)

Delivers a message to a connected client. Sent for all messages in threads the client is a participant of.

**Notification params:** The signed message envelope -- the same structure sent via `ainm.message.send`.

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.message.deliver",
  "params": {
    "id": "019580a1-7b3e-7000-8000-abcdef123456",
    "from": "alice@messaging.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T12:34:56.789Z",
    "type": "message",
    "content": [
      {
        "type": "text",
        "version": "1.0",
        "source": "user",
        "text": "Hello, world!",
        "fallback": "Hello, world!"
      }
    ],
    "signature": "dGhpcyBpcyBhIGJhc2U2NHVybCBlbmNvZGVkIHNpZ25hdHVyZQ..."
  }
}
```

No response is expected (notification pattern). Clients receiving this notification SHOULD verify the message signature before displaying the content (see security.md).

#### ainm.thread.create (client -> server, request)

Creates a new thread or sub-thread.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "ainm.thread.create",
  "params": {
    "thread_id": "019580a3-5678-7000-8000-111111111111",
    "parent_thread_id": "019580a0-1234-7000-8000-000000000001",
    "parent_message_id": "019580a1-7b3e-7000-8000-abcdef123456",
    "title": "Debugging the fibonacci implementation",
    "participants": [
      "alice@messaging.example.com",
      "code-bot@agents.example.com"
    ]
  }
}
```

**Parameters:**
- `thread_id` (string, REQUIRED): UUIDv7 for the new thread. Generated by the client.
- `parent_thread_id` (string, OPTIONAL): UUIDv7 of the parent thread. If provided, this creates a sub-thread. If omitted, this creates a top-level thread.
- `parent_message_id` (string, OPTIONAL): UUIDv7 of the message this thread branches from. Used for sub-threads to indicate the branching point.
- `title` (string, OPTIONAL): Human-readable thread title.
- `participants` (array of strings, REQUIRED): Initial participant addresses. The creator MUST be included.

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "thread_id": "019580a3-5678-7000-8000-111111111111",
    "created_at": "2026-03-28T12:38:00.000Z"
  }
}
```

**Errors:** `-32030` (thread creation failed), `-32011` (unknown identity in participants list).

Thread semantics (nesting rules, context inheritance, participant management) are normatively defined in threading.md.

#### ainm.thread.join (client -> server, request)

Adds a participant to an existing thread.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "ainm.thread.join",
  "params": {
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "identity": "data-analyst@agents.example.com"
  }
}
```

**Parameters:**
- `thread_id` (string, REQUIRED): The thread to join.
- `identity` (string, REQUIRED): The AINM address of the participant joining.

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": {
    "joined": true
  }
}
```

**Errors:** `-32024` (thread not found), `-32031` (already a participant).

#### ainm.thread.leave (client -> server, request)

Removes a participant from a thread.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "ainm.thread.leave",
  "params": {
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "identity": "data-analyst@agents.example.com"
  }
}
```

**Parameters:**
- `thread_id` (string, REQUIRED): The thread to leave.
- `identity` (string, REQUIRED): The AINM address of the participant leaving.

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "left": true
  }
}
```

**Errors:** `-32024` (thread not found), `-32025` (not a participant).

#### ainm.presence.update (client -> server, notification)

Updates the client's presence status. This is a notification -- no response is expected.

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.presence.update",
  "params": {
    "identity": "alice@messaging.example.com",
    "status": "online",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "detail": null
  }
}
```

**Parameters:**
- `identity` (string, REQUIRED): The participant's AINM address.
- `status` (string, REQUIRED): Presence status. Valid values depend on participant type:
  - **Human participants:** `"online"`, `"offline"`, `"away"`, `"dnd"` (do not disturb).
  - **Agent participants:** `"available"`, `"busy"`, `"processing"`, `"offline"`.
  - See identity.md for how participant type is determined.
- `thread_id` (string, OPTIONAL): If present, the status applies only to this thread. If omitted, the status applies globally.
- `detail` (string, OPTIONAL): Human-readable detail about the status (e.g., `"Fetching weather data from API"` for an agent in `"processing"` state).

#### ainm.presence.notify (server -> client, notification)

Broadcasts a participant's presence change to other thread members.

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.presence.notify",
  "params": {
    "identity": "weather-bot@agents.example.com",
    "status": "processing",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "detail": "Fetching weather data from API"
  }
}
```

The parameter structure is identical to `ainm.presence.update`. The server broadcasts this notification to all connected participants in the relevant thread (or globally if `thread_id` is omitted).

#### ainm.ping / ainm.pong (keepalive)

Keepalive mechanism to detect dead connections.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 99,
  "method": "ainm.ping",
  "params": {}
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 99,
  "result": {
    "timestamp": "2026-03-28T12:40:00.000Z"
  }
}
```

Clients SHOULD send `ainm.ping` every 30 seconds during idle periods. If the server does not respond with `ainm.pong` within 10 seconds, the client SHOULD consider the connection dead and initiate reconnection (Section 4.5). Servers MAY also send `ainm.ping` to clients to verify liveness.

#### ainm.shutdown (client -> server, notification)

Graceful disconnection.

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.shutdown",
  "params": {
    "reason": "User logged out"
  }
}
```

**Parameters:**
- `reason` (string, OPTIONAL): Human-readable reason for disconnection.

After sending `ainm.shutdown`, the client sends a WebSocket close frame. The server clears session state, marks the participant as offline, and begins queuing messages for store-and-forward delivery.

### 4.5 Reconnection

Clients MUST implement reconnection handling for network failures and server restarts.

**Reconnection procedure:**
1. Detect disconnection (WebSocket close event or ping timeout).
2. Wait with exponential backoff: 1s, 2s, 4s, 8s, 16s, 32s, 60s. After reaching 60s, continue retrying every 60s.
3. Establish a new WebSocket connection.
4. Re-initialize with `ainm.initialize` -- a new session is created.
5. After `ainm.initialized`, the server delivers any queued messages (store-and-forward) in chronological order.

Clients SHOULD add random jitter (0-1s) to backoff intervals to prevent reconnection storms when a server restarts and many clients reconnect simultaneously.

## 5. HTTP Transport Binding

The HTTP binding handles server-to-server communication in a federated network. When Alice on Server A sends a message to Bob on Server B, Server A relays the message to Server B via HTTP POST.

### 5.1 Server-to-Server Relay

- **Endpoint:** `POST https://{target-server}/ainm/v1/relay`
- **Mutual TLS (mTLS) REQUIRED.** Both the originating and target servers present TLS certificates during the handshake. This provides bidirectional authentication -- each server verifies the other's identity.
- **Server discovery:** To relay a message, the originating server resolves the recipient's identity (see identity.md) to determine their home server, then resolves the target server's identity to get its relay endpoint and TLS certificate fingerprint.

### 5.2 Server Identity

Servers publish their identity at a well-known endpoint for federation discovery.

**Endpoint:** `GET https://{domain}/.well-known/ainm/server`

**Response:**
```json
{
  "server_id": "messaging.example.com",
  "public_key": "MCowBQYDK2VwAyEA6bR-Y57oVDQrVixqH_Zt6eGbamVR1N3gILEf0nG1I3U",
  "relay_endpoint": "https://messaging.example.com/ainm/v1/relay",
  "tls_fingerprints": ["sha256:a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2"],
  "supported_versions": ["0.1.0"]
}
```

**Fields:**
- `server_id` (string): The server's identifier, typically its domain name.
- `public_key` (string): Base64url-encoded Ed25519 public key for verifying server-level signatures on relay payloads.
- `relay_endpoint` (string): Full URL of the relay endpoint.
- `tls_fingerprints` (array of strings): Array of SHA-256 fingerprints of the server's TLS certificates in format `sha256:hex-encoded`. Supports certificate rotation by including both current and previous fingerprints (max 2 entries). Receivers MUST accept a connection if the peer certificate matches ANY fingerprint in the array.
- `supported_versions` (array of strings): Protocol versions this server supports.

Implementations SHOULD cache server identity responses with appropriate TTL (from HTTP `Cache-Control` headers, or a default of 1 hour).

### 5.3 Relay Request

The relay request uses JSON-RPC 2.0 framing over HTTP POST. The originating server batches one or more messages for delivery.

**Request:**
```json
POST /ainm/v1/relay HTTP/1.1
Host: messaging.example.com
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "ainm.relay",
  "params": {
    "messages": [
      {
        "id": "019580a1-7b3e-7000-8000-abcdef123456",
        "from": "alice@agents.example.com",
        "thread_id": "019580a0-1234-7000-8000-000000000001",
        "timestamp": "2026-03-28T12:34:56.789Z",
        "type": "message",
        "content": [
          {
            "type": "text",
            "version": "1.0",
            "source": "user",
            "text": "Hello from Server A!",
            "fallback": "Hello from Server A!"
          }
        ],
        "signature": "dXNlci1tZXNzYWdlLXNpZ25hdHVyZQ..."
      }
    ],
    "from_server": "agents.example.com",
    "relay_timestamp": "2026-03-28T12:34:56.789Z",
    "relay_id": "019580a1-7b3e-7000-8000-ffffffffffff",
    "server_signature": "c2VydmVyLWxldmVsLXNpZ25hdHVyZS1vZi1yZWxheS1wYXlsb2Fk..."
  }
}
```

**Parameters:**
- `messages` (array, REQUIRED): Array of signed message envelopes to relay. Each message carries the original sender's Ed25519 signature.
- `from_server` (string, REQUIRED): The originating server's identifier (matches `server_id` in the server identity response).
- `relay_timestamp` (string, REQUIRED): RFC 3339 timestamp of when the relay payload was created. Receivers MUST reject payloads where `relay_timestamp` is more than 5 minutes in the past or more than 1 minute in the future.
- `relay_id` (string, REQUIRED): UUIDv7 identifier unique to this relay payload. Receivers MUST track seen `relay_id` values within the replay window and reject duplicates.
- `server_signature` (string, REQUIRED): Base64url-encoded Ed25519 signature of the relay payload by the originating server. The signed content is the JCS-canonicalized `params` object with the `server_signature` field removed. Note that `relay_timestamp` and `relay_id` ARE included in the signed payload (only `server_signature` itself is excluded).

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "accepted": 1,
    "rejected": 0,
    "errors": []
  }
}
```

**Response fields:**
- `accepted` (integer): Number of messages accepted for delivery.
- `rejected` (integer): Number of messages rejected.
- `errors` (array): Error details for rejected messages. Each error contains `message_id` and `reason`.

### 5.4 Relay Validation

The receiving server performs four levels of validation on relay requests:

0. **Replay protection:** The `relay_timestamp` MUST be within 5 minutes of the receiver's current time. The `relay_id` MUST NOT have been seen before within the replay window. If either check fails, the relay is rejected with error code `-32026` (ReplayDetected).
1. **mTLS certificate validation:** The originating server's TLS certificate fingerprint MUST match one of the entries in the `tls_fingerprints` array from its server identity response at `/.well-known/ainm/server`.
2. **Server signature validation:** The `server_signature` is verified against the originating server's `public_key` from the server identity response.
3. **Individual message signature validation:** Each message's `signature` is verified against the sender's public key (resolved via identity.md). This ensures end-to-end message integrity -- even if a relay server is compromised, it cannot forge messages from individual participants.

If any validation level fails, the relay request is rejected with the appropriate error code (`-32026` for replay detection, `-32010` for signature failures, `-32011` for unknown server identity).

### 5.5 Backfill

Remote servers can request thread message history from the origin server via the `ainm.backfill` method on the relay endpoint.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "ainm.backfill",
  "params": {
    "thread_id": "messaging.example.com:019580a0-1234-7000-8000-000000000001",
    "after_sequence": 0,
    "limit": 50
  }
}
```

**Parameters:**
- `thread_id` (string, REQUIRED): Thread ID to backfill history for.
- `after_sequence` (integer, optional): Sequence number cursor. Returns messages with sequence > `after_sequence`. Default 0 (from start).
- `limit` (integer, optional): Maximum messages to return. Default 50, max 200.

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "messages": [...],
    "has_more": true,
    "next_cursor": 50
  }
}
```

**Authorization:** The requesting server MUST have at least one current participant in the thread. Requests from servers without a participant are rejected with error code `-32025` (NotAParticipant).

**Visibility:** Backfill returns only messages with sequence numbers after the requesting server's earliest participant joined the thread. Pre-join history is not accessible.

**Pagination:** Use `next_cursor` as the `after_sequence` value in subsequent requests when `has_more` is true.

## 6. Delivery Modes

AINM supports two delivery modes that operate automatically based on recipient connection status.

### 6.1 Streaming Mode (Foreground)

Streaming mode is the default for connected clients. Messages are delivered immediately as they arrive.

- **Activation:** Automatic when a recipient has an active WebSocket session (completed `ainm.initialized` handshake).
- **Delivery:** Server sends `ainm.message.deliver` notification immediately upon receiving a valid message for a thread the client is a participant of.
- **Ordering:** Messages are delivered in the order the server receives them. The `server_timestamp` in the `ainm.message.send` response provides the authoritative ordering.
- **Real-time:** There is no batching or delay. Messages, presence updates, and thread events are pushed to clients as they occur.

### 6.2 Store-and-Forward Mode (Background/Offline)

Store-and-forward mode activates automatically when a recipient is not connected.

- **Activation:** When a message targets a thread where one or more participants have no active WebSocket session.
- **Storage:** The server queues the message for later delivery. Messages are stored in the server's message store (implementation-specific).
- **Delivery on reconnect:** After a client completes re-initialization (`ainm.initialized`), the server delivers all queued messages in chronological order (by `server_timestamp`).
- **Queue semantics:**
  - Servers SHOULD retain queued messages for at least 7 days.
  - Servers MAY communicate their retention policy during capability exchange (e.g., a `messageRetentionDays` capability).
  - Messages are delivered in `server_timestamp` order -- earliest first.
  - **At-least-once delivery:** Network failures during delivery may cause the same message to be delivered more than once. Clients MUST handle deduplication by tracking received message `id` values (UUIDv7). See security.md for the deduplication mechanism.

### 6.3 Mode Transitions

A participant transitions between modes automatically:

- **Online -> Offline:** Client disconnects (network failure, `ainm.shutdown`, or ping timeout). Server switches to store-and-forward for this participant.
- **Offline -> Online:** Client reconnects and completes `ainm.initialize` / `ainm.initialized`. Server delivers queued messages, then switches to streaming mode.

The sending client does not need to know which mode the recipient is in. The server handles mode selection transparently.

## 7. Error Handling

### Transport-Specific Errors

| Code | Name | When Returned |
|------|------|---------------|
| `-32020` | Unsupported protocol version | During `ainm.initialize` when client and server protocol versions are incompatible (different MAJOR version). |
| `-32021` | Not initialized | Any method is called before the `ainm.initialize` / `ainm.initialized` handshake completes. |
| `-32022` | Rate limited | Client or relay source has exceeded the server's rate limit. The `data` field SHOULD include `retry_after` (seconds until the client can retry). |
| `-32023` | Server unavailable | During relay, the target server is unreachable or did not respond within the timeout. |
| `-32024` | Thread not found | The `thread_id` in the request does not match any known thread. |
| `-32025` | Not a thread participant | The sender is not a member of the specified thread and therefore cannot send messages or perform operations on it. |
| `-32026` | Replay detected | Relay payload failed replay protection (stale `relay_timestamp` or duplicate `relay_id`). |

### Error Response Format

All errors follow the JSON-RPC 2.0 error response format:

```json
{
  "jsonrpc": "2.0",
  "id": 7,
  "error": {
    "code": -32022,
    "message": "Rate limited",
    "data": {
      "retry_after": 30,
      "limit": "100 requests per minute"
    }
  }
}
```

The `data` field is OPTIONAL and provides additional context specific to the error type.

## 8. Security Considerations

### TLS 1.3 Requirement

TLS 1.3 is REQUIRED for both WebSocket and HTTP transport bindings. There is no fallback to TLS 1.2 or earlier. This requirement ensures:

- Forward secrecy on all connections.
- Reduced handshake latency (1-RTT for new connections, 0-RTT for resumed).
- Removal of known-weak cipher suites.

Implementations MUST reject connections that do not negotiate TLS 1.3. For the WebSocket binding, this means only `wss://` URLs are valid. For the HTTP binding, both sides of the mTLS handshake MUST use TLS 1.3.

### Server-to-Server Relay Security

The HTTP relay binding uses three layers of security:

1. **mTLS:** Bidirectional TLS authentication ensures both servers are who they claim to be.
2. **Server signatures:** The `server_signature` on relay payloads ensures the originating server authorized the relay.
3. **Message signatures:** Individual Ed25519 signatures on each message ensure end-to-end integrity from sender to recipient, even across relay hops.

This defense-in-depth approach means compromising any single layer does not compromise message integrity.

### Rate Limiting

Servers SHOULD implement rate limiting to protect against abuse:

- **Per-client session:** Limit the number of requests per time window from a single WebSocket session.
- **Per-relay source:** Limit the number of relay requests per time window from a single federated server.
- **Per-thread:** Limit the message rate within individual threads to prevent flood attacks.

Rate limit responses use error code `-32022` with a `retry_after` value in the error `data`.

### Connection Hijacking Prevention

The initialization handshake (`ainm.initialize`) requires the client to provide its identity and a signed nonce. This prevents:

- **Session hijacking:** An attacker cannot take over an established session without the participant's Ed25519 private key.
- **Replay attacks:** The server-generated nonce ensures initialization signatures cannot be reused.
- **Identity spoofing:** The server verifies the identity claim against the resolved public key during initialization.

## 9. Phase 2 Extensions

This section documents protocol extensions introduced in Phase 2 (agent-native features). Phase 2 adds new JSON-RPC methods for streaming, memory, and context management, as well as new error code ranges for Phase 2 features. All Phase 2 methods follow the existing `ainm.{domain}.{action}` naming convention (Section 3.5) and use the same JSON-RPC 2.0 framing.

### 9.1 Phase 2 Method Registry

The following methods are defined by Phase 2 specifications. Each method's full definition (parameters, responses, constraints) is in the referenced spec file.

| Method | Type | Direction | Spec | Description |
|--------|------|-----------|------|-------------|
| `ainm.stream.start` | Notification | Server -> Client | streaming.md | Declares a new stream and its block layout. Initiates progressive content delivery. |
| `ainm.stream.delta` | Notification | Server -> Client | streaming.md | Delivers incremental content tokens to a specific block within a stream. |
| `ainm.stream.end` | Notification | Server -> Client | streaming.md | Terminates a stream. Triggers collapse into a permanent signed message. |
| `ainm.stream.cancel` | Request | Client -> Server | streaming.md | Client requests cancellation of an active stream. |
| `ainm.stream.error` | Notification | Server -> Client | streaming.md | Reports a streaming error. Terminates the stream abnormally. |
| `ainm.memory.store` | Request | Client -> Server | memory.md | Stores a structured fact in the authenticated participant's archival memory. |
| `ainm.memory.query` | Request | Client -> Server | memory.md | Queries conversation memory (thread history) or archival memory (cross-conversation facts). |
| `ainm.memory.evict` | Request | Client -> Server | memory.md | Removes a fact from the authenticated participant's archival memory. |
| `ainm.context.frame` | Notification | Server -> Client | memory.md | Delivers a structured context frame to a participant on thread join or context refresh. |

**Availability:** Phase 2 methods are available only when both client and server declare support during capability negotiation (Section 4.3). Specifically:

- Streaming methods (`ainm.stream.*`) require `streaming: true` on both sides.
- Memory methods (`ainm.memory.*`, `ainm.context.frame`) require the corresponding `memory.*` capability flags.
- Tool call events (`tool.call.started`, `tool.call.progress`, `tool.call.completed`) are message types (not methods) delivered via `ainm.message.deliver`. They require `toolVisibility: true` on the client for collapsible rendering; clients without this capability still receive the events but render them via TextBlock fallback.

### 9.2 Phase 2 Error Code Ranges

Phase 2 specifications define error codes in the following ranges. Each range is owned by the referenced spec file, which provides the normative definition of each error code.

| Range | Domain | Spec | Description |
|-------|--------|------|-------------|
| `-32040` to `-32049` | Streaming errors | streaming.md | Errors during stream lifecycle: invalid stream_id (`-32040`), invalid block_index (`-32041`), delta after stream end (`-32042`), stream not found for cancel (`-32043`), stream already ended (`-32044`). |
| `-32050` to `-32059` | Memory errors | memory.md | Errors during memory operations: fact not found (`-32050`), storage quota exceeded (`-32051`), invalid memory query (`-32052`), archival memory not supported (`-32053`), context frame too large (`-32054`). |
| `-32060` to `-32069` | Agent Card errors | agent-card.md | Errors during Agent Card discovery: card not found (`-32060`), invalid card format (`-32061`), version mismatch (`-32062`). |
| `-32070` to `-32079` | Interactive content errors | interactive.md | Errors during interactive block processing: invalid action_id (`-32070`), approval already resolved (`-32071`), form validation failed (`-32072`). |

These ranges are reserved and MUST NOT be used by implementations for custom error codes. Custom server-specific error codes SHOULD use the range `-32000` to `-32099` outside of the reserved ranges listed above and in Section 3.6.

### 9.3 Phase 2 Capability Negotiation Example

A client supporting all Phase 2 features negotiates with a server:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "ainm.initialize",
  "params": {
    "protocolVersion": "0.1.0",
    "identity": "alice@messaging.example.com",
    "signature": "aW5pdC1zaWduYXR1cmU...",
    "capabilities": {
      "streaming": true,
      "subThreads": true,
      "contentTypes": ["text", "code", "image", "file", "button", "form", "table", "approval", "action"],
      "parallelStreaming": true,
      "toolVisibility": true,
      "memory": {
        "conversation": true,
        "archival": true,
        "maxContextFrameSize": 65536
      }
    }
  }
}
```

Server response with Phase 2 capabilities:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "0.1.0",
    "serverInfo": {
      "name": "messaging.example.com",
      "version": "0.2.0"
    },
    "capabilities": {
      "streaming": true,
      "subThreads": true,
      "storeAndForward": true,
      "maxContentBlocks": 50,
      "maxMessageSize": 1048576,
      "parallelStreaming": true,
      "toolVisibility": true,
      "memory": {
        "conversation": true,
        "archival": true,
        "maxContextFrameSize": 32768
      }
    },
    "nonce": "c2VydmVyLW5vbmNl..."
  }
}
```

The effective session capabilities are the intersection: both sides support streaming, parallelStreaming, toolVisibility, and memory. The effective `maxContextFrameSize` is `min(65536, 32768) = 32768` bytes (the more restrictive value).

## 10. Examples

### Example 1: Full WebSocket Session

A complete client session from connection to shutdown.

```
Step 1: WebSocket Upgrade
  Client opens wss://messaging.example.com/ainm/v1
  Server responds with 101 Switching Protocols

Step 2: Initialize
```
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "ainm.initialize",
  "params": {
    "protocolVersion": "0.1.0",
    "identity": "alice@messaging.example.com",
    "signature": "aW5pdC1zaWduYXR1cmU...",
    "capabilities": {
      "streaming": true,
      "subThreads": true,
      "contentTypes": ["text", "code", "image", "file"]
    }
  }
}
```
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "0.1.0",
    "serverInfo": { "name": "messaging.example.com", "version": "0.1.0" },
    "capabilities": {
      "streaming": true,
      "subThreads": true,
      "storeAndForward": true,
      "maxContentBlocks": 50,
      "maxMessageSize": 1048576
    },
    "nonce": "c2VydmVyLW5vbmNl..."
  }
}
```
```json
{
  "jsonrpc": "2.0",
  "method": "ainm.initialized"
}
```
```
Step 3: Send a message
```
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "ainm.message.send",
  "params": {
    "id": "019580a1-7b3e-7000-8000-abcdef123456",
    "from": "alice@messaging.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T12:34:56.789Z",
    "type": "message",
    "content": [
      {
        "type": "text",
        "version": "1.0",
        "source": "user",
        "text": "Can someone help me debug this?",
        "fallback": "Can someone help me debug this?"
      }
    ],
    "signature": "bWVzc2FnZS1zaWduYXR1cmU..."
  }
}
```
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": { "accepted": true, "server_timestamp": "2026-03-28T12:34:57.000Z" }
}
```
```
Step 4: Receive a message from another participant
```
```json
{
  "jsonrpc": "2.0",
  "method": "ainm.message.deliver",
  "params": {
    "id": "019580a2-9c4f-7000-8000-fedcba987654",
    "from": "code-bot@agents.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T12:35:02.123Z",
    "type": "message",
    "content": [
      {
        "type": "text",
        "version": "1.0",
        "source": "agent",
        "text": "I can help! What error are you seeing?",
        "fallback": "I can help! What error are you seeing?"
      }
    ],
    "signature": "YWdlbnQtbWVzc2FnZS1zaWduYXR1cmU..."
  }
}
```
```
Step 5: Graceful shutdown
```
```json
{
  "jsonrpc": "2.0",
  "method": "ainm.shutdown",
  "params": { "reason": "User closed the app" }
}
```
```
  Client sends WebSocket close frame (1000, "Normal Closure")
  Server sends WebSocket close frame (1000)
  Connection closed.
```

### Example 2: Server-to-Server Federation Relay

Alice on Server A sends a message to Bob on Server B.

```
Step 1: Alice sends message via WebSocket to Server A
  Server A accepts the message (ainm.message.send)
  Server A resolves Bob's identity: bob@messaging-b.example.com
  Bob's identity response indicates server: wss://messaging-b.example.com/ainm/v1

Step 2: Server A resolves Server B's federation identity
  GET https://messaging-b.example.com/.well-known/ainm/server
```
```json
{
  "server_id": "messaging-b.example.com",
  "public_key": "MCowBQYDK2VwAyEAx9z2B3c4D5e6F7g8H9i0J1k2L3m4N5o6P7q8R9s0T1U",
  "relay_endpoint": "https://messaging-b.example.com/ainm/v1/relay",
  "tls_fingerprints": ["sha256:f1e2d3c4b5a6978869574a3b2c1d0e1f2a3b4c5d6e7f8091a2b3c4d5e6f7a8b9"],
  "supported_versions": ["0.1.0"]
}
```
```
Step 3: Server A relays the message to Server B via HTTP POST (mTLS)
```
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "ainm.relay",
  "params": {
    "messages": [
      {
        "id": "019580a1-7b3e-7000-8000-abcdef123456",
        "from": "alice@agents-a.example.com",
        "thread_id": "019580a0-1234-7000-8000-000000000001",
        "timestamp": "2026-03-28T12:34:56.789Z",
        "type": "message",
        "content": [
          {
            "type": "text",
            "version": "1.0",
            "source": "user",
            "text": "Hello Bob, from across the federation!",
            "fallback": "Hello Bob, from across the federation!"
          }
        ],
        "signature": "YWxpY2UtbWVzc2FnZS1zaWduYXR1cmU..."
      }
    ],
    "from_server": "agents-a.example.com",
    "relay_timestamp": "2026-03-28T12:34:57.000Z",
    "relay_id": "019580a1-8c4f-7000-8000-aaaaaaaaaaaa",
    "server_signature": "c2VydmVyLWEtcmVsYXktc2lnbmF0dXJl..."
  }
}
```
```
Step 4: Server B validates and delivers
  0. Replay protection: verify relay_timestamp is within 5 minutes, relay_id not seen before
  1. Verify mTLS certificate fingerprint matches one of agents-a.example.com tls_fingerprints
  2. Verify server_signature against agents-a.example.com public key
  3. Verify Alice's message signature against alice@agents-a.example.com public key
  4. If Bob is online: deliver via ainm.message.deliver (streaming mode)
  5. If Bob is offline: queue for store-and-forward delivery
```
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": { "accepted": 1, "rejected": 0, "errors": [] }
}
```

### Example 3: Reconnection with Queued Message Delivery

A client loses connection and reconnects to receive queued messages.

```
Step 1: Alice is online, sends messages normally.
Step 2: Alice's network drops. Server detects via ping timeout (10s).
  Server marks Alice as offline.
  Server begins queuing messages for Alice's threads.

Step 3: During offline period, two messages arrive for Alice's threads.
  Server stores both messages in the queue.

Step 4: Alice's network recovers. Client initiates reconnection.
  Backoff: waits 1s, attempts WebSocket connection.
  Connection succeeds.

Step 5: Re-initialization
```
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "ainm.initialize",
  "params": {
    "protocolVersion": "0.1.0",
    "identity": "alice@messaging.example.com",
    "signature": "cmVjb25uZWN0LXNpZ25hdHVyZQ...",
    "capabilities": {
      "streaming": true,
      "subThreads": true,
      "contentTypes": ["text", "code", "image", "file"]
    }
  }
}
```
```
  Server responds with capabilities, client sends ainm.initialized.

Step 6: Server delivers queued messages in chronological order
```
```json
{
  "jsonrpc": "2.0",
  "method": "ainm.message.deliver",
  "params": {
    "id": "019580b0-1111-7000-8000-aaaaaaaaaaaa",
    "from": "code-bot@agents.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T12:40:00.000Z",
    "type": "message",
    "content": [
      {
        "type": "text",
        "version": "1.0",
        "source": "agent",
        "text": "I found the bug -- it's a null pointer on line 42.",
        "fallback": "I found the bug -- it's a null pointer on line 42."
      }
    ],
    "signature": "cXVldWVkLW1lc3NhZ2UtMS1zaWc..."
  }
}
```
```json
{
  "jsonrpc": "2.0",
  "method": "ainm.message.deliver",
  "params": {
    "id": "019580b1-2222-7000-8000-bbbbbbbbbbbb",
    "from": "bob@messaging.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T12:41:00.000Z",
    "type": "message",
    "content": [
      {
        "type": "text",
        "version": "1.0",
        "source": "user",
        "text": "Nice find! I'll push a fix.",
        "fallback": "Nice find! I'll push a fix."
      }
    ],
    "signature": "cXVldWVkLW1lc3NhZ2UtMi1zaWc..."
  }
}
```
```
Step 7: Normal streaming mode resumes.
  Alice's client deduplicates any messages already received before disconnection
  using the message id field (UUIDv7).
```

---

*AI-Native Messaging Protocol -- Transport Specification v0.1.0*
*Cross-references: security.md (message signing, identity verification), content.md (content blocks), identity.md (address resolution, server discovery), threading.md (thread semantics), streaming.md (Phase 2 streaming methods), memory.md (Phase 2 memory methods), agent-card.md (Phase 2 Agent Card errors), tool-visibility.md (Phase 2 tool events), interactive.md (Phase 2 interactive content errors), compatibility.md (Phase 2 bridge integration)*
