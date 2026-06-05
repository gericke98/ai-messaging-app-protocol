# Connector-Agent Card Publishing — Design (v1)

**Date:** 2026-06-05
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Context:** The hosted-assistant-card sub-project (shipped 2026-06-05) gave the hosted
`assistant@<host>` a real, schema-conformant AgentCard at the client-facing well-known route, and
reconciled the iOS decoder to canonical camelCase. But **live connector-connected agents** (Claude
Code, OpenAI, etc., via the TypeScript connector SDK) still have no real card: at
`handle_agent_connection` the server stores only a minimal `{identity, capabilities, connected_at}`
blob in `AgentRegistry`, and the connector handshake (`packages/connector/src/protocol/handshake.ts`)
accepts `name`/`skills` in `HandshakeOptions` but **never puts them into the `ainm.initialize`
params**. So when the iOS app tries to resolve a connector agent by address, the well-known route
returns nothing usable. This sub-project makes a connected connector agent publish a real card.

## 1. Goal & scope

A live connector agent publishes a real, schema-conformant `AgentCard` at
`/.well-known/ainm/{name}/agent-card.json` **while it is connected**, so the iOS app resolves and
renders it like any other agent (name, description, skills, capability badges, tools).

**Decisions (locked in brainstorming):**
- **Card assembled server-side.** The connector sends structured fields in the handshake; the
  server assembles the canonical camelCase `AgentCard` (one source of card-shape truth, in Rust,
  mirroring `build_assistant_card`). The connector does NOT build full card JSON.
- **Tools included.** The connector sends each registered tool's `{name, description, parameters}`;
  the server maps `parameters` → `inputSchema` **best-effort** (accept JSON-object schemas, drop
  malformed ones, never fail the handshake). Real tool capabilities are the point of "real cards".
- **Ephemeral.** The card is served only while the agent is connected; the route 404s once the
  agent's last connection drops. Truthful about availability; matches the existing
  `AgentRegistry::cleanup` behavior. No new storage.
- **`description`/`version` source:** add optional `description?` + `version?` to `AINMAgentConfig`;
  the server defaults when absent (`description` → `"{name} agent"`, `version` → `"0.1.0"`) so the
  schema's `minLength: 1` on those fields is always satisfied.

**Out of scope (v1):** `fed_agent_card_handler` parity (the federation mTLS listener still serves
the static card under any name — a server-to-server surface, tracked separately); persisting
connector cards across disconnects; DB-configurable cards; iOS UI changes beyond what the existing
`AgentDiscoveryView` / `AgentCardDetailView` already render.

## 2. Architecture & components

```
Connector SDK (TypeScript, packages/connector):
  src/protocol/types.ts        (modify) AINMAgentConfig: add description?, version?
  src/agent.ts                 (modify) collect registered tools; pass card fields to handshake
  src/protocol/handshake.ts    (modify) HandshakeOptions + performHandshake: emit params.card
  test (TS)                    (new/modify) assert performHandshake emits params.card

Server (Rust, server/src):
  agent_card.rs                (modify) add build_connector_card(address, init_params) -> Value
  server.rs                    (modify) handle_agent_connection: register build_connector_card(...)
                                        published_card_for: add AgentRegistry lookup branch
                                        AgentRegistry::get_agent_card: drop #[allow(dead_code)]
  tests/                       (modify/new) build_connector_card unit tests; WS-agent route
                                        integration test (DB-or-skip): connected → 200, dropped → 404

iOS: no change (decoder already canonical camelCase from the hosted-assistant sub-project).
```

## 3. Connector SDK (`packages/connector`)

- **`AINMAgentConfig`** (`src/protocol/types.ts`): add `description?: string` and `version?: string`.
  `name` (required), `capabilities?` (content types), and `skills?: Array<{name, description}>`
  already exist.
- **`agent.ts`**: it already holds `tools: Map<string, { definition: ToolDefinition; handler }>`.
  Build a serializable tool list `[{ name, description, parameters }]` from that map (the
  `ToolDefinition` carries `description` and `parameters`). Pass `description`, `version`, `skills`,
  and the tool list into `performHandshake` (alongside the existing `name`, `capabilities`).
- **`handshake.ts`**: extend `HandshakeOptions` with `description?`, `version?`, and
  `tools?: Array<{ name: string; description: string; parameters: Record<string, unknown> }>`.
  In `performHandshake`, add a **`card`** object to the `ainm.initialize` params:
  ```
  params.card = {
    name, description?, version?,
    skills: options.skills ?? [],
    tools: options.tools ?? [],
  }
  ```
  The existing `params.capabilities` object is unchanged (the server still reads
  `participant_type`/`contentTypes`/`streaming`/`subThreads` from it). **Backward-compatible:**
  omitting `card` means the server builds the card from `capabilities` + defaults only.

## 4. Server: `build_connector_card`

A pure function in `agent_card.rs` beside `build_assistant_card`:
`pub fn build_connector_card(address: &str, init_params: &serde_json::Value) -> serde_json::Value`.
It assembles the canonical **camelCase** `AgentCard` (validating against
`schemas/agent-card.schema.json` shape):

- **Top-level:** `name` (from `card.name`, else local-part of `address`), `description` (from
  `card.description`, else `"{name} agent"`), `url` (`https://<host>` from the address host),
  `version` (from `card.version`, else `"0.1.0"`), `protocolVersion: "0.1.0"`,
  `capabilities: { streaming, pushNotifications: false }` (`streaming` from
  `capabilities.streaming`, default true), `skills` (from `card.skills`, each normalized to a valid
  `AgentSkill` — `id` derived from a slug of `name` if absent, since the schema requires
  `id`/`name`/`description`; default `[]`).
- **`ainm`:** `{ protocolVersion: "0.1.0", address, type: "agent", contentTypes (from
  capabilities.contentTypes, default ["text"]), streaming: { supported, parallelBlocks: false,
  preferredChunkSize: "token" }, threading: { subThreads (from capabilities.subThreads, default
  false), maxConcurrentThreads: null }, memory: { archivalSupported: false, contextFramePreferred:
  false }, tools }`.
- **Tools mapping (best-effort):** for each `card.tools[]` entry, emit
  `{ name, description, inputSchema }` where `inputSchema` is the entry's `parameters` **iff** it is
  a JSON object (the schema requires `inputSchema` to be an object). If `parameters` is missing or
  not an object, **drop that tool** and log at `warn`. Never panic; never fail the handshake. A tool
  missing `name` or `description` is also dropped. Result is always a valid (possibly empty) array.

`memory` defaults to `false/false` for connector agents (unlike the hosted assistant, a generic
connector is not assumed to support archival memory / context frames). `skills`/`tools` empty arrays
are valid per schema.

## 5. Server: registration + route resolution

- **`handle_agent_connection`** (`server.rs`): replace the minimal blob
  ```
  { "identity": from_addr, "capabilities": capabilities, "connected_at": now }
  ```
  with `build_connector_card(&from_addr, params)` where `params` is `init_req["params"]`. Register
  that card in `AgentRegistry` via the existing `register(&from_addr, &card)`. The ephemeral
  lifecycle is unchanged: `AgentRegistry::cleanup` already removes the card when the agent's last
  connection drops.
- **`published_card_for`** (`server.rs`): add a **third branch**, after the hosted-assistant branch
  and after the static-card branch, before returning `None`:
  ```
  // (3) Live connector agent: serve its registered card while connected.
  let addr = format!("{agent_name}@{}", state.server_domain);
  if let Some(card) = state.agent_registry.get_agent_card(&addr).await { return Some(card); }
  None
  ```
  This requires `published_card_for` (and `agent_card_handler`) to be `async` (the registry lock is
  async) — currently `published_card_for` is sync. Make it `async fn` and `.await` it in the handler
  (the handler is already async). Drop the `#[allow(dead_code)]` on `AgentRegistry::get_agent_card`.
  Unknown name or offline agent → no registry entry → `None` → 404 (unchanged contract).

  **Address reconstruction:** the registry is keyed by the full handshake `identity`
  (`name@host`). The well-known route only carries the local-part `{agent_name}`, so the lookup
  reconstructs `"{agent_name}@{server_domain}"`. This matches how connector agents register on their
  home server (the `identity` they hand-shake with is `name@<that-server>`).

## 6. Data flow

connector `agent.start()` → signed `ainm.initialize` with `params.card` → server verifies the
account-key binding (existing anti-spoof gate) → on `is_agent`, `handle_agent_connection` →
`build_connector_card` → `AgentRegistry.register(name@host, card)` →
`GET /.well-known/ainm/{name}/agent-card.json` → `published_card_for` branch (3) → canonical card
(200) → iOS `AgentCard` decodes & renders (name, description, skills, capability badges, tools). On
disconnect → `AgentRegistry.cleanup` removes the card → route 404s → iOS treats as `unreachable`; a
saved contact falls back to its cached `cardJson` snapshot.

## 7. Error handling

- **No `card` in params** (older connector / non-connector agent): build from `capabilities` +
  defaults; still a valid card (name = local-part, description = `"{name} agent"`).
- **Malformed tool `parameters`** (not a JSON object): that tool is dropped (logged `warn`); the
  card is still served with the remaining valid tools.
- **Skill missing `id`:** derive a slug from the skill `name` so the schema's required `id` is
  satisfied; a skill missing `name`/`description` is dropped.
- **Agent offline / unknown name:** registry has no entry → 404 → iOS `unreachable` (saved contact
  shows cached snapshot).
- **Card fails schema shape:** caught by the `build_connector_card` unit test (validated by explicit
  key/value assertions, matching the hosted-assistant approach since `jsonschema` is not a server
  dependency and the schema has an external `$ref`).

## 8. Testing

- **Connector (TS):** unit test that `performHandshake` puts `name`, `description`, `version`,
  `skills`, and `tools` into `params.card` of the emitted `ainm.initialize` request. A test that
  `agent.ts` collects registered tools into the serialized tool list.
- **Server unit (`agent_card.rs`):** `build_connector_card` —
  (a) full params → canonical camelCase card with name/description/version/skills/tools mapped;
  (b) malformed tool `parameters` (string, not object) → that tool dropped, others kept;
  (c) no `card` object → built from capabilities + defaults (name = local-part, description
  `"{name} agent"`, `tools: []`); (d) skill without `id` → id slugged from name.
- **Server integration (`server.rs` / tests):** DB-or-skip — boot the binary, connect a WS client
  as an agent (signed handshake, `participant_type: "agent"`, with a `card` carrying a tool), GET
  `/.well-known/ainm/<name>/agent-card.json` → 200 + `name` + the tool present; then close the WS
  connection, wait for cleanup, GET again → 404. (Mirrors the `ws_conversation_e2e.rs` harness for
  the signed agent handshake.)
- **iOS:** no change required; the existing `AgentCardDecodeTests` already cover decoding this exact
  canonical shape (tools included).
- **Manual live (human):** run a real connector (Claude Code or OpenAI) against a local server, add
  it by address in the iOS app, confirm its real card + tools render; stop the connector and confirm
  the app reports it unreachable / falls back to the cached snapshot.

## 9. Definition of Done

1. The connector SDK sends a `card` object (name, description, version, skills, tools) in
   `ainm.initialize`; unit-tested.
2. `build_connector_card` produces a schema-conformant camelCase card with best-effort tool mapping;
   unit-tested (incl. malformed-tool and no-card-object cases).
3. A connected connector agent's card is served at `/.well-known/ainm/{name}/agent-card.json` (200);
   after disconnect the route 404s; locked by an integration test (DB-or-skip).
4. `published_card_for` is `async` and consults the `AgentRegistry` as branch (3); hosted-assistant
   and static branches and the unknown-name → 404 contract are preserved (existing
   `agent_card_route` tests still green).
5. Server suite green except the two known pre-existing failures (`test_graph_traversal`,
   `test_compilation_dedup_updates_confidence`); connector package tests green.
6. Live: a real connector agent renders a real card (with tools) in the iOS app (human-verified).
7. Documented limitations: `fed_agent_card_handler` parity, persisted/offline cards,
   DB-configurable cards remain future.

## 10. Build order

Connector SDK (types + handshake `params.card` + agent tool collection) + TS test →
`build_connector_card` + unit tests → wire into `handle_agent_connection` → make `published_card_for`
async + registry branch + drop dead_code → route integration test → full suites green → manual live.
The server builder can land before or after the connector change (they meet at the `params.card`
contract); builder-first lets the integration test assert the exact shape.
