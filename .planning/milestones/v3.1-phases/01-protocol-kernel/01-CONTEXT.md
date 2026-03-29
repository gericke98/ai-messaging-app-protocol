# Phase 1: Protocol Kernel - Context

**Gathered:** 2026-03-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Define the foundational protocol specification: identity, security, transport, content format, threading, and mixed human-agent threads. Output is a set of modular spec documents that a developer can read and implement independently. No reference implementation code -- spec only.

</domain>

<decisions>
## Implementation Decisions

### Spec Document Organization
- **D-01:** Modular per-domain files: `spec/identity.md`, `spec/security.md`, `spec/transport.md`, `spec/content.md`, `spec/threading.md`. Each stands alone.
- **D-02:** Developer-friendly prose style. Use RFC keywords (MUST/SHALL/MAY) for normative statements only, not throughout. Think Stripe API docs meets protocol spec.
- **D-03:** Inline JSON examples throughout -- every message type gets a concrete example right next to its definition.
- **D-04:** Spec files live at `spec/` directory at project root.

### Identity Resolution
- **D-05:** `.well-known` URL pattern for identity resolution. `GET https://{provider}/.well-known/ainm/{name}` returns identity data.
- **D-06:** Flat JSON response: `{ "id", "public_key", "server", "type", "agent_card_url" }`. One request to get everything needed to communicate.
- **D-07:** Key rotation via key history array -- current key + array of previous keys with validity periods. Old messages remain verifiable.
- **D-08:** Participant type (human/agent) exposed in identity metadata via `type` field. Protocol treats both equally but clients can adapt UI (e.g., presence semantics).

### Message Envelope Design
- **D-09:** Lean envelope: routing (from, thread_id), message metadata (id, timestamp, type), signature, and content blocks array. Trust annotations live INSIDE content blocks, not the envelope.
- **D-10:** Trust annotations via `source` field per content block: `user`, `agent`, or `system`. Set by sender, verified by signature. Directly addresses prompt injection.
- **D-11:** Thread-addressed messages -- messages sent TO a thread (thread_id), not to individual participants. All thread members receive. DMs = 2-person thread.
- **D-12:** At-least-once delivery with client-side deduplication via unique message IDs.

### Content Block Extensibility
- **D-13:** Closed core set + namespaced extensions. Core types (text, code, image, file) are protocol-defined. Third parties use reverse-domain namespacing (`com.example.custom-chart`). Clients MUST render core, MAY render extensions.
- **D-14:** Mandatory `fallback` field on every content block -- plain text or markdown. Unknown block types render the fallback. Guarantees graceful degradation.
- **D-15:** Block-level `version` field. Each block type can evolve independently from the protocol version. Clients check version to know which fields to expect.

### Claude's Discretion
- No areas deferred to Claude's discretion -- all gray areas resolved with explicit decisions.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Research
- `research/landscape-protocols-projects.md` -- Survey of 15+ protocols, 10+ frameworks, competing products. Key design influences: AMP, A2A, A2UI, Letta/MemGPT, MCP, AG-UI
- `research/bottlenecks-current-messaging-apps.md` -- Analysis of Telegram, Slack, Discord, WhatsApp limitations for agent communication

### Standards & Specs Referenced
- RFC 8785 (JSON Canonicalization Scheme) -- Required for deterministic signing format
- RFC 7033 (WebFinger) -- Considered but NOT chosen; using `.well-known` URL pattern instead
- RFC 2119 (Key words for RFCs) -- MUST/SHALL/MAY keywords used sparingly in normative statements
- RFC 7517 (JWK) -- Considered but NOT chosen for identity response; using flat JSON instead
- AsyncAPI v3.1.0 -- Transport binding definitions (Phase 3 formalization, but informs Phase 1 design)
- JSON Schema 2020-12 -- Message schema definitions (Phase 3 formalization, but informs Phase 1 design)

### Protocol Influences
- AMP protocol -- Email-like addressing, Ed25519 signatures, federation model
- A2A (Google) -- Agent Cards for capability discovery (Phase 2, but identity metadata informs Phase 1)
- A2UI (Google) -- Declarative UI-as-data model (Phase 2, but content block design informs Phase 1)
- MCP (Anthropic) -- JSON-RPC simplicity pattern

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- No existing code -- greenfield spec-writing phase.

### Established Patterns
- No codebase patterns yet. This phase establishes the foundational patterns.

### Integration Points
- Spec files at `spec/` will be consumed by Phase 3 (Docusaurus site, JSON Schema formalization, AsyncAPI bindings).
- Identity resolution design directly feeds Phase 2 Agent Card discovery (DISC-01 through DISC-05).

</code_context>

<specifics>
## Specific Ideas

No specific requirements -- open to standard approaches within the decisions captured above.

</specifics>

<deferred>
## Deferred Ideas

None -- discussion stayed within phase scope.

</deferred>

---

*Phase: 01-protocol-kernel*
*Context gathered: 2026-03-28*
