# Phase 3: Specification Publication & Working Demo - Research

**Researched:** 2026-03-28
**Domain:** Protocol formalization (JSON Schema 2020-12, AsyncAPI 3.1), spec publication (Docusaurus 3.x), Rust CLI demo (Axum WebSocket, Ed25519 signing, terminal REPL)
**Confidence:** HIGH

## Summary

Phase 3 has three distinct workstreams: (1) extracting formal JSON Schema 2020-12 definitions from the 11 existing spec files (~213 inline JSON examples), (2) building a Docusaurus 3.x site to publish the spec with embedded AsyncAPI explorer, and (3) creating a single-binary Rust CLI demo that proves the protocol end-to-end.

The schema extraction workstream is the largest -- every message type, content block, interactive block, streaming event, memory operation, and agent card structure needs a formal schema derived from the prose constraints and inline examples already written. The Docusaurus site is straightforward with one known risk: AsyncAPI React component integration requires the `docusaurus-asyncapi` preset (v1.0.2) which pins `@asyncapi/react-component` at v2.6.3 to avoid webpack Node.js polyfill issues. The Rust demo is a contained single-binary that starts an Axum server, connects an in-process agent via tokio-tungstenite, and opens a rustyline REPL for human input -- no external dependencies, works offline.

**Primary recommendation:** Execute in three waves -- schemas first (they block AsyncAPI and demo validation), then Docusaurus site and Rust demo in parallel (independent workstreams), then integration testing.

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** CLI terminal demo -- Rust server + terminal REPL. Human types messages, agent responds with streaming text and rich content blocks rendered as formatted terminal output. No web UI.
- **D-02:** Single binary architecture -- one `cargo run --bin ainm-demo` starts the Axum WebSocket server, registers identities, connects the agent as an in-process WebSocket client, and opens a terminal REPL for human input. External devs run one command.
- **D-03:** Mock/scripted agent -- hardcoded responses that showcase different protocol features (streaming, Form, Button, Approval, CodeBlock, tool call). No API key needed, works offline, reproducible.
- **D-04:** Core flow + highlights coverage -- identity registration/resolution, thread creation, message signing/verification, streaming (start/delta/end with multi-block), 2-3 interactive blocks (Form, Button, Approval), Agent Card serve/resolve, and one tool visibility demo. NOT exercised: federation, memory tiers, MCP/A2A bridges, sub-threading.
- **D-05:** Per-domain JSON Schema bundles mirroring spec/ structure: `schemas/identity.schema.json`, `schemas/security.schema.json`, `schemas/content.schema.json`, `schemas/interactive.schema.json`, `schemas/transport.schema.json`, `schemas/threading.schema.json`, `schemas/streaming.schema.json`, `schemas/agent-card.schema.json`, `schemas/memory.schema.json`, `schemas/tool-visibility.schema.json`, `schemas/compatibility.schema.json`, plus `schemas/common.schema.json` for shared definitions.
- **D-06:** Schemas derived from inline JSON examples in the existing 11 spec files. The prose defines constraints (MUST/SHALL), the examples show structure. Schema writing is a formalization pass.
- **D-07:** Single AsyncAPI document (`asyncapi.yaml`) defining all channels (client-server WebSocket, server-server HTTP). References per-domain JSON Schema files via $ref.
- **D-08:** Initial protocol version: 0.1.0. SemVer with 0.x freedom.
- **D-09:** Single protocol version for the whole spec -- all 11 spec files, all schemas, and asyncapi.yaml share v0.1.0.
- **D-10:** SemVer with 0.x freedom. Breaking changes documented in CHANGELOG.md.
- **D-11:** Docusaurus 3.x spec-centric site with reference sidebar. Structure: landing page, /spec/, /schemas/, /asyncapi/, /demo/, /changelog.
- **D-12:** Hosted on GitHub Pages with GitHub Actions CI/CD on push.
- **D-13:** AsyncAPI explorer embedded via @asyncapi/react-component in a Docusaurus MDX page.

### Claude's Discretion
- Terminal output formatting for rich content blocks (how Forms, Buttons, Approvals render in terminal)
- Demo agent's specific scripted conversation scenarios
- Schema $ref linking strategy and common.schema.json content decomposition
- Docusaurus theme/styling choices
- GitHub Actions workflow configuration details
- CHANGELOG.md format and structure
- Landing page copy and quick-start guide content

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.

</user_constraints>

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SPEC-01 | Protocol spec published as open document (Markdown + JSON Schema + AsyncAPI) | Docusaurus 3.9.2 site hosts rendered spec/, schemas/ directory with 12 JSON Schema 2020-12 files, asyncapi.yaml with $ref to schemas. GitHub Pages deployment via Actions. |
| SPEC-02 | All message types have formal JSON Schema definitions | 213 inline JSON examples across 11 spec files provide ground truth. Per-domain schema bundles (D-05) with common.schema.json for shared definitions. jsonschema crate v0.45.0 validates at runtime. |
| SPEC-03 | Transport bindings have formal AsyncAPI v3.1 definitions | Single asyncapi.yaml defines WebSocket client-server and HTTP server-server channels. ~20 JSON-RPC methods mapped as operations with request-reply patterns. @asyncapi/cli v6.0.0 validates in CI. |
| SPEC-04 | Spec versioned with clear backwards compatibility rules | Version 0.1.0 (D-08/D-09). SemVer rules defined in D-10. CHANGELOG.md tracks changes. Docusaurus versioning support for future versions. |
| SPEC-05 | Working demo proves protocol end-to-end (1 human + 1 agent, streaming, rich content) | Rust single-binary demo (D-01/D-02) with Axum server, tokio-tungstenite agent client, rustyline REPL. Scripted agent (D-03) showcases streaming, interactive blocks, tool visibility. All messages Ed25519 signed. |

</phase_requirements>

## Project Constraints (from CLAUDE.md)

- Protocol-first: spec must be complete before building iOS app (Phase 3 is spec publication, aligns)
- Open standard: protocol spec and reference implementations fully open source
- Federation: not in scope for Phase 3 demo (confirmed by D-04)
- Transport: WebSocket (primary) + HTTP (fallback/federation) -- AsyncAPI must define both bindings
- Security: cryptographic identity and message signing non-negotiable (demo must implement Ed25519)
- Technology stack locked in CLAUDE.md: Rust 1.85+/Axum 0.8.x for server, Docusaurus 3.x for docs, JSON Schema 2020-12, AsyncAPI v3.1.0, Ed25519 via ring crate

## Standard Stack

### Core -- Schema & Spec Tooling

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| JSON Schema 2020-12 | Draft 2020-12 | Message schema definitions | Industry standard. Supports $ref, $defs, $id for modular multi-file schemas. Verified: all target validators (jsonschema Rust crate, AJV) support 2020-12. |
| AsyncAPI | v3.1.0 | Transport binding definitions | Purpose-built for event-driven APIs. WebSocket + HTTP bindings. Request-reply support. v3.1.0 is current stable. |
| @asyncapi/cli | 6.0.0 | AsyncAPI validation in CI | Validates asyncapi.yaml structure and $ref integrity. Latest stable. |

### Core -- Documentation Site

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| @docusaurus/core | 3.9.2 | Spec website framework | Current stable. Versioning, MDX, React components. |
| @docusaurus/preset-classic | 3.9.2 | Standard Docusaurus preset | Docs + blog + pages plugins bundled. |
| docusaurus-asyncapi | 1.0.2 | AsyncAPI Docusaurus integration | Wraps @asyncapi/react-component v2.6.3 as Docusaurus preset. Solves the webpack Node.js polyfill issue (STATE.md concern). |
| react | 18.x or 19.x | UI framework (Docusaurus dep) | Required by Docusaurus 3.9.2 |

### Core -- Rust Demo

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum | 0.8.8 | HTTP/WebSocket server | Tokio team. Built-in WebSocket upgrade. Per CLAUDE.md. |
| tokio | 1.50.0 | Async runtime | The Rust async runtime. |
| tokio-tungstenite | 0.29.0 | WebSocket client (agent side) | Most-used Rust WS library. Agent connects to local server. |
| serde + serde_json | 1.x | JSON serialization | Rust standard for JSON. |
| serde_json_canonicalizer | 0.3.2 | RFC 8785 JCS | Canonical JSON for Ed25519 signing input. |
| ring | 0.17.14 | Ed25519 signing/verification | Audited crypto. Ed25519KeyPair for signing, UnparsedPublicKey for verification. |
| uuid | 1.23.0 | UUIDv7 message/stream IDs | Time-ordered unique identifiers per spec. |
| chrono | 0.4.44 | RFC 3339 timestamps | Timestamp formatting on all protocol messages. |
| rustyline | 17.0.2 | Terminal REPL | Readline implementation. History, completion. More mature than reedline for simple REPL. |
| colored | 3.1.1 | Terminal colors | Simplest colored output. For signature verification status, block type headers. |
| base64 | 0.22.1 | Base64url encoding | Ed25519 signatures encoded as base64url without padding. |
| tracing | 0.1.44 | Structured logging | Debug output, connection lifecycle visibility. |
| jsonschema | 0.45.0 | Runtime schema validation | Optional: validate messages against schemas in demo for correctness proof. |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| crossterm | 0.29.0 | Terminal manipulation | If rich block rendering needs cursor control, box drawing, or async input handling beyond rustyline. |
| termimad | (latest) | Markdown terminal rendering | If TextBlock markdown content should render with formatting in terminal. |
| rand | 0.10.0 | Random bytes | Nonce generation for ainm.initialize handshake. |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| rustyline | reedline 0.46.0 | reedline is nushell's engine, more features but heavier. rustyline is simpler for a demo REPL. |
| colored | crossterm styling | crossterm is more powerful but overkill for basic coloring. Use crossterm only if box-drawing needed. |
| docusaurus-asyncapi preset | Manual @asyncapi/react-component v3.1.0 | Direct v3.1.0 import has webpack Node polyfill issues (GitHub issue #734). The preset pins v2.6.3 which works. |
| jsonschema crate | No runtime validation | Could skip runtime validation in demo and rely on schema files + CI tests only. But runtime validation proves schemas work. |

### Installation

**Spec tooling (website directory):**
```bash
npx create-docusaurus@3.9.2 website classic
cd website
npm install docusaurus-asyncapi@1.0.2
npm install -D @asyncapi/cli@6.0.0
```

**Rust demo (demo directory):**
```bash
cargo init --name ainm-demo demo
# Add to demo/Cargo.toml:
# axum = { version = "0.8", features = ["ws"] }
# tokio = { version = "1.50", features = ["full"] }
# tokio-tungstenite = "0.29"
# serde = { version = "1", features = ["derive"] }
# serde_json = "1"
# serde_json_canonicalizer = "0.3"
# ring = "0.17"
# uuid = { version = "1", features = ["v7"] }
# chrono = { version = "0.4", features = ["serde"] }
# rustyline = "17"
# colored = "3"
# base64 = "0.22"
# tracing = "0.1"
# tracing-subscriber = "0.3"
```

## Architecture Patterns

### Recommended Project Structure

```
ai-native-messaging/
├── spec/                        # Existing 11 spec markdown files (unchanged)
│   ├── identity.md
│   ├── security.md
│   ├── transport.md
│   ├── content.md
│   ├── interactive.md
│   ├── threading.md
│   ├── streaming.md
│   ├── agent-card.md
│   ├── memory.md
│   ├── tool-visibility.md
│   └── compatibility.md
├── schemas/                     # NEW: JSON Schema 2020-12 files
│   ├── common.schema.json       # Shared definitions ($defs)
│   ├── identity.schema.json
│   ├── security.schema.json
│   ├── transport.schema.json
│   ├── content.schema.json
│   ├── interactive.schema.json
│   ├── threading.schema.json
│   ├── streaming.schema.json
│   ├── agent-card.schema.json
│   ├── memory.schema.json
│   ├── tool-visibility.schema.json
│   └── compatibility.schema.json
├── asyncapi.yaml                # NEW: AsyncAPI 3.1.0 transport bindings
├── CHANGELOG.md                 # NEW: Version history
├── demo/                        # NEW: Rust single-binary demo
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs              # Entry point: start server, agent, REPL
│       ├── server.rs            # Axum WebSocket server
│       ├── agent.rs             # Mock agent (scripted responses)
│       ├── identity.rs          # Identity registration/resolution
│       ├── signing.rs           # Ed25519 signing + RFC 8785 canonical JSON
│       ├── protocol.rs          # JSON-RPC message types, envelopes
│       ├── streaming.rs         # Stream lifecycle (start/delta/end)
│       ├── repl.rs              # Terminal REPL (rustyline)
│       └── render.rs            # Terminal rendering for content blocks
├── website/                     # NEW: Docusaurus spec site
│   ├── docusaurus.config.js
│   ├── sidebars.js
│   ├── docs/
│   │   ├── intro.md             # Landing page / quick-start
│   │   ├── spec/                # Rendered spec (imports from ../spec/)
│   │   ├── schemas/             # Schema reference pages
│   │   └── demo.md              # How to run the demo
│   ├── src/
│   │   └── pages/
│   │       └── asyncapi.mdx     # AsyncAPI explorer page
│   └── static/
│       └── schemas/             # Copy of schemas/ for download
└── .github/
    └── workflows/
        └── deploy-docs.yml      # GitHub Actions: build + deploy to Pages
```

### Pattern 1: JSON Schema $ref Linking (common.schema.json)

**What:** Shared definitions extracted into `common.schema.json`, referenced via `$ref` from domain schemas.
**When to use:** Any type used across multiple schema files (message envelope, content block base, address format, UUIDv7 format).

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://ainm.dev/schemas/common.schema.json",
  "$defs": {
    "AINMAddress": {
      "type": "string",
      "pattern": "^[a-zA-Z0-9][a-zA-Z0-9._-]{0,63}@[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?(\\.[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?)+$",
      "description": "AINM address in name@provider format"
    },
    "UUIDv7": {
      "type": "string",
      "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$",
      "description": "UUIDv7 identifier (RFC 9562)"
    },
    "RFC3339Timestamp": {
      "type": "string",
      "format": "date-time",
      "description": "RFC 3339 timestamp with millisecond precision and UTC timezone"
    },
    "Ed25519Signature": {
      "type": "string",
      "description": "Base64url-encoded Ed25519 signature (64 bytes, no padding)"
    },
    "ContentBlockBase": {
      "type": "object",
      "required": ["type", "version", "source", "fallback"],
      "properties": {
        "type": { "type": "string" },
        "version": { "type": "string", "pattern": "^\\d+\\.\\d+$" },
        "source": { "type": "string", "enum": ["user", "agent", "system"] },
        "fallback": { "type": "string" }
      }
    },
    "MessageEnvelope": {
      "type": "object",
      "required": ["id", "from", "thread_id", "timestamp", "type", "content", "signature"],
      "properties": {
        "id": { "$ref": "#/$defs/UUIDv7" },
        "from": { "$ref": "#/$defs/AINMAddress" },
        "thread_id": { "$ref": "#/$defs/UUIDv7" },
        "timestamp": { "$ref": "#/$defs/RFC3339Timestamp" },
        "type": { "type": "string" },
        "content": { "type": "array", "items": { "$ref": "#/$defs/ContentBlockBase" } },
        "signature": { "$ref": "#/$defs/Ed25519Signature" }
      }
    }
  }
}
```

Domain schemas reference common definitions:
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://ainm.dev/schemas/streaming.schema.json",
  "$defs": {
    "StreamStart": {
      "type": "object",
      "required": ["stream_id", "message_id", "from", "thread_id", "timestamp", "blocks"],
      "properties": {
        "stream_id": { "$ref": "common.schema.json#/$defs/UUIDv7" },
        "message_id": { "$ref": "common.schema.json#/$defs/UUIDv7" },
        "from": { "$ref": "common.schema.json#/$defs/AINMAddress" },
        "thread_id": { "$ref": "common.schema.json#/$defs/UUIDv7" },
        "timestamp": { "$ref": "common.schema.json#/$defs/RFC3339Timestamp" },
        "blocks": {
          "type": "array",
          "items": { "$ref": "#/$defs/BlockLayoutEntry" }
        }
      }
    }
  }
}
```

### Pattern 2: AsyncAPI 3.1.0 Structure for AINM

**What:** Single asyncapi.yaml mapping all JSON-RPC methods as channels/operations with $ref to schemas.
**When to use:** Defining the complete transport binding document.

```yaml
asyncapi: 3.1.0
info:
  title: AI-Native Messaging Protocol
  version: 0.1.0
  description: |
    Transport bindings for the AINM protocol.
    Client-to-server via WebSocket, server-to-server via HTTP.

servers:
  client-server:
    host: "{server}"
    protocol: wss
    description: WebSocket endpoint for client connections
  federation:
    host: "{server}"
    protocol: https
    description: HTTP endpoint for server-to-server relay

channels:
  initialize:
    address: /
    messages:
      initializeRequest:
        $ref: "#/components/messages/InitializeRequest"
      initializeResponse:
        $ref: "#/components/messages/InitializeResponse"
  messageDelivery:
    address: /
    messages:
      messageDeliver:
        $ref: "#/components/messages/MessageDeliver"
  streaming:
    address: /
    messages:
      streamStart:
        $ref: "#/components/messages/StreamStart"
      streamDelta:
        $ref: "#/components/messages/StreamDelta"
      streamEnd:
        $ref: "#/components/messages/StreamEnd"

operations:
  sendInitialize:
    action: send
    channel:
      $ref: "#/channels/initialize"
    messages:
      - $ref: "#/channels/initialize/messages/initializeRequest"
    reply:
      channel:
        $ref: "#/channels/initialize"
      messages:
        - $ref: "#/channels/initialize/messages/initializeResponse"

components:
  messages:
    InitializeRequest:
      name: ainm.initialize
      title: Initialize Session
      contentType: application/json
      payload:
        $ref: "schemas/transport.schema.json#/$defs/InitializeParams"
    StreamStart:
      name: ainm.stream.start
      title: Stream Start
      contentType: application/json
      payload:
        $ref: "schemas/streaming.schema.json#/$defs/StreamStart"
```

### Pattern 3: Rust Demo Single-Binary Architecture

**What:** One binary that spawns server + agent + REPL as concurrent tokio tasks.
**When to use:** The demo entry point (D-02).

```rust
// main.rs -- conceptual structure
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::init();

    // 1. Generate identity key pairs
    let human_keys = identity::generate_keypair("alice@demo.ainm.local")?;
    let agent_keys = identity::generate_keypair("helper@demo.ainm.local")?;

    // 2. Start Axum server on localhost (includes identity registry)
    let (server_addr, server_handle) = server::start(human_keys.clone(), agent_keys.clone()).await?;

    // 3. Connect agent as in-process WebSocket client
    let agent_handle = tokio::spawn(agent::run(server_addr, agent_keys));

    // 4. Run terminal REPL (blocking on main thread via spawn_blocking)
    let repl_handle = tokio::spawn(repl::run(server_addr, human_keys));

    // 5. Wait for REPL exit, then shutdown
    repl_handle.await??;
    server_handle.abort();
    agent_handle.abort();
    Ok(())
}
```

### Pattern 4: Docusaurus Spec Import Strategy

**What:** Import existing `spec/*.md` files into Docusaurus without duplicating content.
**When to use:** Rendering the 11 spec files in the documentation site (D-11).

Two viable approaches:
1. **Symlink/copy in build**: Copy spec/ files to website/docs/spec/ during build step. Simple, works with sidebars.
2. **Docusaurus path config**: Configure `@docusaurus/plugin-content-docs` with `path: '../spec'` for a second docs plugin instance.

Recommended: **Option 1 (copy in build)**. Avoids Docusaurus path resolution issues. Add a build script:
```json
{
  "scripts": {
    "prebuild": "cp -r ../spec ./docs/spec && cp -r ../schemas ./static/schemas",
    "build": "docusaurus build"
  }
}
```

### Anti-Patterns to Avoid

- **Monolithic schema file:** Do NOT put all schemas in a single file. The spec has 11 distinct domains -- match that structure (D-05).
- **Schema-first redesign:** Do NOT redesign message structures during schema writing. Schemas formalize existing inline examples (D-06). If an example conflicts with prose, flag it -- do not silently change the schema.
- **Dynamic AsyncAPI loading:** Do NOT fetch asyncapi.yaml at runtime in Docusaurus. Use the docusaurus-asyncapi preset which processes it at build time.
- **Full ratatui TUI:** Do NOT build a full TUI application for the demo. A simple REPL with colored output is sufficient (D-01). Ratatui adds complexity with no benefit for a protocol showcase.
- **External agent dependency:** Do NOT require an LLM API key or network access. The agent is scripted/mock (D-03).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON Canonicalization (RFC 8785) | Custom JSON serializer with sorted keys | `serde_json_canonicalizer` v0.3.2 | Edge cases in number representation, Unicode normalization. The crate handles ECMAScript number serialization rules. |
| Ed25519 signing | Custom crypto | `ring` v0.17.14 | Audited, misuse-resistant API. Ed25519KeyPair + UnparsedPublicKey. |
| Base64url encoding | Manual encoding | `base64` v0.22.1 with URL_SAFE_NO_PAD | Off-by-one errors in padding, alphabet confusion. |
| UUIDv7 generation | Custom timestamp-based UUID | `uuid` v1.23.0 with `v7` feature | Correct timestamp embedding, proper random bits. |
| AsyncAPI rendering | Custom parser + renderer | `docusaurus-asyncapi` v1.0.2 | The @asyncapi/react-component has complex parsing requirements. The preset handles webpack integration. |
| Terminal line editing | Custom stdin reader | `rustyline` v17.0.2 | History, line editing, ctrl-c handling, Unicode support. |
| Markdown rendering in terminal | Custom parser | `termimad` (optional) | Handles bold, italic, code, lists. Not critical for v1 demo. |

**Key insight:** The demo's value is proving the *protocol* works, not demonstrating bespoke terminal rendering. Use established libraries for everything that is not AINM-protocol-specific.

## Common Pitfalls

### Pitfall 1: AsyncAPI + Docusaurus Webpack Node.js Polyfill Failure

**What goes wrong:** Importing `@asyncapi/react-component` v3.x directly in Docusaurus causes webpack 5 build failures: "Can't resolve 'http'" and similar errors for Node.js core modules.
**Why it happens:** The AsyncAPI parser depends on `@apidevtools/json-schema-ref-parser` which requires Node.js built-in modules not polyfilled in webpack 5. This was flagged as a risk in STATE.md.
**How to avoid:** Use `docusaurus-asyncapi` v1.0.2 preset, which pins `@asyncapi/react-component` v2.6.3 (known working version). Do NOT upgrade to v3.x of the react component.
**Warning signs:** Build error mentioning 'http', 'https', 'stream', 'zlib' resolution failures.

### Pitfall 2: Schema $ref Resolution Across Files

**What goes wrong:** JSON Schema $ref paths break when schemas are moved, bundled, or served from different base URLs.
**Why it happens:** JSON Schema 2020-12 $ref resolution uses $id as the base URI. If $id URIs don't match actual hosting paths, validators can't resolve cross-file references.
**How to avoid:** Set $id on every schema file with a canonical URI (`https://ainm.dev/schemas/X.schema.json`). Use relative $ref for same-directory references (e.g., `"$ref": "common.schema.json#/$defs/UUIDv7"`). Test with jsonschema crate locally before publishing.
**Warning signs:** "Unable to resolve reference" errors during validation.

### Pitfall 3: Signing Input Mismatch Between Spec and Demo

**What goes wrong:** Demo produces different canonical JSON than what the spec describes, causing signature verification failures.
**Why it happens:** RFC 8785 has subtleties: number serialization (no trailing zeros, scientific notation thresholds), Unicode escaping rules. serde_json_canonicalizer handles these, but the signing input must exclude the `signature` field from the envelope before canonicalization.
**How to avoid:** Implement signing exactly per security.md Section 4: (1) clone envelope, (2) remove `signature` field, (3) canonicalize with JCS, (4) sign UTF-8 bytes with Ed25519. Test with the RFC 8032 Section 7.1 test vector from Phase 1 decisions.
**Warning signs:** Signature verification fails on messages that appear correct by inspection.

### Pitfall 4: AsyncAPI 3.1 Request-Reply Modeling for JSON-RPC

**What goes wrong:** Modeling JSON-RPC request/response over WebSocket in AsyncAPI is awkward because all messages share the same WebSocket address ("/"), so channels must be differentiated by message type, not address.
**Why it happens:** AsyncAPI channels traditionally map to different topics/addresses. WebSocket JSON-RPC multiplexes all messages on one connection.
**How to avoid:** Define logical channels by domain (initialize, messaging, streaming, threading, presence, memory) rather than by WebSocket path. Use the request-reply `reply` property on operations. Reference the Kraken WebSocket example from the AsyncAPI spec repo which uses address "/" for all channels.
**Warning signs:** AsyncAPI validator warnings about duplicate addresses, confusing channel organization.

### Pitfall 5: Demo Stream Timing -- Deltas Too Fast for Terminal

**What goes wrong:** Scripted agent sends stream deltas instantly (no actual LLM generation delay), making streaming indistinguishable from instant message delivery.
**Why it happens:** The agent is mock/scripted (D-03) with no real generation latency.
**How to avoid:** Add artificial delay between deltas (e.g., 30-80ms per token) using `tokio::time::sleep`. This makes the streaming protocol visible to the observer. Without delay, the demo fails to showcase the streaming lifecycle.
**Warning signs:** Stream-start and stream-end appear simultaneously, streaming looks like instant message delivery.

### Pitfall 6: Docusaurus Spec File Frontmatter Conflicts

**What goes wrong:** Existing spec/*.md files don't have Docusaurus-compatible frontmatter, causing build errors or wrong sidebar ordering.
**Why it happens:** Spec files have their own header format (Status, Version, Last Updated) that Docusaurus doesn't recognize.
**How to avoid:** Add Docusaurus frontmatter during the copy step or use a build script that prepends frontmatter to spec files. Alternatively, wrap spec content in MDX pages that import the raw markdown.
**Warning signs:** Sidebar shows files in wrong order, missing titles, Docusaurus parse errors.

## Code Examples

### Ed25519 Signing with RFC 8785 Canonical JSON (Rust)

```rust
// Source: ring docs + serde_json_canonicalizer docs + security.md Section 4
use ring::signature::{Ed25519KeyPair, UnparsedPublicKey, ED25519};
use serde_json_canonicalizer::to_string as canonicalize;

fn sign_message(
    key_pair: &Ed25519KeyPair,
    envelope: &mut serde_json::Value,
) -> Result<String, Box<dyn std::error::Error>> {
    // 1. Remove signature field for signing input
    let mut signing_input = envelope.clone();
    signing_input.as_object_mut().unwrap().remove("signature");

    // 2. Canonicalize per RFC 8785
    let canonical = canonicalize(&signing_input)?;

    // 3. Sign the canonical bytes
    let sig = key_pair.sign(canonical.as_bytes());

    // 4. Encode as base64url without padding
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    let sig_str = URL_SAFE_NO_PAD.encode(sig.as_ref());

    // 5. Attach signature to envelope
    envelope["signature"] = serde_json::Value::String(sig_str.clone());

    Ok(sig_str)
}

fn verify_signature(
    public_key_bytes: &[u8],
    envelope: &serde_json::Value,
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut signing_input = envelope.clone();
    let sig_str = signing_input["signature"].as_str().unwrap().to_string();
    signing_input.as_object_mut().unwrap().remove("signature");

    let canonical = canonicalize(&signing_input)?;

    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    let sig_bytes = URL_SAFE_NO_PAD.decode(&sig_str)?;

    let public_key = UnparsedPublicKey::new(&ED25519, public_key_bytes);
    Ok(public_key.verify(canonical.as_bytes(), &sig_bytes).is_ok())
}
```

### Axum WebSocket Server with JSON-RPC Dispatch (Rust)

```rust
// Source: Axum WebSocket examples + transport.md Section 4
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(text) = msg {
            let rpc: serde_json::Value = serde_json::from_str(&text).unwrap();
            let method = rpc["method"].as_str().unwrap_or("");

            match method {
                "ainm.initialize" => {
                    // Handle initialization, return capabilities
                    let response = create_init_response(&rpc);
                    socket.send(Message::Text(response)).await.ok();
                }
                "ainm.message.send" => {
                    // Verify signature, accept message, route to agent
                    let response = handle_message_send(&rpc).await;
                    socket.send(Message::Text(response)).await.ok();
                }
                _ => {
                    let error = json_rpc_error(rpc["id"].clone(), -32601, "Method not found");
                    socket.send(Message::Text(error.to_string())).await.ok();
                }
            }
        }
    }
}
```

### Terminal Rich Block Rendering (Rust)

```rust
// Source: Claude's discretion -- terminal formatting for interactive blocks
use colored::*;

fn render_content_block(block: &serde_json::Value) {
    let block_type = block["type"].as_str().unwrap_or("unknown");
    match block_type {
        "text" => {
            let text = block["text"].as_str().unwrap_or("");
            println!("{}", text);
        }
        "code" => {
            let lang = block["language"].as_str().unwrap_or("plain");
            let code = block["code"].as_str().unwrap_or("");
            println!("{}", format!("--- {} ---", lang).dimmed());
            println!("{}", code.cyan());
            println!("{}", "----------".dimmed());
        }
        "button" => {
            let label = block["label"].as_str().unwrap_or("Button");
            let style = block["style"].as_str().unwrap_or("primary");
            let styled = match style {
                "primary" => format!("[ {} ]", label).bold().blue(),
                "danger" => format!("[ {} ]", label).bold().red(),
                _ => format!("[ {} ]", label).bold().white(),
            };
            println!("  {}", styled);
        }
        "form" => {
            let title = block["title"].as_str().unwrap_or("Form");
            println!("{}", format!("=== {} ===", title).bold().yellow());
            if let Some(fields) = block["fields"].as_array() {
                for field in fields {
                    let label = field["label"].as_str().unwrap_or("Field");
                    let ftype = field["field_type"].as_str().unwrap_or("text");
                    println!("  {} ({}): ___", label.bold(), ftype.dimmed());
                }
            }
            println!("{}", "============".bold().yellow());
        }
        "approval" => {
            let prompt = block["prompt"].as_str().unwrap_or("Approval needed");
            println!("{}", format!("! APPROVAL: {} !", prompt).bold().on_yellow().black());
            if let Some(options) = block["options"].as_array() {
                for opt in options {
                    let label = opt["label"].as_str().unwrap_or("Option");
                    let style = opt["style"].as_str().unwrap_or("primary");
                    let styled = match style {
                        "primary" => format!("  [{}]", label).green(),
                        "danger" => format!("  [{}]", label).red(),
                        _ => format!("  [{}]", label).white(),
                    };
                    println!("{}", styled);
                }
            }
        }
        _ => {
            // Fallback for unknown block types
            let fallback = block["fallback"].as_str().unwrap_or("[unsupported block]");
            println!("{} {}", "[fallback]".dimmed(), fallback);
        }
    }
}
```

### Docusaurus Config with AsyncAPI Preset

```javascript
// Source: docusaurus-asyncapi README + Docusaurus docs
// website/docusaurus.config.js
const config = {
  title: 'AI-Native Messaging Protocol',
  tagline: 'Where humans and agents are equal participants',
  url: 'https://your-org.github.io',
  baseUrl: '/ai-native-messaging/',
  organizationName: 'your-org',
  projectName: 'ai-native-messaging',
  trailingSlash: false,

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.js',
          path: 'docs',
        },
      },
    ],
    [
      'docusaurus-asyncapi',
      {
        specs: [
          {
            spec: '../asyncapi.yaml',
            route: '/asyncapi/',
          },
        ],
        config: {
          show: {
            sidebar: true,
          },
        },
      },
    ],
  ],
};

module.exports = config;
```

### GitHub Actions Deploy Workflow

```yaml
# Source: Docusaurus deployment docs
# .github/workflows/deploy-docs.yml
name: Deploy Docs to GitHub Pages

on:
  push:
    branches: [main]
    paths:
      - 'website/**'
      - 'spec/**'
      - 'schemas/**'
      - 'asyncapi.yaml'

permissions:
  pages: write
  id-token: write

jobs:
  build-deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - name: Install and build
        working-directory: website
        run: |
          npm ci
          cp -r ../spec ./docs/spec
          cp -r ../schemas ./static/schemas
          npm run build
      - uses: actions/upload-pages-artifact@v3
        with:
          path: website/build
      - uses: actions/deploy-pages@v4
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| AsyncAPI 2.x | AsyncAPI 3.1.0 | 2024 | v3 restructured channels/operations. Must use v3 patterns (operations are separate from channels). |
| @asyncapi/react-component direct import | docusaurus-asyncapi preset | 2025 | Preset handles webpack issues. Direct v3.x import still broken in Docusaurus. |
| JSON Schema draft-07 | JSON Schema 2020-12 | 2020 | $defs replaces definitions, dynamic references added. jsonschema Rust crate v0.45.0 supports 2020-12. |
| serde_jcs | serde_json_canonicalizer | 2023 | serde_jcs abandoned, had RFC 8785 compliance bugs. serde_json_canonicalizer v0.3.2 is actively maintained. |
| ring 0.16.x | ring 0.17.14 | 2024 | API changes in key generation. Use `Ed25519KeyPair::generate_pkcs8` with SystemRandom. |

**Deprecated/outdated:**
- `serde_jcs` -- abandoned, do not use. Use `serde_json_canonicalizer` instead.
- Direct `@asyncapi/react-component` import in Docusaurus -- use the `docusaurus-asyncapi` preset.
- AsyncAPI 2.x syntax in asyncapi.yaml -- use 3.1.0 channel/operation structure.

## Open Questions

1. **Schema Hosting Base URI**
   - What we know: Schemas need $id URIs for cross-file $ref resolution. A canonical base URL like `https://ainm.dev/schemas/` is ideal.
   - What's unclear: Whether the project has a domain name, or if GitHub Pages URL should be the $id base.
   - Recommendation: Use `https://ainm.dev/schemas/` as the canonical $id base. If the domain doesn't exist yet, the $id still works for schema resolution -- it's an identifier, not a required fetch URL. Update later when hosting is confirmed.

2. **docusaurus-asyncapi Preset with Dual Presets**
   - What we know: Docusaurus supports multiple presets. The config uses both `classic` and `docusaurus-asyncapi`.
   - What's unclear: Whether the docusaurus-asyncapi preset's theme conflicts with classic preset's theme.
   - Recommendation: Test early in implementation. If conflicts arise, fall back to embedding the AsyncAPI standalone bundle via `useEffect` in an MDX page.

3. **Spec File Frontmatter Transformation**
   - What we know: Existing spec files have `**Status:** Draft` / `**Version:** 0.1.0` headers, not Docusaurus frontmatter (`---` YAML block).
   - What's unclear: Whether Docusaurus can render them without frontmatter, or if a transformation script is needed.
   - Recommendation: Add a build-time script that prepends Docusaurus frontmatter (title, sidebar_position) to spec files during copy. Keep original spec files unchanged.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Node.js | Docusaurus build | Yes | v24.7.0 | -- |
| npm | Package management | Yes | 11.5.1 | -- |
| Rust (cargo/rustc) | Demo binary | Yes | 1.94.0 | -- |
| Git | Version control | Yes | (system) | -- |

**Missing dependencies with no fallback:** None -- all required tooling is available.

**Missing dependencies with fallback:** None.

## Sources

### Primary (HIGH confidence)
- JSON Schema 2020-12 structuring and bundling: [json-schema.org/understanding-json-schema/structuring](https://json-schema.org/understanding-json-schema/structuring)
- JSON Schema bundling formalized: [json-schema.org/blog/posts/bundling-json-schema-compound-documents](https://json-schema.org/blog/posts/bundling-json-schema-compound-documents)
- AsyncAPI 3.1.0 specification: [asyncapi.com/docs/reference/specification/v3.1.0](https://www.asyncapi.com/docs/reference/specification/v3.1.0)
- Axum WebSocket examples: [github.com/tokio-rs/axum/examples/websockets](https://github.com/tokio-rs/axum/blob/main/examples/websockets/src/main.rs)
- ring::signature docs: [docs.rs/ring/latest/ring/signature](https://docs.rs/ring/latest/ring/signature/index.html)
- serde_json_canonicalizer: [crates.io/crates/serde_json_canonicalizer](https://crates.io/crates/serde_json_canonicalizer)
- Docusaurus deployment: [docusaurus.io/docs/deployment](https://docusaurus.io/docs/deployment)
- tokio-tungstenite examples: [github.com/snapview/tokio-tungstenite](https://github.com/snapview/tokio-tungstenite)

### Secondary (MEDIUM confidence)
- docusaurus-asyncapi preset: [github.com/mtso/docusaurus-asyncapi](https://github.com/mtso/docusaurus-asyncapi) -- v1.0.2, published 2025-04-16, actively maintained
- AsyncAPI React component Docusaurus issue #734: [github.com/asyncapi/asyncapi-react/issues/734](https://github.com/asyncapi/asyncapi-react/issues/734) -- confirmed webpack polyfill issue; docusaurus-asyncapi preset is the validated workaround
- Kraken WebSocket request-reply AsyncAPI example: [github.com/asyncapi/spec/examples](https://github.com/asyncapi/spec/blob/master/examples/kraken-websocket-request-reply-multiple-channels-asyncapi.yml) -- reference for modeling JSON-RPC over WebSocket in AsyncAPI

### Tertiary (LOW confidence)
- None -- all findings verified with primary or secondary sources.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all crate/package versions verified against registries on 2026-03-28. CLAUDE.md tech stack confirmed.
- Architecture: HIGH -- patterns derived from official examples (Axum WebSocket, JSON Schema bundling) and locked decisions (D-01 through D-13).
- Pitfalls: HIGH -- AsyncAPI+Docusaurus issue verified via GitHub issue #734 and docusaurus-asyncapi workaround confirmed. JCS/signing patterns verified against RFC 8785 and ring docs.

**Research date:** 2026-03-28
**Valid until:** 2026-04-28 (30 days -- stable ecosystem, no fast-moving dependencies)
