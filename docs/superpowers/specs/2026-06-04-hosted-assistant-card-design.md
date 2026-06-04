# Hosted Assistant Card Publishing — Design (v1)

**Date:** 2026-06-04
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Context:** The Agent Discovery sub-project shipped per-account saved agents and routing, and a
guard so the client-facing agent-card route 404s unknown names. But the hosted assistant
(`assistant@<host>`) still has **no published AgentCard**: `HostedAgent` carries only
`address`/`key`/`system_prompt`/`enabled`, and in `--mode human` `state.agent_card` is `null`. So
the iOS app's seeded Assistant renders a synthesized **placeholder** card, never a real one.
Compounding this, there is a **client/spec casing mismatch**: the canonical schema
(`schemas/agent-card.schema.json`) and the server's existing cards use **camelCase**
(`pushNotifications`, `protocolVersion`, `contentTypes`, `subThreads`, `archivalSupported`, …),
but the iOS `AgentCard` decoder uses **snake_case** CodingKeys — so a schema-conformant card would
fail to decode on iOS today. This sub-project publishes a real assistant card AND reconciles the
casing so iOS renders it.

## 1. Goal & scope

Serve a real, schema-conformant `AgentCard` for the hosted assistant at
`/.well-known/ainm/assistant/agent-card.json`, and make the iOS client decode the canonical
(camelCase) shape, so the app shows the real Assistant card (name, capability badges, skills)
instead of the placeholder.

**Decisions (locked in brainstorming):**
- **Scope = hosted assistant only.** Connector-agent card publishing is a separate future
  sub-project (it needs a TS connector-SDK change to actually send `name`/`description`/`skills`
  at handshake, server storage of the real card, and a live connector to validate).
- **Card source = static builder in code.** A pure server fn produces the canonical card from the
  agent's real runtime capabilities. No DB column, no config surface (the assistant is a single
  fixed first-party agent).
- **Casing = Approach A: schema-conformant card + fix the iOS decoder.** The published JSON Schema
  (camelCase, A2A-compatible) is authoritative. The iOS decoder is corrected to the canonical
  camelCase; the snake_case scenario fixture + one test cardJSON are updated to match.

**Out of scope (v1):** connector-agent card publishing; federation listener parity
(`fed_agent_card_handler` in `main.rs` still serves the static card under any name —
server-to-server surface, tracked separately); DB-stored/configurable cards; enumerating the
assistant's live MCP tools in the card (`tools: []` for v1).

## 2. Architecture & components

```
Server (Rust):
  src/agent_card.rs            (new)    pub fn build_assistant_card(address: &str) -> serde_json::Value
                                        — canonical camelCase, schema-valid AgentCard for the assistant
  src/server.rs                (modify) published_card_for(): add a hosted-assistant branch that
                                        returns build_assistant_card(...) when agent_name matches the
                                        provisioned hosted agent's local-part
  tests/agent_card_route.rs    (modify) extend with the assistant-card assertion (DB-or-skip)
  src/agent_card.rs (#[cfg(test)])      unit test: build_assistant_card is schema-conformant

iOS (Swift):
  Models/AgentCard.swift                 (modify) drop snake_case CodingKeys → canonical camelCase
  Testing/Fixtures/scenario-03-*.json    (modify) card JSON → camelCase
  AINMMessengerTests/.../SavedAgentStoreTests.swift (modify) minimal cardJSON → camelCase
  AINMMessengerTests/.../AgentCardDecodeTests.swift  (new) decode a canonical camelCase card
```

## 3. Server: `build_assistant_card`

A pure function (no DB, no async) producing the canonical card. Signature:
`pub fn build_assistant_card(address: &str) -> serde_json::Value` where `address` is the hosted
agent's AINM address (`assistant@<host>`). The card MUST validate against
`schemas/agent-card.schema.json` (camelCase keys throughout):

- Top-level: `name: "Assistant"`, `description` (e.g. "Your home server's built-in AI assistant."),
  `url` (derived from the address host, `https://<host>`), `version` (e.g. "0.1.0"),
  `protocolVersion: "0.1.0"`, `capabilities: { streaming: true, pushNotifications: false }`,
  `skills: []` (a representative skill list is acceptable but not required for v1).
- `ainm`: `{ protocolVersion: "0.1.0", address, type: "agent",
  contentTypes: ["text", "code"], streaming: { supported: true, parallelBlocks: false,
  preferredChunkSize: "token" }, threading: { subThreads: true, maxConcurrentThreads: null },
  memory: { archivalSupported: true, contextFramePreferred: true }, tools: [] }`.

Capabilities reflect what the hosted runtime actually supports (streaming, archival memory +
context frames, sub-threads). `tools: []` for v1 — enumerating the live MCP toolset in the card is
a future enhancement (documented limitation).

## 4. Server: route resolution

Extend `published_card_for(state, agent_name)` (the helper added by the issue-#2 guard) with a
hosted-assistant branch, checked BEFORE the existing static-card identity match:

1. **Hosted assistant:** if `state.hosted_agent` is `Some(agent)` and the local-part of
   `agent.address` equals `agent_name` → return `Some(build_assistant_card(&agent.address))`.
2. **Static card** (existing): serve `state.agent_card` iff its declared `address`/`ainm.address`
   local-part equals `agent_name`.
3. Else `None` → 404 (existing behavior).

So `/.well-known/ainm/assistant/agent-card.json` resolves whenever an assistant is provisioned
(production iOS server has a DB → `hosted_agent` is `Some`); unknown names still 404. In
`--mode human` with no provisioned agent (`hosted_agent: None`), `/assistant` 404s — the iOS app
then keeps the placeholder, which is correct (no assistant exists to describe).

## 5. iOS: decoder reconciliation

`Models/AgentCard.swift` currently maps several fields to snake_case via `CodingKeys`. The
canonical schema is camelCase, which equals the Swift property names — so the fix is to **remove
the snake_case `CodingKeys`** (and nested-type CodingKeys) so Swift's synthesized coding uses the
property names directly. Affected keys: `protocolVersion`, `capabilities.pushNotifications`,
`ainm.protocolVersion`, `ainm.contentTypes`, `streaming.parallelBlocks`,
`streaming.preferredChunkSize`, `threading.subThreads`, `threading.maxConcurrentThreads`,
`memory.archivalSupported`, `memory.contextFramePreferred`, `tools.inputSchema`.

Because `AgentCard` is both decoded (from the server / cached `cardJson`) and encoded (in
`AppState.saveAgent` → stored `cardJson`), the change is symmetric: stored snapshots become
camelCase and round-trip cleanly. The seeded placeholder card (built via the memberwise init) also
encodes camelCase.

**Fixtures/tests updated to camelCase (so they keep decoding):**
- `Testing/Fixtures/scenario-03-agent-discovery.json` — the two embedded card JSON strings
  (`agentCards` + `savedAgents`).
- `SavedAgentStoreTests.testRecordPersistsAndDecodesCard` — the inline minimal cardJSON.

## 6. Data flow

iOS `seedDefaultAssistantIfNeeded` / `resolveAgent("assistant@host")`
→ `GET <scheme>://<host>:<port>/.well-known/ainm/assistant/agent-card.json`
→ server `published_card_for` matches the provisioned hosted agent
→ `build_assistant_card` → canonical camelCase JSON (200)
→ iOS `AgentCard` decodes → saved as the default contact and rendered (name, capability badges,
skills) in `AgentDiscoveryView` / `AgentCardDetailView`.

## 7. Error handling

- No provisioned assistant (`hosted_agent: None`) → `/assistant` 404 → iOS falls back to the
  placeholder card (unchanged behavior; the app still functions).
- Network/timeout/undecodable → `resolveAgent` returns `.unreachable` / seed uses placeholder
  (existing behavior).
- A card that fails to validate against the schema is a build/test failure, caught by the
  `build_assistant_card` unit test — never shipped.

## 8. Testing

- **Server unit:** `build_assistant_card` returns a card that validates against
  `schemas/agent-card.schema.json` (use the `jsonschema` crate already in the workspace; load the
  schema from the repo path) — or, if loading the schema file in a unit test is awkward, assert the
  required camelCase keys + values explicitly (`name == "Assistant"`,
  `capabilities.pushNotifications` present, `ainm.memory.archivalSupported == true`,
  `ainm.address == address`). Prefer real schema validation.
- **Server integration:** extend `tests/agent_card_route.rs` — boot the binary with a DB so the
  assistant is provisioned (DB-or-skip, mirroring `ws_conversation_e2e.rs`), GET
  `/.well-known/ainm/assistant/agent-card.json`, assert 200 + `name == "Assistant"` + a camelCase
  key (`capabilities.pushNotifications`). Keep the existing unknown-name → 404 assertion.
- **iOS unit:** a new `AgentCardDecodeTests` decodes a canonical camelCase card JSON and asserts
  the nested fields (`capabilities.pushNotifications`, `ainm.memory.archivalSupported`,
  `ainm.streaming.parallelBlocks`) decode correctly. Existing fixture/seed tests pass with the
  updated camelCase JSON.
- **iOS UITest:** `AgentDiscoveryUITests` stays green (scenario-03 fixture updated to camelCase).
- **Manual live (human):** in the app against a DB-backed server, the seeded Assistant shows the
  real card (capability badges / description), not the placeholder.

## 9. Definition of Done

1. `build_assistant_card` produces a schema-conformant camelCase `AgentCard`; unit-tested
   (schema-validated).
2. `/.well-known/ainm/assistant/agent-card.json` returns the assistant card (200) when an assistant
   is provisioned; unknown names still 404.
3. iOS `AgentCard` decodes the canonical camelCase shape; scenario-03 fixture + `SavedAgentStoreTests`
   cardJSON updated; new decode test green; full iOS unit suite + `AgentDiscoveryUITests` green.
4. Server suite green except the two known pre-existing failures (`test_graph_traversal`,
   `test_compilation_dedup_updates_confidence`).
5. Live: the seeded Assistant renders a real card (human-verified).
6. Documented limitations: connector-agent cards, federation-listener parity, DB-configurable
   cards, and live-tool enumeration remain future.

## 10. Build order

`build_assistant_card` + its schema-validation unit test → wire into `published_card_for` + extend
the route integration test → iOS `AgentCard` camelCase decoder + fixture/test updates + decode test
→ full suites green → manual live. The decoder + builder are independent and could land in either
order, but the builder-first order lets the iOS decode test use the exact server card shape.
