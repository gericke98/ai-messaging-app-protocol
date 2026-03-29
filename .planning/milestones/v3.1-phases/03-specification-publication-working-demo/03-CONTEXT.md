# Phase 3: Specification Publication & Working Demo - Context

**Gathered:** 2026-03-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Formalize the complete AINM protocol specification with machine-readable JSON Schemas and AsyncAPI transport bindings, publish it as an open document via a Docusaurus site, and build a working CLI demo (Rust + Axum) that proves the protocol end-to-end: one human and one agent in a thread with streaming and rich interactive content, all messages cryptographically signed. No iOS app, no federation, no full production server -- spec publication + proof-of-concept demo only.

</domain>

<decisions>
## Implementation Decisions

### Demo Scope & Architecture
- **D-01:** CLI terminal demo -- Rust server + terminal REPL. Human types messages, agent responds with streaming text and rich content blocks rendered as formatted terminal output. No web UI.
- **D-02:** Single binary architecture -- one `cargo run --bin ainm-demo` starts the Axum WebSocket server, registers identities, connects the agent as an in-process WebSocket client, and opens a terminal REPL for human input. External devs run one command.
- **D-03:** Mock/scripted agent -- hardcoded responses that showcase different protocol features (streaming, Form, Button, Approval, CodeBlock, tool call). No API key needed, works offline, reproducible.
- **D-04:** Core flow + highlights coverage -- identity registration/resolution, thread creation, message signing/verification, streaming (start/delta/end with multi-block), 2-3 interactive blocks (Form, Button, Approval), Agent Card serve/resolve, and one tool visibility demo. NOT exercised: federation, memory tiers, MCP/A2A bridges, sub-threading.

### Schema Organization
- **D-05:** Per-domain JSON Schema bundles mirroring spec/ structure: `schemas/identity.schema.json`, `schemas/security.schema.json`, `schemas/content.schema.json`, `schemas/interactive.schema.json`, `schemas/transport.schema.json`, `schemas/threading.schema.json`, `schemas/streaming.schema.json`, `schemas/agent-card.schema.json`, `schemas/memory.schema.json`, `schemas/tool-visibility.schema.json`, `schemas/compatibility.schema.json`, plus `schemas/common.schema.json` for shared definitions.
- **D-06:** Schemas derived from inline JSON examples in the existing 11 spec files. The prose defines constraints (MUST/SHALL), the examples show structure. Schema writing is a formalization pass -- read spec, find examples, derive schema, add prose constraints, link shared definitions via $ref.
- **D-07:** Single AsyncAPI document (`asyncapi.yaml`) defining all channels (client-server WebSocket, server-server HTTP). References per-domain JSON Schema files via $ref.

### Spec Versioning
- **D-08:** Initial protocol version: 0.1.0. Signals "working draft, expect changes." SemVer rules apply but major=0 means no stability guarantees.
- **D-09:** Single protocol version for the whole spec -- all 11 spec files, all schemas, and asyncapi.yaml share v0.1.0. Block-level versioning (Phase 1 D-15) is separate and handles content evolution independently.
- **D-10:** SemVer with 0.x freedom. Breaking changes: field removal/rename, type change, message type removal, signing rule changes (require major bump after 1.0). Non-breaking: optional fields, new message types, new block types, new JSON-RPC methods. During 0.x, breaking changes allowed in any release but documented in CHANGELOG.md.

### Publication Site
- **D-11:** Docusaurus 3.x spec-centric site with reference sidebar. Structure: landing page (what/why/quickstart), /spec/ (11 rendered spec files), /schemas/ (JSON Schema reference), /asyncapi/ (embedded explorer), /demo/ (how to run), /changelog (version history).
- **D-12:** Hosted on GitHub Pages with GitHub Actions CI/CD on push. Free, standard for open-source protocol docs.
- **D-13:** AsyncAPI explorer embedded via @asyncapi/react-component in a Docusaurus MDX page. Renders interactive channel/message exploration inline. Addresses the STATE.md concern about AsyncAPI + Docusaurus integration -- this is the validated integration path.

### Claude's Discretion
- Terminal output formatting for rich content blocks (how Forms, Buttons, Approvals render in terminal)
- Demo agent's specific scripted conversation scenarios
- Schema $ref linking strategy and common.schema.json content decomposition
- Docusaurus theme/styling choices
- GitHub Actions workflow configuration details
- CHANGELOG.md format and structure
- Landing page copy and quick-start guide content

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Protocol Spec Files (ground truth for schema extraction)
- `spec/identity.md` -- Identity model, registration, resolution, key rotation
- `spec/security.md` -- Message signing, canonical format, trust annotations
- `spec/transport.md` -- WebSocket + HTTP bindings, JSON-RPC lifecycle, method registry
- `spec/content.md` -- Content block base schema, core block types
- `spec/interactive.md` -- ButtonBlock, FormBlock, TableBlock, ApprovalBlock, ActionBlock
- `spec/threading.md` -- Thread model, sub-threading, presence, mixed participation
- `spec/streaming.md` -- Stream lifecycle, deltas, multi-block, cancellation, collapse
- `spec/agent-card.md` -- Agent Card format, discovery, A2A compatibility
- `spec/memory.md` -- Three-tier memory, context frames, operations
- `spec/tool-visibility.md` -- Tool call events, lifecycle, rendering
- `spec/compatibility.md` -- MCP/A2A bridge mappings

### Prior Phase Context
- `.planning/phases/01-protocol-kernel/01-CONTEXT.md` -- Foundational decisions (modular specs, developer-friendly prose, extensions model, fallback, block versioning)
- `.planning/phases/02-agent-native-features/02-CONTEXT.md` -- Agent-native decisions (streaming model, interactive blocks, Agent Cards, memory architecture)

### Standards
- JSON Schema 2020-12 -- Target schema draft for all definitions
- AsyncAPI v3.1.0 -- Transport binding specification format
- SemVer 2.0.0 -- Versioning scheme for protocol releases

### Tech Stack (from CLAUDE.md)
- Rust 1.85+ with Axum 0.8.x -- Reference server / demo
- Docusaurus 3.x -- Spec publication site
- @asyncapi/react-component -- AsyncAPI explorer embedding
- @asyncapi/cli -- AsyncAPI spec validation in CI

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- 11 spec files in `spec/` (~442KB total) with inline JSON examples for every message type -- these ARE the schema extraction source
- JSON-RPC method naming convention (`ainm.*`) established across transport.md, streaming.md, memory.md
- Content block base schema defined in content.md Section 3 -- common.schema.json inherits from this

### Established Patterns
- RFC-keyword prose with inline JSON examples (Phase 1 D-02/D-03)
- Closed core + namespaced extensions (Phase 1 D-13) -- schemas must encode this pattern
- Thread-addressed messages (Phase 1 D-11) -- all events go to threads
- Ed25519 signing with RFC 8785 canonical JSON -- demo must implement this

### Integration Points
- `schemas/` directory will hold all JSON Schema files (new)
- `asyncapi.yaml` at project root (new)
- `demo/` directory for Rust demo binary (new, or `crates/demo/`)
- `website/` or `docs/` directory for Docusaurus site (new)
- Spec files remain in `spec/` -- Docusaurus renders them via MDX or raw markdown import

</code_context>

<specifics>
## Specific Ideas

- The demo should feel like a protocol showcase, not a chat app. Each scripted interaction should demonstrate a specific protocol feature (streaming, forms, approvals, tool calls) with signature verification visible in terminal output.
- AsyncAPI + Docusaurus integration via @asyncapi/react-component is the key technical risk identified in STATE.md -- validate this early in implementation.
- Schema extraction is a formalization pass, not a redesign. The inline JSON examples in the spec files are the ground truth.

</specifics>

<deferred>
## Deferred Ideas

None -- discussion stayed within phase scope.

</deferred>

---

*Phase: 03-specification-publication-working-demo*
*Context gathered: 2026-03-28*
