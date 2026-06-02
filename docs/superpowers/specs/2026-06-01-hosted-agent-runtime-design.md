# Hosted Agent Runtime — Design (sub-project B, v1)

**Date:** 2026-06-01
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Milestone context:** Sub-project B of the hosted multi-tenant runtime initiative. Decomposition:
A = Accounts & identity binding (shipped) → **B = Hosted agent runtime (this doc, the moat)** →
C = Per-user agent credentials & quota → D = Isolation/sandboxing → E = Deployment/HA → F = Onboarding UX.

## 1. Goal

Make the server **run a genuine, multi-step, tool-calling agent loop in-process** so that AINM hosts the
agent runtime itself — not an external connector on someone's laptop. Because we run the loop, **every
memory read and every tool call passes through our consent gate before it happens**: this is the moat
(post-handoff consent enforcement is only real when we control execution). v1 ships **one built-in
first-party agent** (`assistant@<server-domain>`) that any user can message; it runs a real LLM tool-use
loop with **compound-memory tools + a bridge to a curated set of MCP servers**, and it only ever sees the
**consent-scoped** memory and tools the requester has granted.

## 2. What exists today / the gap

(From a codebase survey, file:line evidence.)

- **Agents are EXTERNAL WebSocket connectors.** `packages/connector` (TS SDK `AINMAgent`), `packages/connector-openai`
  (calls OpenAI chat/completions), `packages/connector-claude-code` (spawns the `claude` CLI). They run off-server,
  bring their own LLM credentials, connect via `/ws`, receive `ainm.message.deliver`, and stream replies back via
  `ainm.message.send`. The server runs **no agent logic**.
- **`AgentRegistry`** (`server/src/server.rs`) is **in-memory/ephemeral** (HashMap of identity → connection senders +
  agent cards); lost on restart.
- **The OpenAI connector is a chat wrapper** (single streaming completion, no tool loop). The Claude Code connector is a
  real tool-using runtime but is an external subprocess. Neither runs in our infra.
- **The server's LLM (`llm.rs`, `AINM_LLM__*`) is global and used only by the memory compiler** (`extract`), not by agents.
  `LlmClient` is single-shot JSON (`response_format: json_object`) — **no tool-use/function-calling support yet**.
- **Post-sub-project-A, agents cannot connect to a DB-backed (enforced) server.** The handshake now requires
  `find_account_by_address(from_addr)` → active account with a registered key (`server.rs` handshake gate). Connectors
  generate their own keypair and claim `helper@host` with no account → **rejected**. So a hosted/first-party agent needs a
  real provisioned account + key.
- **Consent scoping exists** (sub-project A): `generate_briefing` enforces resolved per-agent consent scopes; `scope_to_categories`
  maps scopes → memory categories. B reuses this as the enforcement spine.
- **Reusable:** `streaming` (streamed responses), `graph`/`briefing`/`repository` (memory ops), `accounts`/`account_keys`
  (identity), `llm.rs` HTTP client + `MockLlmClient` pattern, the message-delivery path.

**The gap B closes:** an in-process, consent-gated, tool-calling agent runtime owned by the server, with a real
first-party agent identity — turning "we host the runtime" from aspiration into the literal enforcement boundary.

## 3. Decisions locked (brainstorming)

- **Host the agent loop server-side** (the moat) — chosen over agent-identity-only or a thin hosted default.
- **Full multi-step tool-calling agentic loop** (not a chat/completions wrapper) — honors the standing principle
  "agents are real runtimes with tools, not chat-completions wrappers."
- **One built-in first-party agent, consent-scoped per user** (not per-user instances or user-creatable agents yet).
- **Tools = compound-memory ops + an MCP bridge to a CURATED/trusted set** of MCP servers (config-defined). Arbitrary /
  user-supplied MCP servers + hard sandboxing → sub-project D.
- **Architecture = Rust-native, in-process** in `ainm-server` (chosen over a supervised TS connector process or a hybrid
  MCP-exec service): tightest consent enforcement, lowest latency, single deployable. The MCP client sits **behind a
  `ToolProvider` trait** so D can later move it into an isolated sandbox without touching the loop.
- **LLM key = the server's own** (`AINM_LLM__*`); per-user keys + quotas are sub-project C.
- **Agent identity = a server-held Ed25519 keypair + auto-provisioned account** (we legitimately hold this key — it's our
  first-party agent, distinct from the zero-knowledge human key escrow).

## 4. Identity & provisioning

- Address: `assistant@<server-domain>` (server-domain = the server's configured domain / `server_id`, matching the address
  humans' keys are bound to).
- **Startup provisioning (idempotent):** ensure (a) an `accounts` row `username='assistant'`, `status='active'`,
  `email_verified=true` (no email flow — it's server-owned); (b) an `account_keys` row binding a **server-held Ed25519
  keypair**; (c) a `hosted_agents` row holding that keypair's private bytes + system prompt + `enabled`. Stable across
  restarts (persisted), so the @handle and public key are stable. The agent is thus a fully handshake-valid participant and
  its outbound messages are signed with its key (reusing `signing`).
- The agent does **not** open a WebSocket to itself — it's invoked in-process (§5). The keypair signs its outbound message
  envelopes exactly as an external agent would.

## 5. Message routing → runtime invocation

- The built-in agent is an addressable participant. v1 focus: a **human ↔ `assistant` thread** (and `@assistant` mentions in
  any thread). When a message is delivered to a thread the agent participates in, the delivery path **spawns the runtime**
  (`runtime::handle_turn`) with: the inbound message, the thread's recent history, the **requester's identity** (the human —
  used for consent scoping), and the thread id.
- The runtime streams the agent's reply through the **existing** `streaming` + `ainm.message.deliver` path (start → deltas →
  end + final `ainm.message.send` envelope signed by the agent key). To the human it is indistinguishable from any agent.
- No change to the human client is required; mixed human-agent threads continue to work — the hosted agent is just another
  participant whose production happens in-process.

## 6. The agentic loop (`runtime.rs`)

Per turn:
1. **Assemble context:** system prompt (from `hosted_agents`) + the **consent-scoped briefing** for the requester
   (`generate_briefing`, already scope-enforced) + recent thread history + the **consent-allowed tool schemas** (§7).
2. **Call the LLM with tools** (`llm.rs::chat_with_tools`, streaming, OpenAI-compatible function-calling). If the model emits
   assistant text, stream deltas to the human. If it emits tool calls, go to 3.
3. **Execute tool calls** (consent-gated, §7), append each result to the running messages, and **loop back to 2**.
4. **Termination:** the model returns a final assistant message with no tool calls → finalize and send. Hard caps:
   `max_tool_rounds` (e.g. 8) and a total token/time budget; on cap, finalize with a brief "stopped early" note rather than
   looping forever.

Provider is config-driven via the existing `AINM_LLM__*` (base_url lets it target OpenAI or Anthropic-via-gateway). Tool-use
uses the OpenAI-compatible `tools`/`tool_calls` shape.

## 7. Tools & the consent gate (the enforcement surface — the moat)

A `ToolProvider` trait exposes: `list_tools() -> Vec<ToolSchema>` and `call(name, args) -> ToolResult`. Two implementations:

- **Memory tools** (in-process; direct calls to `graph`/`briefing`/`repository`): `retrieve_briefing`, `query_memory`,
  `store_fact`. Results are **scope-filtered** — categories the requester has not granted are never returned (reusing
  `scope_to_categories` + the per-agent resolved scopes).
- **MCP tools** (`mcp.rs`, `rmcp` client): connects to the **curated MCP servers from config**, lists their tools, and invokes
  them. Each curated server declares a `required_capability`.

**Two consent gates, both keyed on the requester's resolved consent scopes (sub-project A):**
1. **Memory visibility:** the injected briefing and every memory-tool result are scope-filtered (an ungranted category is
   invisible and unqueryable).
2. **Tool allow-list:** the tool set *offered to the LLM* is filtered to consent-allowed tools — each tool / MCP server maps to
   a required scope/capability; a tool the requester hasn't granted is **never offered**, and if the model somehow names a
   disallowed tool it is **refused with a tool-error** (never silently executed).

The MCP client is behind the `ToolProvider` trait specifically so sub-project D can replace it with a sandboxed executor with
no change to the loop or the gates.

## 8. Data model & config

- **`hosted_agents`** (new table): `id`, `address` (unique), `public_key`, `private_key` BYTEA (server-held),
  `system_prompt`, `enabled` BOOL, `created_at`, `updated_at`. v1 holds exactly one row (`assistant@…`).
- **Curated MCP servers**: defined in **server config** (TOML + `AINM_*` env), a list of
  `{ name, transport: stdio|http, command|url, required_capability }`. Not user-editable in v1.
- **Tool-call audit**: structured `tracing` logs in v1 (tool name, allowed/denied, requester, latency). A dedicated audit
  table is a later follow-up (useful for the consent-receipt story but not required to prove the moat).
- **Reuses** (no schema change): `accounts`, `account_keys`, the consent-scope machinery, the memory graph, `streaming`.

## 9. Module boundaries

- **`runtime.rs`** (new) — the loop: turn assembly, the LLM↔tool iteration, streaming, caps/budget. One public entry
  `handle_turn(state, thread_id, requester, message)`.
- **`tools.rs`** (new) — `Tool`/`ToolProvider` trait, the registry, **consent gating of the offered tool set**, LLM
  tool-schema (de)serialization, and the in-process **memory-tool** implementation.
- **`mcp.rs`** (new) — the `rmcp`-based MCP client implementing `ToolProvider` for the curated servers (the D-isolation seam).
- **`hosted_agent.rs`** (new) — idempotent startup provisioning of the agent account/key/config; the signer for outbound
  agent messages.
- **`llm.rs`** (extend) — add `chat_with_tools(...)` to `LlmClient` returning streamed text deltas and/or tool-call requests;
  extend `MockLlmClient` to script multi-turn tool use.
- **`server.rs`** (extend) — invoke `runtime::handle_turn` from the delivery path when the built-in agent is a participant;
  register the agent as a participant; thread the requester identity through.
- **`main.rs`** (extend) — call `hosted_agent` provisioning at startup; build the `ToolProvider`s (memory + MCP) from config
  and inject into `ServerState`.

## 10. Error handling

- **LLM failure** → stream a clear error message and end the turn cleanly.
- **Tool error / MCP server unreachable** → return the error as a tool-result so the model can recover within the cap; log it.
- **Consent denial** → the tool is not offered; a disallowed call is refused with a tool-error result — never silently run.
- **Loop runaway** → hard `max_tool_rounds` + total token/time budget; finalize with a "stopped early" note.
- **Crash/disconnect mid-stream** → finalize the partial turn gracefully (end the stream); no half-written state.
- **No DB / agent not provisioned** → the hosted agent is simply absent (the runtime is not invoked); the server still routes
  human/external-agent traffic as today.

## 11. Testing

- **Unit (Rust):** tool-schema generation; **consent gating** — e.g. a `Minimal`-scope requester is never offered `store_fact`
  and a curated MCP tool mapped to an ungranted capability is absent from the offered set; cap/budget enforcement;
  agent-message signing.
- **`MockLlmClient` (extended):** scripts multi-turn tool use (turn 1 → emit a tool call; turn 2 → final text), so the whole
  loop is exercised with no real LLM. **`MockToolProvider`** returns canned MCP results (no real MCP server in tests).
- **Integration (DB-backed, `server/tests/`):**
  1. Idempotent **agent bootstrap**: startup twice → exactly one `assistant` account + key + `hosted_agents` row.
  2. **Full hosted turn**: human message → runtime → a consent-scoped memory tool → streamed reply delivered + signed by the
     agent key; assert the agent saw only scope-allowed memory.
  3. **Consent gate**: a requester whose scopes exclude a category/tool → that memory is absent and that tool is never offered
     (and a forced disallowed call is refused).
  4. **MCP path** via `MockToolProvider`: the loop calls a tool, gets a result, and incorporates it into the final reply.
  5. **Cap**: a mock LLM that always emits a tool call → the loop stops at `max_tool_rounds` and finalizes.

## 12. Risks

- **Rust MCP client (`rmcp`) maturity.** The official Rust MCP SDK is less battle-tested than the TS/Python SDKs. Mitigation:
  the `ToolProvider` trait isolates it (fallback: a thin MCP sidecar process speaking a minimal internal protocol), and v1
  uses only curated/trusted servers (no untrusted-input hardening needed yet). Verify `rmcp` client capability (stdio + HTTP
  transports, tool list/call) during plan-phase research; if inadequate, implement the sidecar fallback behind the same trait.
- **LLM tool-use wiring in Rust.** `llm.rs` is currently single-shot JSON. Adding a streaming tool-use loop is real work;
  the `MockLlmClient` scripting keeps it testable without burning tokens.
- **Server-held agent key.** Storing the agent's private key in `hosted_agents` is acceptable (first-party, we host it) but is
  a secret at rest — keep it out of logs; a later hardening pass can move it to a KMS/enclave (aligns with D/E).
- **Consent correctness is the whole point.** The gate must filter the *offered* tool set and *every* result, not just the
  briefing — the tests above assert both directions (granted appears, ungranted never does).
- **Large slice.** The plan will wave it: (1) agent identity provisioning + `hosted_agents`; (2) `llm.rs` tool-use +
  `MockLlmClient` scripting; (3) the `tools.rs` registry + memory tools + consent gating; (4) the `runtime.rs` loop + wiring
  into delivery; (5) the `mcp.rs` curated-server bridge; (6) integration tests.

## 13. Out of scope (v1)

Per-user LLM credentials + quotas (sub-project C); arbitrary / user-supplied MCP servers + hard sandboxing/isolation
(sub-project D); user-created or multiple hosted agents, BotFather-style instant agent tokens (later); HA / horizontal scaling
/ process isolation (sub-project E); onboarding UX (sub-project F); a persisted tool-call audit table (later follow-up);
agent-initiated (unprompted) messages.

## 14. Definition of done

1. Startup idempotently provisions the `assistant@<domain>` account + server-held key + `hosted_agents` row.
2. `llm.rs` supports a streaming tool-use loop; `MockLlmClient` can script multi-turn tool use.
3. `tools.rs` exposes memory tools through `ToolProvider`, with consent gating of both the offered tool set and tool results.
4. `mcp.rs` bridges the curated MCP servers through the same `ToolProvider` trait (verified against a mock in tests; against a
   real curated server manually).
5. `runtime.rs` runs the bounded tool-calling loop and is invoked from the delivery path; replies stream back signed by the
   agent key.
6. A human can hold a conversation with the hosted agent end-to-end, the agent uses a memory tool and (mock) MCP tool, and the
   consent gate provably hides ungranted memory and never offers ungranted tools (integration tests).
7. Full server suite green except the two known pre-existing failures (`test_graph_traversal`,
   `test_compilation_dedup_updates_confidence`).
