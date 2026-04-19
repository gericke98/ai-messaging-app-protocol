# Roadmap: AI-Native Messaging Protocol

## Milestones

- ✅ **v3.1 AI-Native Messaging Protocol v1 Specification & Demo** — Phases 1-6 (shipped 2026-03-29) — [Archive](milestones/v3.1-ROADMAP.md)
- ✅ **v3.2 iOS Reference Client** — Phases 10-14 (shipped 2026-04-19) — [Archive](milestones/v3.2-ROADMAP.md)
- 🚧 **v3.3 Universal Agent Client** — Phases 15-21 (started 2026-04-18)

## Phases

<details>
<summary>✅ v3.2 iOS Reference Client (Phases 10-14) — SHIPPED 2026-04-19</summary>

- [x] Phase 10: Foundation (5/5 plans) — completed 2026-03-30
- [x] Phase 11: Core Messaging UI (4/4 plans) — completed 2026-03-30
- [x] Phase 12: Streaming & Interactive Blocks (4/4 plans) — completed 2026-03-30
- [x] Phase 13: AI-Native Features & Platform (5/5 plans) — completed 2026-03-30
- [x] Phase 14: UAT Automation Harness (6/6 plans) — completed 2026-04-19

</details>

## Current Milestone: v3.3 Universal Agent Client (Phases 15-21) — IN PROGRESS

- [ ] **Phase 15: v3.2 Tech Debt + Off-GSD Ratification** — Close 13 v3.2 audit items and ratify 4 off-GSD connector packages against an 8-point checklist before new connector surface ships
- [ ] **Phase 16: Connector Auth Hardening + Delivery Gating** — Ship the `connector_keys` trust anchor, two-key identity split, signed challenge handshake, and server-side @-mention routing (keystone — blocks every subsequent connector phase)
- [ ] **Phase 17: Python Connector SDK** — Ship `ainm-connector` Python package with feature parity to `@ainm/connector`, cross-SDK signature conformance, and PyPI release
- [ ] **Phase 18: OpenAI Responses API Upgrade** — Migrate OpenAI connector from chat/completions to Responses API with 1:1 thread-to-response-chain mapping and hosted-tool visibility
- [ ] **Phase 19: MCP Bridge Connector** — Any MCP server becomes an AINM agent participant via one-bridge-per-MCP-server architecture with full `inputSchema` passthrough
- [ ] **Phase 20: iOS Live Agent Discovery + Compound Memory Groundwork** — Replace HTTP-snapshot Agent Discovery with live push-subscription; ship memory provenance + connector write channel + portability primitives for v3.4
- [ ] **Phase 21: Multi-Agent UX + Connector Lifecycle + Tool Transparency** — iOS surfaces that realize the "universal agent client" claim end-to-end: distinct agent identity, add/revoke flows, approve-before-run policy, grouped/truncated tool calls

### Phase 15: v3.2 Tech Debt + Off-GSD Ratification
**Goal**: v3.2 iOS HUMAN-UAT items are closed and off-GSD connector packages (`@ainm/connector`, `@ainm/connector-claude-code`, `@ainm/connector-openai`, server `AgentRegistry`) are production-hardened before any new connector surface ships
**Depends on**: Nothing (first phase of v3.3)
**Requirements**: DEBT-01, DEBT-02, DEBT-03, DEBT-04, DEBT-05, DEBT-06, DEBT-07, DEBT-08, DEBT-09, DEBT-10, DEBT-11, DEBT-12, DEBT-13, RAT-01, RAT-02, RAT-03, RAT-04, RAT-05, RAT-06
**Success Criteria** (what must be TRUE):
  1. iOS can complete a live WebSocket handshake against a running Rust server (ainm.initialize + ainm.initialized) with cross-platform Ed25519 + RFC 8785 JCS signatures verifying bidirectionally (Swift ↔ Rust)
  2. Ed25519 keypair persists across iOS app launches via Keychain with the correct accessibility class, and `make uat` is green end-to-end after each DEBT cluster lands
  3. All 4 off-GSD packages pass the 8-point ratification checklist (deps pinned, tests green, DECISIONS.md present, logs redacted, errors mapped, versions aligned with SDK parity, no blocking TODOs, CI integrated)
  4. `dist/` directories are excluded from git via `.gitignore` + CI enforcement, and `DeepLinkUITests` + `SubThreadBranchUITests` pass without XCTSkip (root-cause documented, not masked)
  5. A dependency map clusters the 13 DEBT items by shared surface (Keychain, offline, UAT timing) and every UAT cluster completes a full XCUITest regression run before moving to the next
**Plans**: TBD

### Phase 16: Connector Auth Hardening + Delivery Gating
**Goal**: Connectors prove identity at handshake with a server-bound challenge nonce signed by a separate-from-signing connector key, users enroll/revoke connectors via a `connector_keys` allowlist, and agents only receive messages they are @-mentioned in — establishing the trust anchor that every subsequent phase depends on
**Depends on**: Phase 15 (cross-language crypto confidence from UAT + ratified TS SDK as Python's parity target)
**Requirements**: AU-01, AU-02, AU-03, AU-04, AU-05, AU-06, AU-07, AU-08, MA-05
**Success Criteria** (what must be TRUE):
  1. A connector completing handshake without a valid Ed25519 signature over a server-issued nonce is rejected with a clear error code; replay of a captured handshake within or beyond the nonce TTL fails
  2. User can onboard a new connector by pasting an iOS-generated token into the connector CLI, completing an OAuth browser flow for hosted runtimes, or supplying a local-only API key that never transits the AINM server
  3. Connector identity uses a two-key split (`k_sign` stable + published in Agent Card; `k_conn` rotated ≤90 days + handshake-only) with the server-side `connector_keys` table enforcing TOFU-in-dev / strict-in-prod and supporting revocation
  4. User can revoke a connector from iOS Settings, which removes the `connector_keys` row and immediately disconnects the active session
  5. An agent in a multi-agent thread only receives messages it is @-mentioned in (or explicitly handed-off to) — blocks ANTI-01 runaway loops structurally, verified with a 3-agent fixture where un-mentioned agents see zero delivery events
  6. Reconnect storms trigger server 503 + Retry-After; connector SDKs honor it with exponential backoff + full jitter (no thundering herd on server restart)
**Plans**: TBD

### Phase 17: Python Connector SDK
**Goal**: Ship `ainm-connector` Python package with feature parity to `@ainm/connector` TypeScript SDK — same wire protocol, byte-identical signatures, same 5-line agent API, published to PyPI as the ecosystem-unlock for LangChain/CrewAI/LangGraph/custom-Python agents
**Depends on**: Phase 16 (signed-handshake contract + `connector_keys` enrollment are the protocol surface Python must match)
**Parallelizable with**: Phase 18
**Requirements**: PY-01, PY-02, PY-03, PY-04, PY-05, PY-06, PY-07, PY-08
**Success Criteria** (what must be TRUE):
  1. A 5-line Python example using `ainm-connector` connects to the Rust server, completes the signed handshake, and exchanges a streamed reply — identical ergonomics to the TS `@ainm/connector` hello-world
  2. A Python-signed envelope verifies byte-identically against both the TS SDK and the Rust server using the `rfc8785` 0.1.4 (Trail of Bits) library — conformance corpus (20+ Rust-generated vectors) runs in CI on every PR to either SDK
  3. Advanced API tier (streaming, memory push, tool calls, context frames) has parity with TS; E2E suite runs against the live Rust demo server with identical scenarios (send, stream, tool call, memory, handshake failures)
  4. Package is published to PyPI as `ainm-connector` with exact version pinning, Python 3.12+ floor (PEP 695 generics), typed (`py.typed`), and MIT-licensed
  5. WebSocket client handles ping/pong, close-code differentiation, and reconnect with jitter using `websockets` 16.0; HTTP Agent Card serves via `fastapi` + `uvicorn` with ETag-conditional GET
**Plans**: TBD

### Phase 18: OpenAI Responses API Upgrade
**Goal**: Migrate `@ainm/connector-openai` from chat/completions to the Responses API with stateful thread-to-response-chain mapping, tool-event translation for OpenAI-hosted tools, and record+replay regression protection — ahead of the 2026-08-26 Assistants API shutdown
**Depends on**: Phase 16 (hardened connector auth is the stable target for any connector upgrade)
**Parallelizable with**: Phase 17
**Requirements**: OA-01, OA-02, OA-03, OA-04, OA-05, OA-06
**Success Criteria** (what must be TRUE):
  1. OpenAI connector defaults to the Responses API; chat/completions is retained behind a per-agent feature flag as a fallback for one release cycle, enabling easy rollback
  2. Each AINM thread maps 1:1 to a `previous_response_id` chain; AINM sub-threads start fresh chains (fork-from-here semantics), and the thread→chain map survives connector restart
  3. Function-calling tool invocations surface as `ainm.tool.*` events attributed to the OpenAI connector; OpenAI-hosted tools (web_search, code_interpreter) surface with `openai:` prefix and the ApprovalBlock-cannot-gate-them limitation is documented
  4. Record+replay cassettes cover ≥6 Responses event shapes (text delta, function call args, tool output, error, completion, refusal) and execute in CI on every PR to the OpenAI connector
**Plans**: TBD

### Phase 19: MCP Bridge Connector
**Goal**: Any MCP server (filesystem, GitHub, Postgres, custom) becomes an AINM thread participant with identity `<server>@mcp.local`, tool invocations attribute to the MCP-agent (not "system"), and the user can @-mention capabilities naturally — one bridge process per MCP server
**Depends on**: Phase 16 (connector auth), benefits from Phase 17/18 (stabilized SDK patterns and stream semantics)
**Requirements**: MCP-01, MCP-02, MCP-03, MCP-04, MCP-05, MCP-06, MCP-07
**Success Criteria** (what must be TRUE):
  1. A running filesystem MCP server wrapped by the bridge appears in the iOS Agent Discovery view as a single agent participant with a synthesized Agent Card (one process per MCP server)
  2. When a user invokes an MCP tool, the tool indicator on iOS attributes to the MCP-agent identity (not "system" or the bridge) and the full MCP `inputSchema` is forwarded with local `ajv` validation before hitting the MCP server
  3. MCP Apps declarative UI (`_meta.ui.resourceUri`) renders inline in the AINM thread — mapped to AINM interactive blocks where possible, WKWebView fallback otherwise; per-tool consent gates surface as ApprovalBlock on first use
  4. MCP `tools/list_changed` notifications cause the bridge to re-fetch schemas and re-publish the Agent Card without restart
  5. Killing the bridge with SIGKILL leaves zero surviving MCP subprocesses (verified by a leak-detection test that spawns 50 MCP servers then SIGKILLs the bridge) — `detached: true` + process-group kill + SIGTERM→SIGKILL escalation works on Linux CI
**Plans**: TBD
**UI hint**: yes

### Phase 20: iOS Live Agent Discovery + Compound Memory Groundwork
**Goal**: Replace the v3.2 HTTP-snapshot Agent Discovery with a live push-subscription reflecting connected agents (via existing HumanRegistry broadcast), and lay the v3.4 compound-memory substrate — provenance, connector write channel, user preview, and export — without shipping the cross-agent read ACL (that's v3.4)
**Depends on**: Phase 16 (memory scope gating rides on `connector_keys`); most valuable after Phases 17/18/19 because richer directory data requires multiple concurrent connector types
**Requirements**: DISC-01, DISC-02, DISC-03, DISC-04, DISC-05, MEM-01, MEM-02, MEM-03, MEM-04, MEM-05
**Success Criteria** (what must be TRUE):
  1. iOS Agent Discovery shows currently-connected agents via `ainm.directory.list` snapshot and receives live `ainm.directory.updated` notifications on connect/disconnect — no rebuild or manual refresh required
  2. Snapshot-plus-deltas uses generation tokens: iOS performs a full resync on reconnect and rejects deltas with stale generation; subscription lifecycle is view-scoped (no leaks across view dismissal)
  3. Every memory archival fact carries provenance (`source_agent_id`, `source_thread_id`, `source_message_id`, `extracted_at`, `confidence`); connectors push facts via `ctx.memory.store(fact, scope)` with default scope `private`
  4. Connector-emitted memory writes surface on iOS as a "Claude wants to remember X" preview action block before persistence — user consent is mandatory, rejection is non-blocking
  5. User can export all archival facts as JSON from iOS Settings; memory scope enum ships with values `private / shared:thread / shared:user / public:user` (server accepts writes at all scopes, default read-filter is `private` — cross-agent read ACL lands in v3.4)
**Plans**: TBD
**UI hint**: yes

### Phase 21: Multi-Agent UX + Connector Lifecycle + Tool Transparency
**Goal**: Realize the "universal agent client" claim end-to-end on iOS — N agents in a thread have distinct identity and parallel indicators, tool calls attribute to the issuing agent and scale to 30+ per turn, users add/revoke connectors via a paired-devices Settings screen, and every tool can carry an approve-before-run policy
**Depends on**: Phase 16 (MA-05 server delivery gating), Phase 20 (live directory powers Add-from-Discovery), Phases 17/18/19 (connector ecosystem produces real tool-call volume to verify against)
**Requirements**: MA-01, MA-02, MA-03, MA-04, MA-06, MA-07, CN-01, CN-02, CN-03, CN-04, CN-05, TC-01, TC-02, TC-03, TC-04, TC-05
**Success Criteria** (what must be TRUE):
  1. In a 3-agent thread, each agent's messages render with a distinct avatar + display name + provider domain, consecutive-sender collapse is disabled for agent senders, and N concurrent typing/processing indicators render in parallel (not one collapsed indicator)
  2. Every tool call indicator shows the issuing agent (`issuer_agent_id`) with attribution consistent across TypeScript, Python, OpenAI, and MCP Bridge connector types
  3. User can tap "Add" on an Agent Card in Discovery, follow an `ainm://add-agent?card=…` deep link, or scan a QR code to pair a laptop connector — each route triggers the Phase 16 paste-token or OAuth flow
  4. iOS Settings shows a paired-connectors list (identity, last-seen, revoke action); revoking triggers the AU-07 server revocation and the connector disconnects within one WebSocket round-trip
  5. Tool calls remain visible by default at connector-emitted volume (verified at 30+ calls per turn), collapse into expandable bundles per agent turn, truncate outputs >500 chars with "show more", and honor a per-tool per-agent approve-before-run policy shared with MCP-04
  6. User can silence an agent in a thread (agent visible but receives no messages) and remove an agent from a thread (participant removal) — both verified on iOS end-to-end
**Plans**: TBD
**UI hint**: yes

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 10. Foundation | v3.2 | 5/5 | Complete | 2026-03-30 |
| 11. Core Messaging UI | v3.2 | 4/4 | Complete | 2026-03-30 |
| 12. Streaming & Interactive Blocks | v3.2 | 4/4 | Complete | 2026-03-30 |
| 13. AI-Native Features & Platform | v3.2 | 5/5 | Complete | 2026-03-30 |
| 14. UAT Automation Harness | v3.2 | 6/6 | Complete | 2026-04-19 |
| 15. v3.2 Tech Debt + Off-GSD Ratification | v3.3 | 0/? | Not started | — |
| 16. Connector Auth Hardening + Delivery Gating | v3.3 | 0/? | Not started | — |
| 17. Python Connector SDK | v3.3 | 0/? | Not started | — |
| 18. OpenAI Responses API Upgrade | v3.3 | 0/? | Not started | — |
| 19. MCP Bridge Connector | v3.3 | 0/? | Not started | — |
| 20. iOS Live Agent Discovery + Compound Memory Groundwork | v3.3 | 0/? | Not started | — |
| 21. Multi-Agent UX + Connector Lifecycle + Tool Transparency | v3.3 | 0/? | Not started | — |

## Notes on Phase Structure

**Keystone phase:** Phase 16 is the keystone. The `connector_keys` table, two-key split, and signed challenge handshake are the trust anchor every subsequent connector phase depends on. MA-05 (server-side @-mention routing) ships in Phase 16 rather than Phase 21 because:

- It is security-class (blocks ANTI-01 runaway loops — the $47K multi-agent failure mode) and must land before any multi-agent thread is shipped to users
- It is a server-side `AgentRegistry` delivery-gating change, structurally adjacent to the connector_keys trust anchor (both gate who is allowed to do what)
- Shipping multi-agent iOS UX in Phase 21 on top of un-gated fan-out would require a second remediation pass once gating lands

Phase 21's iOS multi-agent UX builds on the already-gated delivery from Phase 16 — the client-side features (identity, attribution, silence, remove) converge with earlier phases that produce real connector ecosystem volume to validate against.

**Parallelizable phases:** Phases 17 (Python SDK) and 18 (OpenAI Responses) are independent — different code, different reviewers, shared only the Phase 16 auth contract.

**Granularity:** `coarse` per `config.json` — seven phases mapping 75 requirements with no artificial splits; each phase delivers a coherent, verifiable capability.
