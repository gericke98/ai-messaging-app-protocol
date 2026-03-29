# Phase 1: Protocol Kernel - Research

**Researched:** 2026-03-28
**Domain:** Protocol specification writing -- identity, security, transport, content format, threading for AI-native messaging
**Confidence:** HIGH

## Summary

Phase 1 is a **spec-writing phase**, not an implementation phase. The output is a set of modular markdown specification documents in `spec/` that a developer can read and independently implement. The technical domain spans five interconnected areas: identity resolution (email-like addresses with Ed25519 key binding), message security (canonical JSON signing per RFC 8785), transport bindings (JSON-RPC 2.0 over WebSocket and HTTP), content blocks (typed, extensible, with fallback), and threading (flat conversations with sub-threads supporting mixed human-agent participation).

The landscape research reveals that AMP (Agent Messaging Protocol) is the closest existing protocol, with a similar address format, Ed25519 signing, and federation model. However, AMP is very early (v0.1.2), uses a simpler signing format (pipe-delimited fields + SHA256 of payload), and lacks content blocks, sub-threading, and mixed participant semantics. MCP provides the best model for spec document structure (JSON-RPC lifecycle, capability negotiation, clear normative language). Matrix provides lessons on threading complexity and store-and-forward delivery. The A2A protocol's Agent Card at `.well-known/agent-card.json` validates the `.well-known` pattern for identity/capability discovery.

**Primary recommendation:** Write five standalone spec files (`identity.md`, `security.md`, `transport.md`, `content.md`, `threading.md`) using MCP's spec style as the organizational model -- normative RFC keywords used sparingly, concrete JSON examples inline with every definition, and a clear connection lifecycle pattern (handshake, operation, shutdown).

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Modular per-domain files: `spec/identity.md`, `spec/security.md`, `spec/transport.md`, `spec/content.md`, `spec/threading.md`. Each stands alone.
- **D-02:** Developer-friendly prose style. Use RFC keywords (MUST/SHALL/MAY) for normative statements only, not throughout. Think Stripe API docs meets protocol spec.
- **D-03:** Inline JSON examples throughout -- every message type gets a concrete example right next to its definition.
- **D-04:** Spec files live at `spec/` directory at project root.
- **D-05:** `.well-known` URL pattern for identity resolution. `GET https://{provider}/.well-known/ainm/{name}` returns identity data.
- **D-06:** Flat JSON response: `{ "id", "public_key", "server", "type", "agent_card_url" }`. One request to get everything needed to communicate.
- **D-07:** Key rotation via key history array -- current key + array of previous keys with validity periods. Old messages remain verifiable.
- **D-08:** Participant type (human/agent) exposed in identity metadata via `type` field. Protocol treats both equally but clients can adapt UI (e.g., presence semantics).
- **D-09:** Lean envelope: routing (from, thread_id), message metadata (id, timestamp, type), signature, and content blocks array. Trust annotations live INSIDE content blocks, not the envelope.
- **D-10:** Trust annotations via `source` field per content block: `user`, `agent`, or `system`. Set by sender, verified by signature. Directly addresses prompt injection.
- **D-11:** Thread-addressed messages -- messages sent TO a thread (thread_id), not to individual participants. All thread members receive. DMs = 2-person thread.
- **D-12:** At-least-once delivery with client-side deduplication via unique message IDs.
- **D-13:** Closed core set + namespaced extensions. Core types (text, code, image, file) are protocol-defined. Third parties use reverse-domain namespacing (`com.example.custom-chart`). Clients MUST render core, MAY render extensions.
- **D-14:** Mandatory `fallback` field on every content block -- plain text or markdown. Unknown block types render the fallback. Guarantees graceful degradation.
- **D-15:** Block-level `version` field. Each block type can evolve independently from the protocol version. Clients check version to know which fields to expect.

### Claude's Discretion

No areas deferred to Claude's discretion -- all gray areas resolved with explicit decisions.

### Deferred Ideas (OUT OF SCOPE)

None -- discussion stayed within phase scope.

</user_constraints>

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| IDNT-01 | Humans and agents use email-like addresses (`name@provider`) | AMP uses `agent-name@tenant.provider` as precedent. D-05 locks the format. RFC 8615 `.well-known` pattern is standard for discovery. |
| IDNT-02 | Identity format is uniform -- no distinction between human and agent addresses at the protocol level | D-08 puts type in metadata, not address. AMP similarly uses uniform addressing. |
| IDNT-03 | Identity is cryptographically bound to a signing key pair | D-06 includes `public_key` in identity response. Ed25519 key pair binding is standard (AMP, JCS Ed25519 Signature 2020). |
| IDNT-04 | Protocol defines identity registration and resolution via the `@provider` portion | D-05 defines `.well-known/ainm/{name}` resolution. D-06 defines response format. Registration requires spec for key submission. |
| IDNT-05 | Protocol defines key rotation mechanism for identities | D-07 defines key history array with validity periods. OpenSSH key rotation (6.8+) provides precedent for history-based approach. |
| SECU-01 | Every message is signed with Ed25519 by the sender | D-09 includes signature in envelope. Ed25519 via `ring` (Rust) / CryptoKit (iOS). |
| SECU-02 | Signatures use a canonical signing format (deterministic field ordering per RFC 8785) | RFC 8785 JCS fully researched. `serde_json_canonicalizer` crate exists for Rust. Signing procedure: JCS canonicalize -> UTF-8 bytes -> Ed25519 sign. |
| SECU-03 | Receivers can verify message authenticity and integrity via signature validation | Verification = resolve sender identity -> get public key -> JCS canonicalize received message (minus signature) -> Ed25519 verify. |
| SECU-04 | Protocol defines trust annotations on message content blocks | D-10 defines `source` field per content block: `user`/`agent`/`system`. Set by sender, verified by signature. |
| SECU-05 | Content and instructions are structurally separated at protocol message level | D-10's trust annotations + D-09's content blocks array provide structural separation. Agents can filter by `source` to distinguish user input from system instructions. |
| TRNS-01 | Protocol defines WebSocket transport binding for client-to-server real-time communication | JSON-RPC 2.0 over WebSocket is the standard pattern (MCP, A2A both use it). MCP lifecycle model (initialize -> operation -> shutdown) is the template. |
| TRNS-02 | Protocol defines HTTP transport binding for server-to-server message relay | HTTP POST with JSON-RPC 2.0 payloads + mutual TLS. AMP and Matrix both use HTTP for federation. |
| TRNS-03 | All transport bindings require TLS 1.3 minimum | Straightforward normative requirement. `rustls` 0.23.x supports TLS 1.3 natively. |
| TRNS-04 | Protocol uses JSON-RPC 2.0 as the message framing format | Full JSON-RPC 2.0 spec researched: request (jsonrpc, id, method, params), response (jsonrpc, id, result/error), notification (no id), batch (array). |
| TRNS-05 | Protocol supports dual delivery modes: streaming and store-and-forward | XMPP XEP-0160 provides offline message handling precedent. Matrix uses full room state sync. Our approach: server queues messages for offline clients, delivers on reconnect. |
| CONT-01 | Messages are composed of typed content blocks with declared schemas | D-09 defines content blocks array. D-13 defines core vs namespaced extension types. D-15 adds block-level versioning. |
| CONT-02 | Protocol defines core block types: TextBlock, CodeBlock, ImageBlock, FileBlock | Four core types need full schema: text (markdown), code (language + content), image (url/data + mime + alt), file (url + mime + digest + size). |
| THRD-01 | Conversations are flat by default (single stream of messages) | D-11 defines thread-addressed messages. Default thread is the conversation root -- all messages in a thread form a flat, chronological stream. |
| THRD-02 | Any participant can create a sub-thread from any message | Matrix MSC3440 threading via relations is precedent. Our design: new thread_id + parent_message_id reference creates a sub-thread. |
| THRD-03 | Sub-threads inherit parent thread context | Spec must define what "context" means: participant list inherited, parent thread metadata accessible, messages in parent visible but not automatically forwarded. |
| THRD-04 | Agents can create sub-threads for side conversations | Same mechanism as THRD-02 -- agents are first-class participants. Sub-thread creation is a protocol operation, not a UI action. |
| MIXD-01 | Threads support any combination of human and agent participants as equals | D-08 exposes type in identity metadata. Thread participant list has no type-based restrictions. |
| MIXD-02 | Thread participant interface is uniform regardless of participant type | All participants use same address format, same message envelope, same content blocks. No type-specific message fields. |
| MIXD-03 | Presence semantics adapt per participant type | RFC 2778 defines presence model. Humans: online/offline/away. Agents: available/busy/processing. Presence is a protocol-level concept with type-adapted states. |
| MIXD-04 | @-mentions work across participant types | Mentions are a content-level feature: a `mention` inline annotation within text blocks referencing a participant address. Works uniformly. |

</phase_requirements>

## Project Constraints (from CLAUDE.md)

- **Protocol-first**: Spec must be complete and implementable before building the iOS app
- **Open standard**: Protocol spec and reference implementations are fully open source
- **Federation**: No single point of control -- any server can participate in the network
- **Transport**: WebSocket (client-to-server) + HTTP (server-to-server) + JSON-RPC 2.0 framing
- **Security**: Cryptographic identity and message signing are non-negotiable from day one
- **Ed25519**: Signing algorithm for messages and identity binding
- **JSON Schema 2020-12**: For message schema definitions (Phase 3 formalization, but informs Phase 1 design)
- **AsyncAPI v3.1.0**: For transport binding definitions (Phase 3 formalization, but informs Phase 1 design)
- **GSD Workflow Enforcement**: All work must go through GSD commands

## Standard Stack

This phase produces specification documents (markdown files), not code. The "stack" is the set of standards, formats, and tools used to write and validate the spec.

### Core Standards

| Standard | Version | Purpose | Why Standard |
|----------|---------|---------|--------------|
| RFC 8785 (JCS) | Informational RFC | Canonical JSON serialization for signing | Industry standard for deterministic JSON. Used by JCS Ed25519 Signature 2020, connect2id HMAC guide. `serde_json_canonicalizer` crate exists for Rust implementation. |
| JSON-RPC 2.0 | 2010-03-26 (stable) | Message framing format | Transport-agnostic RPC. Used by MCP, A2A, Polkadot/Substrate. Supports request/response, notifications, batch. |
| RFC 8615 | Standards Track | `.well-known` URI pattern | Used by Matrix, Mastodon, OAuth/OIDC for service discovery. A2A uses `.well-known/agent-card.json`. |
| RFC 9562 (UUIDv7) | Standards Track | Time-ordered message IDs | 48-bit Unix timestamp + random. Sortable, collision-resistant, no MAC address leakage. |
| Ed25519 (RFC 8032) | Standards Track | Message signing algorithm | Misuse-resistant, deterministic nonce, 64-byte signatures, constant-time. Used by AMP, SSH, JWTs. |
| RFC 2119 + RFC 8174 | BCP 14 | Normative keywords (MUST/SHALL/MAY) | Standard for protocol specs. Used sparingly per D-02. |
| RFC 2778 | Informational | Presence and instant messaging model | Defines presence tuples (status, address, markup). Foundation for MIXD-03 adapted presence. |

### Spec Writing Tools

| Tool | Purpose | When to Use |
|------|---------|-------------|
| Markdown | Spec prose | All five spec documents |
| JSON code blocks | Inline examples | Every message type definition (D-03) |
| Mermaid diagrams | Sequence diagrams, flow charts | Connection lifecycle, signing procedure, thread creation |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| RFC 8785 (JCS) | OLPC canonical JSON | JCS is an RFC, OLPC is informal. JCS has more implementations. |
| UUIDv7 | ULID | UUIDv7 is an RFC (9562), ULID is informal spec. Both time-ordered, but UUID has broader tooling. |
| Ed25519 | ECDSA P-256 | Ed25519 is misuse-resistant, faster, smaller signatures. P-256 has historical implementation bugs. |
| `.well-known` + custom path | WebFinger (RFC 7033) | WebFinger adds JRD format complexity. `.well-known/ainm/{name}` is simpler and sufficient. Decision D-05 locks this. |

## Architecture Patterns

### Spec Document Structure

Each of the five spec files follows this internal organization, modeled after MCP's specification style:

```
spec/
├── identity.md      # IDNT-01 through IDNT-05
├── security.md      # SECU-01 through SECU-05
├── transport.md     # TRNS-01 through TRNS-05
├── content.md       # CONT-01, CONT-02
└── threading.md     # THRD-01 through THRD-04, MIXD-01 through MIXD-04
```

### Pattern 1: Spec Document Internal Structure

**What:** Each spec document follows a consistent internal template.
**When to use:** Every spec file.

```markdown
# [Domain] Specification

## Overview
[2-3 paragraphs: what this spec defines, how it fits in the protocol]

## Terminology
[Domain-specific terms defined here]

## [Core Concept 1]
### Definition
[Normative prose with RFC keywords]

### Wire Format
```json
{
  // Concrete JSON example inline
}
```

### Behavior
[How implementations MUST/SHOULD handle this]

### Error Handling
[What happens when things go wrong]

## [Core Concept 2]
[Same structure...]

## Security Considerations
[Domain-specific security notes]

## Examples
[End-to-end worked examples showing the full flow]
```

### Pattern 2: Message Envelope Design (D-09)

**What:** The lean message envelope wrapping content blocks.
**When to use:** Every message sent through the protocol.

```json
{
  "id": "019580a1-7b3e-7000-8000-abcdef123456",
  "from": "alice@example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:00:00.000Z",
  "type": "message",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "user",
      "text": "Hello, can you help me with this code?",
      "fallback": "Hello, can you help me with this code?"
    }
  ],
  "signature": "<base64url-encoded-ed25519-signature>"
}
```

**Design rationale:**
- `id`: UUIDv7 -- time-ordered, enables deduplication (D-12)
- `from`: sender's identity address (IDNT-01)
- `thread_id`: thread-addressed delivery (D-11)
- `timestamp`: RFC 3339 with milliseconds
- `type`: message type discriminator (message, thread.create, presence, etc.)
- `content`: array of typed content blocks (D-09)
- `signature`: Ed25519 signature over JCS-canonicalized envelope minus the signature field (SECU-01, SECU-02)

### Pattern 3: Signing Procedure

**What:** Step-by-step signing of messages using RFC 8785 + Ed25519.
**When to use:** Every message sent.

```
1. Construct the message object WITHOUT the `signature` field
2. Apply RFC 8785 JSON Canonicalization:
   a. Sort all object keys lexicographically (UTF-16 code unit comparison)
   b. Remove all whitespace between tokens
   c. Serialize numbers per ECMAScript rules
   d. Serialize strings with proper escaping
3. Encode the canonical JSON as UTF-8 bytes
4. Sign the UTF-8 bytes with the sender's Ed25519 private key
5. Base64url-encode the 64-byte signature (no padding)
6. Insert the signature into the message object as the `signature` field
```

**Verification reverses this:**
```
1. Extract and remove `signature` from the received message
2. Resolve the sender's identity to get their current public key (or historical key for old messages per D-07)
3. Apply RFC 8785 canonicalization to the remaining message
4. Encode as UTF-8 bytes
5. Verify the Ed25519 signature against the public key
```

### Pattern 4: Connection Lifecycle (modeled on MCP)

**What:** WebSocket connection handshake, operation, and shutdown.
**When to use:** Client-to-server connections.

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
  |--- ainm.message.send -------->|
  |<-- ainm.message.deliver ------|
  |                               |
  |--- close frame -------------->|  WebSocket close
  |<-- close frame ---------------|
```

### Pattern 5: Identity Resolution Flow

**What:** How a participant resolves another participant's identity.
**When to use:** Before sending a first message to a new participant.

```
1. Parse address: "bob@agents.example.com"
   - name = "bob"
   - provider = "agents.example.com"

2. HTTP GET https://agents.example.com/.well-known/ainm/bob
   - TLS 1.3 required
   - Response: application/json

3. Response:
   {
     "id": "bob@agents.example.com",
     "public_key": "<base64url-encoded-ed25519-public-key>",
     "server": "wss://agents.example.com/ainm/v1",
     "type": "agent",
     "agent_card_url": "https://agents.example.com/.well-known/agent-card.json",
     "key_history": [
       {
         "public_key": "<previous-key>",
         "valid_from": "2026-01-01T00:00:00Z",
         "valid_until": "2026-03-01T00:00:00Z",
         "revocation_reason": "scheduled_rotation"
       }
     ]
   }

4. Cache the identity data (with TTL from Cache-Control headers)
5. Use `server` field to establish WebSocket connection
6. Use `public_key` to verify received messages
```

### Anti-Patterns to Avoid

- **Overusing RFC keywords:** D-02 explicitly says NOT to use MUST/SHALL throughout. Use them only for normative interoperability requirements. Example: "Messages are signed with Ed25519" (prose), but "Implementations MUST reject messages with invalid signatures" (normative).
- **Spec without examples:** D-03 requires inline JSON examples next to every definition. A message type defined without a concrete example is incomplete.
- **Mixing concerns across spec files:** Each file MUST stand alone (D-01). Cross-references are fine, but a developer reading `identity.md` should not need to also read `security.md` to understand identity resolution.
- **Underspecifying error cases:** Protocol specs commonly fail by not defining what happens on error. Every operation in the spec must define error responses with JSON-RPC error codes.
- **Ambiguous normative language:** "The server should probably validate..." is useless. Either it MUST validate (interop requirement) or it MAY validate (implementation choice). No middle ground.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON canonicalization | Custom key-sorting serializer | RFC 8785 algorithm (spec reference) | UTF-16 code unit sorting has edge cases (surrogate pairs, emoji). The RFC handles them all. Implementers use `serde_json_canonicalizer` (Rust), `json-canonicalize` (npm), `rfc8785` (Python). |
| Message ID generation | Custom timestamp + random | UUIDv7 (RFC 9562) | Time-ordered, 128-bit, collision-resistant. Well-specified bit layout. Libraries exist for every language. |
| Signing algorithm | Custom signature scheme | Ed25519 (RFC 8032) | Misuse-resistant by design. Deterministic nonce prevents random number generator failures. Constant-time execution prevents timing attacks. |
| RPC framing | Custom message envelope | JSON-RPC 2.0 specification | Battle-tested, simple, supports requests/responses/notifications/batches. Error codes are standardized. |
| Well-known URIs | Custom discovery paths | RFC 8615 pattern | Standardized, understood by CDNs, proxies, and security tools. Used by Matrix, Mastodon, OIDC. |
| Presence model | Custom status system | RFC 2778 presence model (adapted) | Defines standard presence tuples. We adapt the status values for agents but use the conceptual model. |

**Key insight:** This phase outputs a specification, not code. The "don't hand-roll" principle means: don't invent custom wire formats, algorithms, or patterns when established RFCs exist. Reference the RFCs and specify how they're used in this protocol.

## Common Pitfalls

### Pitfall 1: Spec Ambiguity Leading to Incompatible Implementations

**What goes wrong:** Natural language specifications contain ambiguities that lead to different implementations making different assumptions, resulting in interoperability failures.
**Why it happens:** English is inherently ambiguous. APNIC research (2022) found even simple protocols like ICMP have 5+ ambiguities in their RFCs.
**How to avoid:** (1) Use RFC keywords precisely and sparingly. (2) Provide concrete JSON examples for every message type. (3) Define error cases explicitly -- what happens when validation fails, when a field is missing, when a key is revoked. (4) Include a "Test Vectors" section with input/output pairs for signing and canonicalization.
**Warning signs:** A definition that reads well but you can't write a conformance test for it.

### Pitfall 2: Canonicalization Edge Cases

**What goes wrong:** Different implementations produce different canonical JSON for the same input, causing signature verification to fail across implementations.
**Why it happens:** RFC 8785 has subtle requirements: UTF-16 code unit sorting (not UTF-8), ECMAScript number serialization (not IEEE 754 string representation), specific escape sequences for control characters. Most developers will use a library, but the spec must be precise enough for someone writing a library.
**How to avoid:** Include test vectors with edge cases: objects with Unicode keys (emoji, CJK, surrogates), numbers at IEEE 754 precision boundaries (0.1, 1e+30, -0.0), strings with control characters. Reference the RFC 8785 test suite directly.
**Warning signs:** The spec says "canonical JSON" without specifying exactly which canonicalization scheme and version.

### Pitfall 3: Key Rotation Without Graceful Transition

**What goes wrong:** When a participant rotates their key, messages signed with the old key become unverifiable. In-flight messages signed with the old key are rejected by recipients who already fetched the new key.
**Why it happens:** Key rotation is inherently a distributed systems problem -- there's no atomic "everyone switch now" moment.
**How to avoid:** D-07's key history array with validity periods is the right approach. The spec must define: (1) overlap period where both old and new keys are valid, (2) how recipients determine which key to use for verification (try current key first, fall back to history), (3) maximum history retention requirements.
**Warning signs:** The spec defines key rotation but not what happens to messages already in transit.

### Pitfall 4: Thread Semantics Complexity Explosion

**What goes wrong:** Threading spec becomes so complex that implementers skip features or implement them inconsistently. Matrix's MSC3440 explicitly punted on nested threads due to complexity.
**Why it happens:** Sub-threading, context inheritance, and participant management interact in complex ways. Each feature multiplies edge cases.
**How to avoid:** Keep threading deliberately simple for v1: (1) Flat conversations are the default (THRD-01). (2) Sub-threads are a single level of nesting only -- no threads-within-threads. (3) Context inheritance means "sub-thread inherits parent participant list" not "all parent messages are visible." (4) Defer deep nesting to v2.
**Warning signs:** The threading spec is longer than the security spec.

### Pitfall 5: Store-and-Forward Without Deduplication

**What goes wrong:** At-least-once delivery (D-12) means the same message may be delivered multiple times. Without proper deduplication, users see duplicate messages and agents process the same request twice.
**Why it happens:** Network interruptions during delivery ACK cause the server to retry. Client reconnections cause queued messages to be re-sent.
**How to avoid:** UUIDv7 message IDs (unique, time-ordered) are the deduplication key. The spec must define: (1) clients MUST track received message IDs and silently discard duplicates, (2) servers MUST track delivered message IDs per session, (3) deduplication window (e.g., last 1000 IDs or last 24 hours).
**Warning signs:** The spec mentions "at-least-once" without defining the deduplication mechanism.

### Pitfall 6: Presence Semantics That Don't Fit Agents

**What goes wrong:** Copying human presence states (online/offline/away/do-not-disturb) for agents creates meaningless states. An agent is never "away for lunch."
**Why it happens:** Reusing human-centric presence models without adaptation.
**How to avoid:** MIXD-03 requires adapted presence. Define: humans get `online`/`offline`/`away`/`dnd`. Agents get `available`/`busy`/`processing`/`offline`. The protocol delivers presence as a typed status with the participant type determining valid values.
**Warning signs:** The spec defines one set of presence states for all participant types.

## Code Examples

These are spec-level examples (JSON structures), not implementation code.

### Identity Resolution Response

```json
// GET https://agents.example.com/.well-known/ainm/weather-bot
// Content-Type: application/json
{
  "id": "weather-bot@agents.example.com",
  "public_key": "MCowBQYDK2VwAyEA6bR-Y57oVDQrVixqH_Zt6eGbamVR1N3gILEf0nG1I3U",
  "server": "wss://agents.example.com/ainm/v1",
  "type": "agent",
  "agent_card_url": "https://agents.example.com/.well-known/agent-card.json",
  "key_history": []
}
```

### Signed Message Envelope

```json
{
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
      "text": "What's the weather in Madrid?",
      "fallback": "What's the weather in Madrid?"
    }
  ],
  "signature": "dGhpcyBpcyBhIGJhc2U2NHVybCBlbmNvZGVkIGVkMjU1MTkgc2lnbmF0dXJl..."
}
```

### Agent Response with Multiple Content Blocks

```json
{
  "id": "019580a2-9c4f-7000-8000-fedcba987654",
  "from": "weather-bot@agents.example.com",
  "thread_id": "019580a0-1234-7000-8000-000000000001",
  "timestamp": "2026-03-28T12:34:58.123Z",
  "type": "message",
  "content": [
    {
      "type": "text",
      "version": "1.0",
      "source": "agent",
      "text": "Here's the current weather in Madrid:",
      "fallback": "Here's the current weather in Madrid:"
    },
    {
      "type": "code",
      "version": "1.0",
      "source": "agent",
      "language": "json",
      "code": "{\n  \"city\": \"Madrid\",\n  \"temp_c\": 22,\n  \"condition\": \"Sunny\"\n}",
      "fallback": "Madrid: 22C, Sunny"
    }
  ],
  "signature": "YW5vdGhlciBiYXNlNjR1cmwgZW5jb2RlZCBzaWduYXR1cmU..."
}
```

### JSON-RPC 2.0 Connection Lifecycle

```json
// 1. Client sends initialize request
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "ainm.initialize",
  "params": {
    "protocolVersion": "0.1.0",
    "identity": "alice@messaging.example.com",
    "capabilities": {
      "streaming": true,
      "subThreads": true,
      "contentTypes": ["text", "code", "image", "file"]
    }
  }
}

// 2. Server responds with capabilities
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
    }
  }
}

// 3. Client confirms initialization
{
  "jsonrpc": "2.0",
  "method": "ainm.initialized"
}
```

### Thread Creation with Sub-Thread

```json
// Creating a sub-thread from a message
{
  "jsonrpc": "2.0",
  "id": 42,
  "method": "ainm.thread.create",
  "params": {
    "parent_thread_id": "019580a0-1234-7000-8000-000000000001",
    "parent_message_id": "019580a1-7b3e-7000-8000-abcdef123456",
    "thread_id": "019580a3-5678-7000-8000-111111111111",
    "title": "Debugging the weather API call",
    "participants": ["alice@messaging.example.com", "weather-bot@agents.example.com"]
  }
}
```

### Presence Update

```json
// Agent presence notification
{
  "jsonrpc": "2.0",
  "method": "ainm.presence.update",
  "params": {
    "identity": "weather-bot@agents.example.com",
    "status": "processing",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "detail": "Fetching weather data from API"
  }
}
```

### Content Block with Extension Type

```json
{
  "type": "com.example.weather-card",
  "version": "1.0",
  "source": "agent",
  "data": {
    "city": "Madrid",
    "temperature": 22,
    "unit": "celsius",
    "condition": "sunny",
    "icon": "sun"
  },
  "fallback": "Madrid: 22C, Sunny"
}
```

### @-Mention in Text Block

```json
{
  "type": "text",
  "version": "1.0",
  "source": "user",
  "text": "Hey @weather-bot@agents.example.com, what's the forecast?",
  "mentions": [
    {
      "address": "weather-bot@agents.example.com",
      "offset": 4,
      "length": 37
    }
  ],
  "fallback": "Hey @weather-bot, what's the forecast?"
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Custom JSON serialization for signing | RFC 8785 JCS | RFC published June 2020 | Standard canonical form. Libraries available in Rust, JS, Python, Java, Elixir. |
| UUIDv4 (random) for message IDs | UUIDv7 (time-ordered) | RFC 9562 published May 2024 | Sortable IDs improve database performance and enable natural ordering. |
| WebFinger (RFC 7033) for identity | `.well-known` + custom path | Ongoing trend (A2A, Matrix) | Simpler than WebFinger's JRD format. Custom path avoids WebFinger's `resource` query complexity. |
| RSA/ECDSA for message signing | Ed25519 | Growing adoption 2020-2026 | Smaller signatures (64 bytes vs 256+), faster, misuse-resistant. AMP, SSH, JOSE all support it. |
| REST APIs for messaging | JSON-RPC 2.0 over WebSocket | MCP (2024), A2A (2025) | Bidirectional, supports notifications without polling, built-in request/response correlation. |
| Human-only presence states | Type-adapted presence | Novel (this protocol) | No existing protocol distinguishes human vs agent presence. This is a differentiator. |

**Deprecated/outdated:**
- **WebFinger for agent identity:** A2A and AMP both moved away from WebFinger toward simpler `.well-known` patterns.
- **JWK for identity response:** D-06 specifies flat JSON, not JWK envelope. JWK adds unnecessary complexity for a single public key.
- **Custom signing formats:** AMP's pipe-delimited signing format (`from|to|subject|...`) is fragile -- adding a new field breaks backward compatibility. JCS canonicalization of the entire message is more robust.

## Open Questions

1. **Registration Flow Details**
   - What we know: Identity resolution is fully specified (D-05, D-06). Agents register with providers and submit Ed25519 public keys.
   - What's unclear: The exact registration API endpoint and request format. Is it a JSON-RPC method? A REST endpoint? Does the provider verify domain ownership?
   - Recommendation: Define a minimal registration endpoint (`POST /.well-known/ainm/register`) with a clear request/response format. Domain verification is out of scope for v1 -- providers are trusted to manage their own namespace.

2. **Server-to-Server Relay Authentication**
   - What we know: TRNS-02 requires HTTP transport for federation. Mutual TLS is specified in CLAUDE.md.
   - What's unclear: How servers discover each other's TLS certificates. Is there a server identity resolution mechanism analogous to participant identity?
   - Recommendation: Servers resolve each other via the same `.well-known` pattern but at the server level. A `/.well-known/ainm/server` endpoint returns the server's public key and TLS certificate fingerprint. Mutual TLS verification uses these certificates.

3. **Content Block Size Limits**
   - What we know: D-09 specifies content blocks array. Phase 2 includes CONT-05 (no arbitrary payload size limits at protocol level).
   - What's unclear: Should Phase 1 define any practical limits for core blocks (text length, image size, file size)?
   - Recommendation: Protocol-level limits should be capabilities exchanged during handshake (maxMessageSize, maxContentBlocks), not hard-coded in the spec. Individual servers can enforce their own limits and communicate them during initialization.

4. **Thread Participant Management**
   - What we know: D-11 defines thread-addressed messages. THRD-03 says sub-threads inherit parent context.
   - What's unclear: How are participants added to or removed from threads? Is there an explicit join/leave mechanism, or is it implicit (first message = join)?
   - Recommendation: Define explicit `ainm.thread.join` and `ainm.thread.leave` methods. Thread creators specify initial participants. Any current participant can invite others. This keeps the participant list deterministic rather than inferred.

5. **Offline Message Queue Semantics**
   - What we know: D-12 specifies at-least-once delivery. TRNS-05 requires store-and-forward mode.
   - What's unclear: How long do servers hold messages for offline participants? Is there a maximum queue depth or time-to-live?
   - Recommendation: Define a SHOULD-level recommendation (e.g., servers SHOULD retain messages for at least 7 days) but allow servers to communicate their retention policy during capability exchange. This avoids hard-coding limits that different deployments may need to vary.

## Sources

### Primary (HIGH confidence)
- [RFC 8785 - JSON Canonicalization Scheme](https://www.rfc-editor.org/rfc/rfc8785) - Complete canonicalization algorithm, sorting rules, number serialization
- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification) - Full request/response/notification/batch format
- [RFC 8615 - Well-Known URIs](https://datatracker.ietf.org/doc/html/rfc8615) - `.well-known` path prefix standard
- [RFC 9562 - UUIDv7](https://www.rfc-editor.org/rfc/rfc9562.html) - Time-ordered UUID specification
- [RFC 8032 - Ed25519](https://ed25519.cr.yp.to/) - EdDSA signature algorithm
- [RFC 2778 - Presence Model](https://datatracker.ietf.org/doc/html/rfc2778) - Presence and instant messaging abstract model
- [MCP Specification (2025-11-25)](https://modelcontextprotocol.io/specification/2025-11-25) - Connection lifecycle, JSON-RPC usage patterns, capability negotiation
- [MCP Base Protocol / Lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle) - Initialize/initialized/shutdown pattern
- [A2A Protocol Specification](https://a2a-protocol.org/latest/specification/) - Agent Card at `.well-known/agent-card.json`, JSON-RPC usage

### Secondary (MEDIUM confidence)
- [AMP Protocol (GitHub)](https://github.com/agentmessaging/protocol) - 11-doc spec structure, Ed25519 signing format, email-like addressing, delivery mechanisms
- [Matrix MSC3440 - Threading via Relations](https://github.com/matrix-org/matrix-spec-proposals/blob/main/proposals/3440-threading-via-relations.md) - Thread implementation via event relations, flat threads, no nested threads in v1
- [serde_json_canonicalizer crate](https://crates.io/crates/serde_json_canonicalizer) - RFC 8785 Rust implementation
- [JCS Ed25519 Signature 2020](https://identity.foundation/JcsEd25519Signature2020/) - JCS + SHA-256 + Ed25519 signing procedure
- [RFC 4101 - Writing Protocol Models](https://datatracker.ietf.org/doc/html/rfc4101) - Best practices for protocol spec writing

### Tertiary (LOW confidence)
- [APNIC Blog - Why Protocol Specifications Don't Work](https://blog.apnic.net/2022/11/10/why-protocol-specifications-dont-always-work/) - Spec ambiguity analysis (informal but insightful)
- [XMPP XEP-0160 - Offline Messages](https://xmpp.org/extensions/xep-0160.html) - Store-and-forward best practices (informative for TRNS-05)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All recommendations are published RFCs or widely-adopted specifications with stable versions
- Architecture: HIGH - Spec document structure follows proven patterns (MCP, AMP, IETF RFCs) and all decisions are locked in CONTEXT.md
- Pitfalls: HIGH - Drawn from concrete failure modes in Matrix (threading), AMP (signing format), and academic analysis of protocol spec ambiguity
- Code examples: MEDIUM - JSON structures are illustrative; exact field names and types may evolve during spec writing
- Open questions: MEDIUM - Questions are real gaps that the planner must address; recommendations are reasonable but need user confirmation

**Research date:** 2026-03-28
**Valid until:** 2026-04-28 (30 days -- stable domain, all based on published RFCs)
