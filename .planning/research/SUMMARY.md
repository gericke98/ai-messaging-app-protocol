# Project Research Summary — v3.3 Universal Agent Client

**Project:** AI-Native Messaging Protocol & App
**Milestone:** v3.3 Universal Agent Client
**Researched:** 2026-04-18
**Confidence:** HIGH

## Executive Summary

v3.3 is an ecosystem-expansion milestone on top of a stable v3.1 / v3.2 protocol foundation. The core challenges are not protocol design (locked) but: (1) maintaining cross-language cryptographic parity as a Python SDK is added, (2) hardening connector identity beyond the current soft-fence trust model, and (3) wiring together shipped-but-disconnected components (AgentRegistry is live; iOS discovery reads HTTP snapshots; no push channel exists yet). Thirteen items of v3.2 tech debt must be closed first, because those items directly validate the signing substrate that every new connector depends on.

The principal risks are all cryptographic: JCS canonicalization drift between Python and TypeScript (silent at unit-test level, catastrophic in production), the current single-key connector model (one compromise = full impersonation + message forgery), and the absence of a server-bound challenge nonce in the handshake (replay attack window). All three risks have clear mitigations that must land in Phases 15/16 before any new connector surface ships.

## Key Findings

### Stack (HIGH confidence)

- **`rfc8785` 0.1.4 (Trail of Bits)** is the ONLY acceptable Python JCS library — it is the only one with a verified RFC 8785 conformance test suite. Any other library causes silent signature drift.
- **`@modelcontextprotocol/sdk` 1.29.0 (TypeScript)**: MCP Bridge built in TypeScript to share the existing connector toolchain (not Python MCP SDK 1.27.0).
- **`openai` Node 6.34.0**: Responses API upgrade is not optional — Assistants API shuts down 2026-08-26.
- **Python floor: 3.12+** (STACK.md overrules ARCHITECTURE.md's 3.11 suggestion — need PEP 695 generics).
- **Zero new dependencies** for connector auth, iOS live discovery, or compound memory groundwork — all are protocol additions on existing crypto/transport.

Python SDK stack: `websockets` 16.0 + `rfc8785` 0.1.4 + `cryptography` 44 (PyCA) + `httpx` 0.28 + `pydantic` 2.9 + `fastapi`/`uvicorn` for Agent Card serving.

### Features (HIGH confidence)

**P1 Must-Ship (milestone fails without these):**
- Multi-agent thread UX: MA-01..07 (distinct identity, parallel indicators, always-attributed messages, tool attribution, @-mention server routing, silence, remove)
- MA-05 is security-class: server-side @-mention routing blocks $47K runaway-loop failure case
- Connector lifecycle: CN-01/02/04/05 (add from discovery, URL invite, list, revoke)
- Auth: AU-01/02 (signed handshake, paste-token onboarding)
- Tool transparency: TC-01/02 (visible by default, collapsed)
- MCP: MCP-01/02 (MCP server as agent participant, attributed tool calls)
- Memory groundwork: MEM-01/02 (provenance tags, connector memory write channel)

**P2 Should-Ship (strong v3.3, acceptable to defer under phase pressure):**
- TC-03/04/05 (grouped calls, truncation, approve-before-run policy)
- CN-03 (QR pairing)
- MCP-03/04 (MCP Apps inline UI, per-tool consent)
- MEM-03/05 (preview writes, export)
- AU-03/04 (OAuth browser, API-key local)
- DIFF-06 (Python SDK — A5 scope item)
- DIFF-07 (MCP compilation — A6 scope item)

**P3 Defer to v3.4+:**
- MEM-04 (cross-agent read ACL — the full compound-memory UX)
- DIFF-02/03 (per-turn memory viz, replaceable agents)
- DIFF-04/05 (parallel conference, HandoffBlock)
- DIFF-01 (signed tool calls — nice-to-have hardening)

**10 documented anti-features** to resist — most critical:
- ANTI-01 (ungated agent loops — blocked by MA-05)
- ANTI-02 (silent memory writes — blocked by MEM-03 preview)
- ANTI-03 (hidden tool calls — blocked by TC-01 visible-default)
- ANTI-05 (cross-agent memory by default — blocked by MEM-04 default-deny, slipped to v3.4)

### Architecture (HIGH confidence — direct codebase reads)

- **`connector_keys` table** is the load-bearing schema addition. Keyed on `(user_addr, agent_identity)`, it gates both identity auth and memory scopes for every subsequent phase. TOFU-in-dev / strict-in-prod recommended.
- **Memory is already user-scoped** in `demo/src/graph.rs` — every INSERT/SELECT filters on `user_addr`. Compound memory's storage layer is ready; the v3.3 gap is *consent policy* (per-agent `memory_scopes`), not schema.
- **Fan-out is currently a flat `broadcast::Sender<String>`** — every agent sees every human message, filtered client-side by `thread_id`. MA-05 delivery gating requires server-side enforcement.
- **iOS live directory uses existing HumanRegistry broadcast** — two new JSON-RPC methods (`ainm.directory.list` request + `ainm.directory.updated` push notification) + a cold-start `GET /.well-known/ainm/directory` endpoint. No new channel.
- **Python SDK produces zero server-side changes** if wire protocol is byte-identical to TypeScript SDK.
- **Two-key connector model**: split `k_sign` (stable, published in Agent Card, message-level) from `k_conn` (per-runtime, ≤90 days, handshake only). One compromise ≠ full impersonation.
- **OpenAI Responses API: 1:1 thread_id → response chain**. AINM sub-threads start fresh chains. OpenAI-hosted tools (web_search, code_interpreter) surface as `ainm.tool.*` events with `openai:` prefix; AINM ApprovalBlock cannot gate them (documented limitation, deferred to v3.4).
- **MCP Bridge: one process per MCP server** (not one meta-bridge). Aligns with AINM's 1 identity ↔ 1 capability-set model; kills tool-name collisions; isolates failure.

### Pitfalls (HIGH confidence, 12 total)

| # | Pitfall | Phase | Prevention |
|---|---------|-------|------------|
| 1 | Python JCS canonicalization drift | 17 | `rfc8785` only; official test-vector corpus in CI; cross-SDK contract test (20 vectors, Rust-generated) |
| 2 | Orphaned MCP stdio subprocesses | 19 | `detached: true` + process-group kill; SIGTERM→SIGKILL escalation; leak-detection test |
| 3 | Single-key compromise (auth + signing same key) | 16 | Two-key: `k_sign` (stable, in Agent Card) + `k_conn` (per-runtime ≤90 days, handshake only) |
| 4 | Replay-vulnerable handshake (timestamp-only) | 16 | Server-bound challenge nonce; one-time nonce cache; 30s TTL |
| 5 | Responses API event-shape break in tool UI | 18 | Exhaustive discriminated-union translator; record+replay cassettes for 6+ event types |
| 6 | MCP schema translation drops constraints | 19 | Forward full `inputSchema`; local `ajv` validation before forwarding; handle `tools/list_changed` |
| 7 | Stale iOS AgentRegistry after reconnect | 20 | Snapshot on every reconnect; generation tokens on deltas; view-scoped Task lifecycle |
| 8 | Cross-agent memory leak without ACL | 20 | Scope model `private/shared:thread/shared:user/public:user`; default `private`; server read-time filter |
| 9 | Memory pollution from bad extraction | 20 | Confidence scores; `source_message_id` traceability; retract+supersede (not delete) |
| 10 | Reconnect storm on server restart | 16 | Exponential backoff + full jitter; server 503 + Retry-After; capacity-aware admission |
| 11 | Off-GSD ratification without checklist | 15 | 8-point checklist; two-person sign-off; budget 50-70% of greenfield, not 10% |
| 12 | v3.2 tech debt items treated as flat list | 15 | Dependency map first; execute clusters; full XCUITest suite after each cluster |

## Definitive Build Order

```
Phase 15 — v3.2 Tech Debt + Off-GSD Ratification       [standalone, S]
    ↓
Phase 16 — Connector Auth Hardening + A1–A4 Ratification  [keystone, M]
    ↓ ──────────────────────────────── parallel ────────────────────────────
Phase 17: Python SDK (A5)           [M]    Phase 18: OpenAI Responses API [S]
    ↓ ──────────────────────────────── converge ────────────────────────────
Phase 19 — MCP Bridge Connector (A6)                      [M]
    ↓
Phase 20 — iOS Live Agent Discovery + Compound Memory Groundwork  [M]
```

**Dependency justifications:**
- 15 before 16: crypto confidence from UAT + ratification gives Python SDK authors authoritative test vectors
- 16 before all: `connector_keys` table is the trust anchor for identity, memory scopes, and enrollment; shipping connectors before this creates two remediation points
- 17 ‖ 18: different engineers, different code, share only auth contract from 16
- 19 after 17/18: MCP Bridge is a consumer of SDK patterns; benefits from stabilized stream semantics and error handling
- 20 last: most valuable when multiple connector types are producing directory data; memory consent validated with concurrent connectors

## Researcher Disagreements Resolved

| Disagreement | Winner | Rationale |
|---|---|---|
| Python floor: ARCHITECTURE.md says 3.11, STACK.md says 3.12+ | **3.12+** | STACK.md verified PEP 695 availability |
| OpenAI Responses: ARCHITECTURE.md opening says stateless; detail says threaded | **Thread-mapped chaining** (`previous_response_id` per `thread_id`) | Stateless loses reasoning-token context on multi-turn threads |
| MCP Bridge phase: PITFALLS.md says 18; ARCHITECTURE.md says 19 | **Phase 19** | ARCHITECTURE.md's full dependency graph analysis is better-reasoned |

## Cross-Cutting Concerns

| Concern | Enforcement |
|---|---|
| JCS cross-language parity | Cross-SDK contract test in CI root; PR to either SDK requires parity checklist sign-off |
| Connector auth as structural lever | `connector_keys` gates both identity AND memory access; nothing bypasses it |
| `dist/` directories excluded from git | `.gitignore` + CI check (Phase 15 ratification) |
| Spec-implementation conformance | Every new message type updates `spec/schemas/*.json` + `website/static/asyncapi.yaml` |
| Structured logging / no secret leakage | CI grep gate rejecting `console.log(signature/nonce)` patterns |
| Exact version pinning (no `^`) | Audited Phase 15; documented per-package in `DECISIONS.md` |
| Memory scope privacy-by-default | Default `private`; `shared:*` requires signed write + user notification; server rejects writes without `scope` field |

## Open Questions

**Must resolve in v3.3:**
- Two-key wire shape: exact Agent Card + handshake message fields for `k_sign` vs `k_conn` (Phase 16)
- Challenge-nonce cache: in-memory LRU vs PostgreSQL, TTL choice (Phase 16)
- Memory scope enum: exact values and upgrade path from unscoped legacy facts (Phase 20)
- MCP tool-selection default: explicit `@bridge:tool` vs keyword heuristic (Phase 19)
- `ainm.agents.subscribe` vs `ainm.directory.list` naming — pick `ainm.directory.list` + `ainm.directory.updated` (mirrors `ainm.thread.*` namespace)

**Can defer to v3.4:**
- MCP `elicitation/create` bridging to ApprovalBlock
- OpenAI-hosted tool approval gates (web_search, code_interpreter)
- Multi-device connector pairing
- Cross-agent memory aggregation, compaction, export (MEM-04 + full compound memory UX)
- LLM-assisted MCP tool selection
- Cost transparency per connector (per-reply $ estimate)
- Federation × connector auth (cross-server enrollment)

## Confidence

| Area | Confidence | Notes |
|---|---|---|
| Stack | HIGH | All library versions verified via official sources April 2026 |
| Features | HIGH | Table stakes verified against Slack/Discord/Cline/Cursor/MCP Apps; anti-features backed by 2026 failure cases |
| Architecture | HIGH | Primary confidence from live codebase reads; external API integration points MEDIUM |
| Pitfalls | HIGH | All 12 backed by official sources or existing project code; zero speculative pitfalls |

**Overall: HIGH**

## Roadmap-Ready Framing

v3.3 delivers a production-hardened connector ecosystem — Python SDK, MCP Bridge, OpenAI Responses API, real connector identity auth, iOS live agent discovery, and compound memory groundwork — across 6 phases (15–20), with **Phase 16 (connector auth hardening) as the keystone**: it ships the `connector_keys` trust anchor that gates connector identity, memory scoping, and the two-key protocol extension, and without it no new connector surface is trustworthy enough to ship. Phase 15 closes 13 items of v3.2 tech debt first to establish cross-language crypto confidence, Phases 17+18 expand the connector ecosystem in parallel, Phase 19 integrates MCP, and Phase 20 wires iOS discovery and memory consent — delivering a "universal agent client" claim that is both technically accurate and security-honest.

---
*Research completed: 2026-04-18*
*Ready for roadmap: yes*
