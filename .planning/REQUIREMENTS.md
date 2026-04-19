# Requirements — v3.3 Universal Agent Client

**Milestone:** v3.3 — Universal Agent Client
**Started:** 2026-04-18
**Status:** Defined, awaiting roadmap

## Goal

Turn AINM from a reference iOS client for the protocol into a universal client where any agent runtime (Claude Code, Codex, GPT, MCP servers, custom Python) can connect and participate as a first-class thread member — with compound memory groundwork laid for v3.4.

## v1 Requirements

### v3.2 Tech Debt (DEBT)

Close the 13 items identified in the v3.2 milestone audit before shipping new surface area.

**Phase 10 foundation carryovers:**
- [ ] **DEBT-01**: iOS can complete live WebSocket handshake against a running Rust server (ainm.initialize request/response + ainm.initialized notification)
- [ ] **DEBT-02**: Cross-platform Ed25519 + RFC 8785 JCS round-trip verified (Swift signs → Rust verifies; Rust signs → Swift verifies; identical test vectors)
- [ ] **DEBT-03**: Bidirectional message signing exercised in integration test (user-signed messages pass Rust verification; agent-signed messages pass Swift verification)
- [ ] **DEBT-04**: Ed25519 keypair persists across iOS app launches via Keychain (`kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`, non-synchronizable)
- [ ] **DEBT-05**: CI UAT target simulator name is discoverable rather than hardcoded (derive from `xcrun simctl list`)

**Phase 11 messaging carryovers:**
- [ ] **DEBT-06**: User can read recent messages offline from GRDB cache when WebSocket is disconnected
- [ ] **DEBT-07**: Conversation list updates in real time as new messages arrive (no manual pull-to-refresh needed)
- [ ] **DEBT-08**: Agent identity badge visually distinct from human badge (icon + color differentiation verified against iOS Dynamic Type + Dark Mode)
- [ ] **DEBT-09**: Multi-block messages (content[] with mixed text+code+image+file blocks) render in order, with no block swallowed or duplicated

**Phase 14 UAT carryovers:**
- [ ] **DEBT-10**: `DeepLinkUITests.test_deepLink_missingMessage_showsNotFoundToast` passes without XCTSkip (root-cause the onAppear timing race, not mask it)
- [ ] **DEBT-11**: `SubThreadBranchUITests` reply-count badge asserted deterministically (root-cause the ForEach refresh timing)

**v3.2 Audit cross-cutting:**
- [ ] **DEBT-12**: Dependency map produced for all DEBT items before execution (clusters, not flat list — e.g. Keychain cluster DEBT-02+03+04; offline cluster DEBT-06+07; UAT timing cluster DEBT-10+11)
- [ ] **DEBT-13**: Full XCUITest suite (`make uat`) green after each DEBT cluster lands (no regressions in already-passing v3.2 coverage)

### Off-GSD Ratification (RAT)

Pull connector packages built outside GSD into milestone artifacts with production-grade hardening.

- [ ] **RAT-01**: `@ainm/connector` TypeScript SDK ratified — test coverage audit, error-path tests, version-pinning policy (no `^` ranges), structured logging without secret leakage, DECISIONS.md in package
- [ ] **RAT-02**: `@ainm/connector-claude-code` ratified — E2E coverage against live Claude Code CLI, OAuth-vs-`--bare` API-key tradeoff documented, orphan-process cleanup verified
- [ ] **RAT-03**: `@ainm/connector-openai` ratified — pre-upgrade baseline test suite (chat/completions path retained as fallback per OA-01), structured logging, version pinning
- [ ] **RAT-04**: Server `AgentRegistry` hardened — test coverage for register/cleanup/broadcast, race-condition tests for concurrent agent connections, graceful handling of mid-session disconnects
- [ ] **RAT-05**: Connector `dist/` directories excluded from git (.gitignore + CI check that fails on committed build artifacts)
- [ ] **RAT-06**: 8-point ratification checklist applied per package (deps pinned, tests green, docs present, logs redacted, errors mapped, versions aligned with SDK parity, no TODOs blocking, CI integrated)

### Connector Auth Hardening (AU)

Replace trust-by-connection with signed identity and server-side allowlist. This is the keystone — every subsequent connector feature depends on it.

- [ ] **AU-01**: Connector proves identity at handshake via Ed25519 signature over server-issued challenge nonce (replaces timestamp-only signing; closes replay attack window)
- [ ] **AU-02**: User can onboard a new connector by pasting a token from iOS app into connector CLI (token binds connector public key to user identity; works for `gh auth login`-style flows)
- [ ] **AU-03**: User can complete OAuth browser flow to connect hosted runtimes (Claude Pro, OpenAI-hosted) — redirect URI handling, PKCE, refresh tokens
- [ ] **AU-04**: Connector SDK supports local-only API-key storage — keys never transit AINM server (documented, connector-side; covers Cline/Continue-style flows)
- [ ] **AU-05**: Two-key connector identity — `k_sign` (stable, published in Agent Card, signs messages) and `k_conn` (per-runtime, rotated ≤90 days, used only in handshake). One compromise ≠ full impersonation
- [ ] **AU-06**: Server `connector_keys` table stores per-user public-key allowlist — TOFU-in-dev, strict-in-prod; revocation supported
- [ ] **AU-07**: User can revoke a connector from iOS Settings (removes `connector_keys` row, disconnects active session)
- [ ] **AU-08**: Reconnect storm protection — server responds 503 + Retry-After when over-capacity; connector SDK uses exponential backoff with full jitter

### Python Connector SDK (PY)

Feature-parity Python SDK matching the TypeScript `@ainm/connector` API. Ecosystem-unlocking for LangChain / CrewAI / LangGraph / custom-Python agents.

- [ ] **PY-01**: Python `ainm-connector` package with simple 5-line agent API matching `@ainm/connector` TS surface
- [ ] **PY-02**: Advanced API tier parity — streaming, memory push, tool calls, context frames (mirrors TS advanced tier)
- [ ] **PY-03**: Cross-SDK signature conformance — Python-signed envelopes verify byte-identically against TS + Rust; Rust-generated test vectors (20+ cases) in CI, run on every PR to either SDK
- [ ] **PY-04**: RFC 8785 JCS via `rfc8785` 0.1.4 (Trail of Bits) — no alternative library accepted; conformance test vectors from official corpus
- [ ] **PY-05**: WebSocket client with ping/pong, close-code handling, reconnect with jitter (`websockets` 16.0)
- [ ] **PY-06**: HTTP Agent Card serving (`fastapi` + `uvicorn` + ETag-conditional GET)
- [ ] **PY-07**: E2E test suite parity with TS connector — runs against live Rust demo server, identical scenarios (send, stream, tool call, memory, handshake failures)
- [ ] **PY-08**: Published to PyPI as `ainm-connector` with exact version pinning, Python 3.12+ floor (PEP 695 generics), typed (`py.typed`)

### OpenAI Responses API Upgrade (OA)

Upgrade OpenAI connector from chat/completions to Responses API. Not optional — Assistants API shuts down 2026-08-26.

- [ ] **OA-01**: OpenAI connector uses Responses API as default path; chat/completions retained behind feature flag as fallback for one cycle
- [ ] **OA-02**: AINM thread_id maps 1:1 to Responses `previous_response_id` chain per thread; sub-threads start fresh chains (fork-from-here semantics)
- [ ] **OA-03**: Function-calling tool invocations from Responses API surface as `ainm.tool.*` events in the thread with attribution to OpenAI connector
- [ ] **OA-04**: OpenAI-hosted tools (web_search, code_interpreter) surface as `ainm.tool.*` events prefixed `openai:`; limitation that ApprovalBlock cannot gate hosted tool execution documented
- [ ] **OA-05**: Record+replay test cassettes for ≥6 Responses event shapes (text delta, function call args, tool output, error, completion, refusal) to prevent silent event-shape regressions
- [ ] **OA-06**: Per-agent feature flag controls chat/completions vs Responses — enables phased rollout and easy rollback

### MCP Bridge Connector (A6) (MCP)

Bridge any MCP server into AINM as an agent-like participant. MCP server's tools become AINM tool calls. One process per MCP server.

- [ ] **MCP-01**: MCP server appears as a single agent participant in AINM (identity `<server>@mcp.local`), published Agent Card with tool-only capabilities, one bridge process per MCP server
- [ ] **MCP-02**: MCP tool invocations attribute to the MCP-agent identity (not to "system" or the bridge) in tool call indicators
- [ ] **MCP-03**: MCP Apps declarative UI (returned via `_meta.ui.resourceUri`) renders inline in the AINM thread — mapped to AINM interactive blocks where possible, WKWebView fallback otherwise
- [ ] **MCP-04**: Per-tool consent gate on first use — user approves or denies, policy stored per-tool per-agent (reuses existing ApprovalBlock UI)
- [ ] **MCP-05**: Full MCP `inputSchema` forwarded to AINM tool call params without constraint loss; local `ajv` validation before forwarding to MCP server
- [ ] **MCP-06**: MCP `tools/list_changed` capability — bridge reacts to mid-session tool registry changes, re-publishes Agent Card
- [ ] **MCP-07**: Stdio subprocess cleanup — `detached: true` + process-group kill; SIGTERM → SIGKILL escalation; leak-detection test

### iOS Live Agent Discovery (DISC)

Replace HTTP-snapshot Agent Discovery with a live push-subscription so dynamically-connected agents appear without rebuild.

- [ ] **DISC-01**: New JSON-RPC method `ainm.directory.list` — iOS client requests a snapshot of agents available to the user
- [ ] **DISC-02**: New JSON-RPC notification `ainm.directory.updated` — server pushes agent connect/disconnect events to subscribed iOS clients via existing HumanRegistry broadcast
- [ ] **DISC-03**: New HTTP endpoint `GET /.well-known/ainm/directory` — cold-start / fallback snapshot for clients that haven't connected WebSocket yet
- [ ] **DISC-04**: iOS Agent Discovery view subscribes on appear, unsubscribes on dismiss (view-scoped Task lifecycle, no subscription leak)
- [ ] **DISC-05**: Snapshot-plus-deltas pattern with generation tokens — iOS performs full resync on reconnect, rejects deltas with stale generation

### Compound Memory Groundwork (MEM)

Position v3.4 for the full compound memory system. This milestone ships provenance, the write channel, and the portability primitives — cross-agent read ACL lands in v3.4.

- [ ] **MEM-01**: Memory archival facts carry provenance fields — `source_agent_id`, `source_thread_id`, `source_message_id`, `extracted_at`, `confidence` — schema migration from unscoped legacy facts planned
- [ ] **MEM-02**: Connector SDK exposes `ctx.memory.store(fact, scope)` — connectors push facts to server with provenance; default scope `private`
- [ ] **MEM-03**: User previews and consents to memory writes — connector-emitted facts surface as an inline "Claude wants to remember X" action block before persistence; rejection is non-blocking
- [ ] **MEM-04**: User can export all archival facts as JSON from iOS Settings (portability primitive — "compound memory that belongs to the user")
- [ ] **MEM-05**: Memory scope enum ships with values `private / shared:thread / shared:user / public:user` — server accepts writes at all scopes but defaults read-filter to `private` (cross-agent read ACL enforcement is v3.4)

### Multi-Agent UX + Connector Lifecycle + Tool Transparency (MA / CN / TC)

iOS-side features that realize the "universal agent client" claim end-to-end. Builds on the full connector ecosystem delivered by earlier phases.

**Multi-agent thread UX:**
- [ ] **MA-01**: Every agent in a thread has a distinct visual identity — avatar + display name + provider domain rendered on every message
- [ ] **MA-02**: User sees per-agent typing/processing indicators concurrently (N agents → N indicators, not one collapsed indicator)
- [ ] **MA-03**: Message attribution always visible — consecutive-sender collapse rule disabled when sender type is `agent`
- [ ] **MA-04**: Tool call indicators show which agent invoked the tool (`issuer_agent_id`); attribution consistent across all connector types
- [ ] **MA-05**: Server-side @-mention routing — an agent receives a message only if @-mentioned (or explicitly handed-off to); AgentRegistry gates delivery by mention-set. Blocks ANTI-01 runaway loops
- [ ] **MA-06**: User can silence an agent in a thread — agent remains visible but stops receiving messages until unsilenced
- [ ] **MA-07**: User can remove an agent from a thread (participant removal)

**Connector lifecycle:**
- [ ] **CN-01**: User can tap "Add" on an Agent Card in the Discovery view to onboard that agent (triggers paste-token or OAuth flow from AU)
- [ ] **CN-02**: `ainm://add-agent?card=https://…` deep link triggers the add-agent flow (extends existing DeepLinkHandler)
- [ ] **CN-03**: User can scan a QR code to pair a local connector (laptop Claude Code → phone) — camera view + server pairing endpoint
- [ ] **CN-04**: iOS Settings shows list of paired connectors with identity, last-seen, and revoke action
- [ ] **CN-05**: User can revoke a paired connector (triggers AU-07 server revocation)

**Tool call transparency at connector scale:**
- [ ] **TC-01**: Every tool call remains visible by default across all connector types (verified at connector-emitted volume, not just v3.2 synthetic fixtures)
- [ ] **TC-02**: Tool call indicators collapsed by default with tap-to-expand — verified scales to 30+ calls in a single turn without UI degradation
- [ ] **TC-03**: Tool calls within a single agent turn grouped under an expandable bundle ("Claude did 14 tool calls → expand")
- [ ] **TC-04**: Large tool outputs (>500 chars) truncated with "show more" — file reads, bash output, long strings don't flood the thread
- [ ] **TC-05**: User can set approve-before-run policy per-tool per-agent — policy persisted, reuses ApprovalBlock UX (shared policy store with MCP-04)

## Future Requirements (deferred to v3.4+)

- **MEM-cross-agent-read (v3.4)**: Cross-agent memory read ACL enforcement — GPT agent can read Claude's extracted facts if user allowed. v3.3 ships provenance + scope enum; v3.4 ships the read gate.
- **DIFF-per-turn-memory-viz (v3.4)**: User sees which facts went into each agent's context window per-turn (generalizes v3.2 memory context sheet).
- **DIFF-replaceable-agents (v3.4)**: Seamless Claude-3.5 → Claude-4 swap with inherited memory (the compound-memory signature feature).
- **DIFF-parallel-conference (v3.4+)**: Side-by-side rendering of 2 agents answering the same question.
- **DIFF-handoff-block (v3.4+)**: First-class HandoffBlock content type for explicit agent-to-agent handoffs.
- **DIFF-signed-tool-calls (v3.4+)**: Ed25519 signatures on tool-call events for cross-agent spoofing protection.
- **Cost transparency per connector (v3.4+)**: "This reply cost $0.03" per-message attribution.
- **Multi-device connector pairing (v3.4+)**: Pair once, use on iPhone + iPad.
- **Tool-call budget enforcement (v3.4+)**: Per-turn / per-thread token and $ budgets.
- **OpenAI-hosted tool approval gates (v3.4+)**: ApprovalBlock gating for web_search / code_interpreter (blocked today by OpenAI's server-side execution model).
- **MCP elicitation → ApprovalBlock (v3.4+)**: MCP `elicitation/create` bridged to AINM ApprovalBlock.
- **LLM-assisted MCP tool selection (v3.4+)**: Smarter default than explicit `@bridge:tool` syntax.
- **Federation × connector auth (v3.4+)**: Cross-server connector enrollment.

## Out of Scope

- **End-to-end encryption** — protocol chose server-visible content by design (agents must read content); incompatible with multi-agent threads
- **Voice / video messaging** — out of scope per PROJECT.md v1 constraints
- **iPad / Mac Catalyst** — iPhone-first per PROJECT.md; SwiftUI adaptation easier later
- **First-party agent marketplace with payments** — federation-hostile, contradicts open-standard constraint; users add agents by URL/QR/discovery
- **Autonomy-level slider** (low/med/high) — pseudo-control anti-pattern; per-tool policy (TC-05, MCP-04) is the real control surface
- **Unlimited agents per thread** — soft cap ~5 with warning (not hard block); beyond that UX breaks
- **Agent-initiated DMs** — spam risk, permission escalation; user must be first-thread-creator per agent
- **Auto-extract memory without preview** (ChatGPT default) — memory poisoning anti-pattern; MEM-03 preview mandatory
- **Hide agent tool calls to reduce noise** (Cursor default) — violates Core Value promise of transparency; TC-01 visible-default mandatory
- **Proprietary connector protocol** — contradicts open-standard constraint; we bridge MCP / A2A, don't compete

## Traceability

REQ-ID → Phase mapping (v3.3 Universal Agent Client — 75 requirements across Phases 15-21).

| Requirement | Phase | Status |
|-------------|-------|--------|
| DEBT-01 | Phase 15 | Pending |
| DEBT-02 | Phase 15 | Pending |
| DEBT-03 | Phase 15 | Pending |
| DEBT-04 | Phase 15 | Pending |
| DEBT-05 | Phase 15 | Pending |
| DEBT-06 | Phase 15 | Pending |
| DEBT-07 | Phase 15 | Pending |
| DEBT-08 | Phase 15 | Pending |
| DEBT-09 | Phase 15 | Pending |
| DEBT-10 | Phase 15 | Pending |
| DEBT-11 | Phase 15 | Pending |
| DEBT-12 | Phase 15 | Pending |
| DEBT-13 | Phase 15 | Pending |
| RAT-01 | Phase 15 | Pending |
| RAT-02 | Phase 15 | Pending |
| RAT-03 | Phase 15 | Pending |
| RAT-04 | Phase 15 | Pending |
| RAT-05 | Phase 15 | Pending |
| RAT-06 | Phase 15 | Pending |
| AU-01 | Phase 16 | Pending |
| AU-02 | Phase 16 | Pending |
| AU-03 | Phase 16 | Pending |
| AU-04 | Phase 16 | Pending |
| AU-05 | Phase 16 | Pending |
| AU-06 | Phase 16 | Pending |
| AU-07 | Phase 16 | Pending |
| AU-08 | Phase 16 | Pending |
| MA-05 | Phase 16 | Pending |
| PY-01 | Phase 17 | Pending |
| PY-02 | Phase 17 | Pending |
| PY-03 | Phase 17 | Pending |
| PY-04 | Phase 17 | Pending |
| PY-05 | Phase 17 | Pending |
| PY-06 | Phase 17 | Pending |
| PY-07 | Phase 17 | Pending |
| PY-08 | Phase 17 | Pending |
| OA-01 | Phase 18 | Pending |
| OA-02 | Phase 18 | Pending |
| OA-03 | Phase 18 | Pending |
| OA-04 | Phase 18 | Pending |
| OA-05 | Phase 18 | Pending |
| OA-06 | Phase 18 | Pending |
| MCP-01 | Phase 19 | Pending |
| MCP-02 | Phase 19 | Pending |
| MCP-03 | Phase 19 | Pending |
| MCP-04 | Phase 19 | Pending |
| MCP-05 | Phase 19 | Pending |
| MCP-06 | Phase 19 | Pending |
| MCP-07 | Phase 19 | Pending |
| DISC-01 | Phase 20 | Pending |
| DISC-02 | Phase 20 | Pending |
| DISC-03 | Phase 20 | Pending |
| DISC-04 | Phase 20 | Pending |
| DISC-05 | Phase 20 | Pending |
| MEM-01 | Phase 20 | Pending |
| MEM-02 | Phase 20 | Pending |
| MEM-03 | Phase 20 | Pending |
| MEM-04 | Phase 20 | Pending |
| MEM-05 | Phase 20 | Pending |
| MA-01 | Phase 21 | Pending |
| MA-02 | Phase 21 | Pending |
| MA-03 | Phase 21 | Pending |
| MA-04 | Phase 21 | Pending |
| MA-06 | Phase 21 | Pending |
| MA-07 | Phase 21 | Pending |
| CN-01 | Phase 21 | Pending |
| CN-02 | Phase 21 | Pending |
| CN-03 | Phase 21 | Pending |
| CN-04 | Phase 21 | Pending |
| CN-05 | Phase 21 | Pending |
| TC-01 | Phase 21 | Pending |
| TC-02 | Phase 21 | Pending |
| TC-03 | Phase 21 | Pending |
| TC-04 | Phase 21 | Pending |
| TC-05 | Phase 21 | Pending |

### Coverage Check

- Total v1 requirements: 75
- Mapped to phases: 75
- Orphaned (unmapped): 0
- Duplicates (mapped to >1 phase): 0

✓ 100% coverage

### Phase Distribution

| Phase | Category Composition | Count |
|-------|----------------------|-------|
| 15 | DEBT × 13 + RAT × 6 | 19 |
| 16 | AU × 8 + MA × 1 (MA-05) | 9 |
| 17 | PY × 8 | 8 |
| 18 | OA × 6 | 6 |
| 19 | MCP × 7 | 7 |
| 20 | DISC × 5 + MEM × 5 | 10 |
| 21 | MA × 6 (MA-01..04, 06, 07) + CN × 5 + TC × 5 | 16 |
| **Total** | | **75** |

### Notes

- **MA-05 in Phase 16**, not Phase 21: server-side @-mention routing is security-class (blocks ANTI-01 runaway loops / $47K multi-agent failure mode). It is a server-side AgentRegistry delivery-gating change, structurally adjacent to the connector_keys trust anchor. Shipping it in Phase 21 alongside iOS UX would require either un-gated multi-agent threads in the interim (unacceptable) or a blocked Phase 21. Lifting it to Phase 16 keeps the trust anchor and the delivery gate colocated and lets Phase 21 build on already-gated delivery.
- **MA × 6 in Phase 21**: MA-01 (distinct identity), MA-02 (parallel indicators), MA-03 (always-attributed), MA-04 (tool attribution), MA-06 (silence agent), MA-07 (remove agent). MA-05 lives in Phase 16 per above.
