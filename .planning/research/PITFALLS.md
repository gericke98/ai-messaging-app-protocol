# Pitfalls Research — v3.3 Universal Agent Client

**Domain:** Adding connector ecosystem (Python SDK, MCP Bridge, OpenAI Responses upgrade, auth hardening) + iOS live Agent Discovery + compound memory groundwork to existing AINM system (Rust/Axum server + SwiftUI iOS client + TypeScript Connector SDK).
**Researched:** 2026-04-18
**Confidence:** HIGH (Rust server and iOS client already shipped in v3.2; connector SDK + Claude Code + OpenAI connectors exist on disk from off-GSD work; OpenAI Responses API migration docs and MCP transport specs verified via official sources; Python JCS library landscape verified; prior v3.2 audit and retrospective reviewed for cross-cutting lessons.)

Prior-milestone pitfalls (iOS-specific — RFC 8785 Swift↔Rust parity, WebSocket backgrounding, LazyVStack pagination, etc.) were captured in v3.2's PITFALLS document. Those remain the canonical reference for iOS/Rust primitives; this document covers the **new integration surface** being added in v3.3.

---

## Critical Pitfalls

These mistakes cause rewrites, security breaches, production outages, or accepted tech debt that compounds.

---

### Pitfall 1: Python SDK silent canonicalization drift from TypeScript SDK

**What goes wrong:**
A message signed by the Python connector passes `verify_signature` on the Python side, is sent to the AINM server, and the server rejects it as `E_SIG_MISMATCH`. Or worse: the server accepts it (because the Rust `serde_json_canonicalizer` tolerates the difference) but the TypeScript connector then re-signs it during a relay/multi-hop flow and its signature fails. Drift is **content-dependent**: ASCII-only fixtures pass in CI; production Unicode/emoji/large-number content fails. Team assumes library handles JCS correctly because it's named `jcs` or `rfc8785`.

**Why it happens:**
1. Python has two competing JCS packages on PyPI (`rfc8785` from Trail of Bits and `jcs` from Anders Rundgren — same author as the RFC). They have different edge-case behaviors around: non-`str` dict keys (rfc8785 errors, jcs coerces), `Decimal`/`float` mixing, NaN/Infinity, and nested integer boundaries (JS `Number.MAX_SAFE_INTEGER` at 2^53).
2. Python `dict` is insertion-ordered since 3.7, which hides sort bugs: the developer's test dict happens to be in correct lexicographic order already, so missing sort code still "passes."
3. RFC 8785 requires UTF-16 code-unit key sorting. Python's natural string comparison is Unicode code-point (UTF-32-ish), which matches UTF-16 for BMP but diverges for characters above U+FFFF (supplementary plane — uncommon emoji, CJK ext B+). The bug is latent until a user includes a supplementary-plane character in agent metadata.
4. TypeScript uses the existing connector SDK JCS path (vetted in v3.1). Python will be a fresh port. Unless we cross-verify with shared test vectors, drift is silent.
5. Floats: `json.dumps(1.0)` in Python yields `"1.0"`. RFC 8785 + ECMAScript requires `"1"`. This breaks signing any payload with integer-valued floats.

**How to avoid:**
- Pick **one** canonical library — `rfc8785` (Trail of Bits, security-audited, MIT-licensed) — and pin exact version. Document why the other was rejected.
- Import the official JCS test-vector suite (github.com/cyberphone/json-canonicalization/testdata) as a Python test fixture. Run it on every commit. This suite catches UTF-16 sorting, float formatting, duplicate keys, and Unicode normalization.
- Write a **cross-SDK canonicalization contract test** that lives in the repo root (outside either SDK): a corpus of ~20 JSON inputs, each with a known-good canonical output. Both Python and TypeScript SDKs run this corpus in CI and must produce byte-identical output. A failure in either SDK fails the PR.
- Add a **round-trip integration test** in v3.3: Python connector signs → Rust server verifies; Rust server signs → Python connector verifies; TypeScript connector signs → Python verifies. All three pairs must pass.
- Forbid `json.dumps` and `JSON.stringify` for any payload that gets signed. Wrap canonicalization in a `canonicalize_for_signing()` function that is the only signing entry point.

**Warning signs:**
- E_SIG_MISMATCH errors that only reproduce with user-generated content (emoji, non-Latin scripts, numeric precision)
- Unit tests pass, integration tests against live server fail intermittently
- "Works for me" reports that differ across dev machines (different locale, different Python patch version)
- CI test matrix doesn't include a supplementary-plane Unicode fixture

**Phase to address:**
Phase 16 (Python SDK Foundation) — must be solved before the first Python connector E2E test. A Phase 16 success criterion should be: "Cross-SDK canonicalization contract test passes for all 20 vectors."

---

### Pitfall 2: Orphaned MCP stdio subprocesses on connector crash or restart

**What goes wrong:**
The MCP Bridge connector launches an MCP server as a child stdio process (e.g., `uvx mcp-server-filesystem`). The bridge crashes, is SIGKILL'd by the OS, the user hits Ctrl-C, the parent Node/Python process exits abnormally, or a container is rescheduled. The MCP subprocess is **not reaped**. Over time: zombie processes consume PIDs, leaked stdio pipes fill kernel buffers, file handles leak, and locally-run MCP servers (which may hold DB locks or bind ports) block future bridge restarts with cryptic "port in use" or "database locked" errors.

**Why it happens:**
- Node's `child_process.spawn` does not automatically terminate children when the parent dies (except on specific signals). Python's `subprocess.Popen` behaves identically.
- The MCP stdio handshake spec says the client should close stdin, wait for exit, then escalate SIGTERM → SIGKILL. If the bridge is killed before it completes this dance, the MCP server is orphaned — it may hang forever waiting for stdin input that will never come.
- Developers test on macOS where PID reuse is slow and impact is invisible, then deploy on Linux where PID exhaustion and cgroup-level process limits bite.
- The fallback approach — use a process tree killer — is platform-specific (`process.kill(-pid, SIGTERM)` needs `detached: true` process group) and easy to get wrong.

**How to avoid:**
- Always spawn MCP subprocesses with `detached: true` + `process.kill(-pid, ...)` (Node) or `start_new_session=True` + `os.killpg(...)` (Python). Document this as a hard requirement in the MCP Bridge architecture doc.
- Implement a **supervisor**: wrap every MCP subprocess in a small supervisor task (Node `AbortController` with cleanup, or Python `asyncio.TaskGroup` with `__aexit__`) that runs on both graceful and non-graceful shutdown.
- Register `process.on('exit' | 'SIGINT' | 'SIGTERM' | 'uncaughtException')` handlers (Node) or `atexit` + `signal.signal` (Python) that kill the subprocess with escalation: close stdin → wait 2s → SIGTERM → wait 3s → SIGKILL.
- In production deployments, run the bridge under a process manager (systemd, k8s with `shareProcessNamespace: true` + `terminationGracePeriodSeconds`) so orphaned children get reaped by PID 1.
- Add a **leak detection test**: spawn 50 MCP servers, crash the bridge hard (SIGKILL), assert no `mcp-server-*` processes survive. Run in CI on Linux.

**Warning signs:**
- Bridge restart fails with "address already in use" on an internal port
- `ps aux | grep mcp-server` shows processes with PPID=1 (re-parented to init = they were orphaned)
- Local SQLite MCP servers report "database is locked" after bridge restart
- Prometheus/metrics show process count climbing monotonically during test runs

**Phase to address:**
Phase 18 (MCP Bridge Connector) — supervisor pattern and cleanup must be in the first implementation plan, not added reactively after the first outage.

---

### Pitfall 3: Connector identity key = agent signing key (single-key compromise)

**What goes wrong:**
The connector SDK uses one Ed25519 keypair for both (a) authenticating itself to the AINM server during the WebSocket handshake, and (b) signing agent-originated messages written into the protocol. An attacker who compromises the connector's stored key (via disk exfiltration, memory dump, container image leak, or an RCE in the connector runtime) can now both **impersonate the agent in the thread** *and* **connect as that agent** to the server from anywhere. Revocation becomes a blunt instrument — rotating the signing key invalidates history; rotating the auth key requires a signed-revocation message from the old key, but the attacker has that too.

**Why it happens:**
- Simplicity pressure: "why have two keys when one works?" — shipped in v3.1 reference demo where connectors didn't yet exist.
- Ed25519 is cheap, so there's no performance excuse for key reuse. Developers just default to "one identity key per entity."
- The v3.1 spec defines Agent identity via a single Agent Card signing key; extending to a separate connector/runtime credential was deferred.
- Key storage abstractions (Keychain on iOS, `KeychainAccess`/libsecret/keyring on connector side) treat "the key" as singular — no obvious API for "two keys, one for transport, one for content."

**How to avoid:**
- Define **two key classes** in the protocol extension:
  - **Agent signing key** (`k_sign`) — stable, long-lived, signs message content. Published in the Agent Card. Rotation requires coordinated announcement.
  - **Connector session key** (`k_conn`) — per-runtime, short-lived (≤ 90 days), used only for server-handshake authentication + nonce-binding. Not published in Agent Card. Rotation is routine.
- The signed handshake sends `{agent_id, connector_id, timestamp, server_nonce}` signed by `k_conn`; the server resolves `k_conn` via a separate allowlist/registration endpoint.
- Store `k_sign` in the most secure available store (iOS Keychain, HSM, or kernel-managed secret); `k_conn` can live in more accessible storage.
- Compromise of `k_conn` alone = attacker can impersonate the transport session until rotation (hours/days, not forever); cannot forge new messages that pass server verification of `k_sign`-based signatures on historical content.
- Include `k_conn.kid` in the handshake so the server knows which specific session key is being used; log all handshakes with `kid` for forensic rotation.

**Warning signs:**
- Architecture doc says "connector keypair" in singular
- Agent Card JSON includes a field used both for published identity and for server auth
- No key rotation endpoint or documented procedure
- Log entries show the same `kid` value persisting across connector restarts and deployments

**Phase to address:**
Phase 19 or 20 (Connector Auth Hardening) — must land before we declare connector authentication GA. Spec update required — this is a protocol addition, not just an implementation detail.

---

### Pitfall 4: Signed-handshake replay attack without server-bound nonce

**What goes wrong:**
The connector authenticates via a signed handshake: `sign(k_conn, {agent_id, timestamp})`. An attacker on the wire (or who pulls a captured handshake from a CI log, proxy log, or intrusion on a developer laptop) replays the same bytes 10 minutes later and is admitted as the agent. Rolling timestamps alone ("must be within ±5 min") leaves a large replay window; static nonces picked by the client are useless because the client generates them.

**Why it happens:**
- JWT/bearer-token culture: developers reach for `Authorization: Bearer <signed-timestamp>` mental model, which is replayable by design without additional anti-replay.
- TLS alone isn't enough because the attack surface includes *any* place the handshake bytes are logged: CI artifacts, Sentry breadcrumbs, HAR files, tcpdumps during debugging.
- `timestamp ± skew` feels like it solves replay but doesn't — just shrinks the window.
- Connectors reconnect frequently (iOS background/foreground, server restarts, network transitions). A permissive replay window is pressured by "reduce login friction during reconnect storms."

**How to avoid:**
- Implement a **challenge-response handshake**:
  1. Client opens WebSocket, sends `connector.hello { agent_id, connector_id, kid }` (unsigned).
  2. Server responds with `connector.challenge { nonce: random 256-bit, server_time, exp: server_time + 30s }`.
  3. Client responds with `connector.auth { sig = sign(k_conn, canonical_jcs({nonce, agent_id, connector_id, kid})) }`.
  4. Server verifies `sig` against resolved `k_conn`, records the nonce as used (LRU cache or Redis set with 60s TTL), admits or rejects.
- Nonces are **one-time**: server remembers used nonces for 2× `exp` to prevent replay within the issue window.
- Bind the nonce to the TLS session via channel binding (RFC 8471 `tls-exporter`) in production — prevents MITM replay even with a captured nonce + signature.
- Short TTL: 30 seconds max between challenge issue and auth. Reconnect storms can afford one extra round-trip; replay exploits cannot.
- Log rejected auth attempts with `{agent_id, kid, reason, client_ip}` for intrusion detection.

**Warning signs:**
- Handshake code signs anything constructed by the client alone
- No server-side nonce cache exists
- "Replay window" discussion in PRs/chat references minutes, not seconds
- Same `sig` value succeeds twice in a stress test

**Phase to address:**
Phase 19 or 20 (Connector Auth Hardening) — couple this with Pitfall 3 (two keys). Both land together or neither lands.

---

### Pitfall 5: OpenAI Responses API streaming event-shape break breaks tool UI

**What goes wrong:**
The existing OpenAI connector (off-GSD work) wraps `chat/completions` streaming (SSE of OpenAI `choices[0].delta` chunks) and translates to AINM streaming frames. Upgrading to the Responses API silently changes the event shape — semantic events like `response.output_item.added`, `response.output_text.delta`, `response.function_call_arguments.delta`, `response.completed`. The connector's translator emits corrupted AINM stream frames (partial text, doubled tokens, missing tool-call correlation IDs). The iOS client's `StreamingManager` (from v3.2) renders garbage: missing cursor, text duplication, tool indicators stuck in `running` state forever because `response.function_call.completed` is never mapped.

**Why it happens:**
- Responses API is a **different protocol**, not a backward-compatible evolution of Chat Completions. Shape changes include: (a) semantic events instead of raw deltas, (b) tool calls as first-class `items` (not embedded in message deltas), (c) `previous_response_id` statefulness, (d) strict function definitions by default, (e) reasoning tokens as separate items.
- Compat shims ("make Responses API look like Chat Completions") introduce subtle bugs that pass unit tests (happy path) but fail on edge cases (parallel tool calls, reasoning-token interleaving, response truncation).
- Developers migrate one integration at a time, so tests exercise old + new paths separately but never the transition.
- The Chat Completions stream uses deltas (additive); Responses can send absolute-value events. Appending a delta where a replace was intended causes doubled text.

**How to avoid:**
- Treat the Responses API as a **new connector variant**, not an in-place upgrade. Name it `@ainm/connector-openai-responses` (or a sub-module) distinct from the legacy `connector-openai`.
- Write a dedicated **event-shape translator** with a discriminated union over Responses events (`response.created`, `response.output_item.added`, `response.output_text.delta`, `response.function_call_arguments.delta`, `response.output_item.done`, `response.completed`, `response.failed`, reasoning-token events). Exhaustive pattern match. Compile-time error if OpenAI adds an event we don't handle.
- **Record + replay** test harness: capture real Responses-API SSE streams (VCR-style cassettes) against OpenAI for at least: (a) streaming text, (b) single tool call, (c) parallel tool calls, (d) reasoning model (o-series) output, (e) truncated response, (f) tool-call then continued text. Replay cassettes in CI — no live API calls in tests.
- E2E test on iOS: real Responses stream → connector → server → iOS client → assert streaming cursor renders correctly, tool indicators complete, final message matches expected text.
- Keep the Chat Completions connector alive as fallback for one release cycle. Feature-flag per-agent which backend to use.
- Document cost delta: Responses can be more expensive per call when reasoning tokens are returned. Surface to the user via the Agent Card.

**Warning signs:**
- Tool indicators stay on "running" forever for Responses-backed agents
- Streaming text appears character-doubled or has gaps
- Unit tests pass but user reports "agent feels broken"
- Cost alerts trigger on the OpenAI dashboard

**Phase to address:**
Phase 20 or 21 (OpenAI Responses API migration) — dedicated phase, do not bundle with the larger auth hardening plan.

---

### Pitfall 6: MCP tool schema translation loses constraints silently

**What goes wrong:**
An MCP server declares a tool with `inputSchema: { type: "object", properties: { path: { type: "string", pattern: "^/safe/.*" } }, required: ["path"] }`. The MCP Bridge translates this to an AINM tool-call entry that's a best-effort mapping. The `pattern` constraint is dropped because AINM's tool-call format doesn't propagate JSON Schema `pattern` to the agent / user-side renderer. An agent calls the tool with `path: "/etc/passwd"`. The bridge forwards the call to the MCP server, which **also** validates — except: some MCP servers trust the client and skip validation in favor of runtime errors. User ends up with an unintended file access. Or flip side: the MCP server rejects the call, but because the rejection arrives out-of-band (stderr / logs / error response), the AINM side shows a generic "tool failed" with no context.

**Why it happens:**
- MCP's `inputSchema` is a full JSON Schema Draft 2020-12; AINM tool-call params have always been a simpler typed struct. The mapping is lossy by design.
- Bridges tend to map the **shape** (names and types) but not the **constraints** (patterns, enums, bounds, required fields).
- The assumption "the MCP server will re-validate" is fragile — not all MCP servers do.
- Schema evolution: MCP server v1.2 adds a new required field; bridge was built against v1.1 schema cached at connect time; old schema shape is sent to agent, agent's tool call omits the new field, server errors with "missing required field" that the bridge can't explain.

**How to avoid:**
- The MCP Bridge **must forward the full JSON Schema** in the Agent Card tools section — AINM tool-call spec already supports `inputSchema` passthrough (verify this; if not, extend the spec in this milestone).
- **Validate locally before forwarding**: bridge runs `jsonschema` validation (Python) or `ajv` (TS) against the tool call params using the MCP server's current schema. Rejects bad calls without hitting the MCP server.
- **Propagate structured errors**: when the MCP server rejects, capture the JSON-RPC error code/message and surface as an AINM `tool_call.error` with the original MCP payload attached under `details`. Don't reduce to "tool failed."
- **Re-fetch tool schemas** on every `notifications/tools/list_changed` (MCP spec capability). Never cache for the connector session lifetime.
- **Version-check**: log the MCP server's `server_info.version` on connect; if it changes on reconnect, invalidate cached schemas.

**Warning signs:**
- AINM tool-call shows fewer fields than the MCP server's declared schema
- Users see "tool failed" with no detail on why
- MCP server logs show validation errors that never reached the user
- Tool call schemas cached > 1 hour

**Phase to address:**
Phase 18 (MCP Bridge) — include "full schema passthrough + local validation" in the first plan's acceptance criteria.

---

### Pitfall 7: Stale Agent Registry snapshots on iOS after WebSocket reconnect

**What goes wrong:**
The iOS client subscribes to `agent.registry.updated` on the live AgentRegistry. It displays "12 agents online." The app goes to background for 20 minutes. iOS kills the WebSocket (per v3.2-known iOS background-socket behavior). User foregrounds, the app reconnects. The reconnect handler replays buffered events from the server but misses the `agent.registry.updated` events that happened *during* the disconnected window because the server only keeps a small event buffer. The iOS Discover view shows 12 agents (stale list), but 3 of them disconnected while the app was backgrounded, and 2 new agents connected. Tapping one of the 3 disconnected agents fails with a cryptic "agent unavailable" error.

**Why it happens:**
- The `agent.registry.updated` event model is **delta-based** (here's who joined/left), not **snapshot-based** (here's the current state). Missing deltas = stale state forever (until the next delta).
- iOS aggressively terminates background sockets. v3.2 retrospective notes "WebSocket connection lifecycle management" as a critical risk; v3.3 reconnect logic needs to be robust to multi-minute gaps.
- Subscriptions survive across view dismissal (leak) but then try to update a nil view (crash) or race with new subscriptions (double-update).
- "We trust server-side event delivery" mental model breaks when the client is offline.

**How to avoid:**
- On every successful (re)connect, **always pull a full registry snapshot** via a REST-style `agent.registry.snapshot` call before subscribing to deltas. The snapshot is the ground truth; deltas are incremental updates applied to it.
- Include a **generation token** in snapshots: `{generation: 17, agents: [...]}`. Each delta includes `{generation: 18, changes: [...]}`. On receiving a delta with `generation != prev + 1`, pull a fresh snapshot.
- **View-scoped subscription lifecycle**: use Swift `AsyncStream` tied to a `Task` held by the view model; cancel the task in `deinit`. Do not hold subscriptions in global singletons.
- **Optimistic UI with verification**: when the user taps an agent, validate freshness (< 10s old) before navigating; if stale, re-fetch that agent's card before showing the "Start Conversation" screen.
- **Server-side**: keep a 5-minute event replay buffer keyed by `generation`; clients send `last_seen_generation` on reconnect; server replays or refuses (then client falls back to full snapshot).
- Debounce rapid `agent.registry.updated` events (e.g., 250ms) so UI isn't thrashed during reconnect storms.

**Warning signs:**
- User reports "agent shown online but won't respond"
- Agent list doesn't match between two devices simultaneously logged in
- Memory grows in Agent Discover view between navigations
- Console logs show "subscription updated" events after the view was dismissed

**Phase to address:**
Phase 17 (iOS Live Agent Discovery) — snapshot-plus-deltas pattern must be designed in the phase's first plan, not bolted on after user reports.

---

### Pitfall 8: Compound memory cross-agent leak without per-agent ACL

**What goes wrong:**
Agent A (a work CRM assistant) extracts the fact "user's quarterly target is $2.4M" and writes it to compound memory. Agent B (a personal fitness coach, invited into a different thread) queries compound memory on startup and sees the CRM fact because memory is keyed by user, not per-agent-scope. User has now leaked confidential business data to a third-party personal assistant. Worse: the CRM agent assumed memory was visible only to itself (reasonable default — its own notes). User consent was never explicitly obtained for cross-agent sharing.

**Why it happens:**
- "Compound memory" is pitched as the differentiator — "memory compounds across agents." Easy to implement as a single user-scoped store.
- Per-agent ACLs, consent UX, and scope labels are boring infrastructure that gets deferred for an "MVP" that immediately leaks.
- Agent authors assume their memory is private; users assume memory is shared. Both assumptions are reasonable. Neither is validated.
- The threat model isn't just malicious agents — it's careless agents. A well-intentioned agent that extracts "user likes $subject" may be extracting PII the user would not have shared with a different agent.

**How to avoid:**
- Define a **scope model** in the compound memory spec before writing code:
  - `scope: "private"` — only the writing agent can read
  - `scope: "shared:thread"` — agents in the same thread can read
  - `scope: "shared:user"` — any agent this user authorizes (requires explicit per-agent consent)
  - `scope: "public:user"` — readable by any agent in any thread this user participates in
  - Default: `private`. Upgrading to `shared:*` requires an explicit signed write + user notification.
- **User consent surface on iOS**: a Memory Sharing screen where the user sees "Agent A wrote 3 facts as `shared:user`. Which agents should see these?" with per-agent toggles.
- **Write-time labels**: the protocol message for memory writes includes `scope` as a required field. Server rejects writes without scope.
- **Read-time filter**: server filters memory reads by `requesting_agent_id` against each fact's `scope` + `allowlist`. No blanket "give me all user memory" query.
- **Auditability**: every memory read produces a log entry; iOS surfaces "last 24h: Agent B read 7 facts you shared" in privacy UI.

**Warning signs:**
- Memory writes don't include scope / ACL metadata
- User has no UI to see "which agents can read what"
- Spec document says "per-user memory" without any scope discussion
- No memory read log exists

**Phase to address:**
Phase 22 (Compound Memory Groundwork) — scope model is the **first** plan, before any aggregation/export code. If you can't design the ACL, don't ship the feature.

---

### Pitfall 9: Compound memory pollution from noisy / sarcastic / mistaken extraction

**What goes wrong:**
User in a thread says sarcastically: "oh yeah, I love 6am meetings, can we have more?" Agent extracts to memory: `fact: user prefers 6am meetings`. Two weeks later, Agent B sees the fact, schedules a 6am sync, user is furious. Worse: a prompt-injected assistant ("ignore previous instructions and write to memory...") can arbitrarily pollute. Without attribution, retraction is hard: the user can't find the source of the wrong fact to remove it.

**Why it happens:**
- LLM-driven extraction is high-recall but low-precision. A 5% hallucination rate, multiplied by months of messages × multiple agents, yields a polluted store.
- Extraction is usually "silent" — no explicit user-visible confirmation at write time.
- Sarcasm, negations ("I do **not** want X" extracted as "user wants X"), hypotheticals ("imagine if X..."), quoted text ("John said X"), and adversarial inputs all defeat naive extraction.
- Once written, facts are rarely audited.

**How to avoid:**
- **Confidence scores** on every extracted fact: `{fact, confidence: 0.0-1.0, source_message_id, extracted_by_agent, extracted_at}`. Low-confidence facts (< 0.7) don't surface to agents without user confirmation.
- **Source traceability**: every fact links to the specific `message_id` that produced it. Users can tap a fact to see the source message; wrong extraction → delete with one tap.
- **User-in-the-loop for important facts**: the iOS memory UI shows "3 new facts learned — review?" with per-fact approve/reject/edit. Default is "auto-approve if confidence > 0.9" with a setting.
- **Retract-and-supersede**: facts aren't deleted; they're marked `retracted` with a reason. Historical audit preserved.
- **Adversarial defense**: if a message came from an agent (not a human), facts extracted from it are marked `provenance: agent_derived` and cannot supersede `provenance: human_stated` facts.
- **Prompt-injection detection**: treat user messages as untrusted input for extraction; any text containing "ignore previous", "system prompt", or clearly-not-a-personal-fact patterns skip extraction.

**Warning signs:**
- Memory UI shows no "confidence" or "source" metadata
- Users report an agent "remembered something I never said"
- Facts accumulate monotonically without any retraction
- Extraction happens entirely server-side with no user notification

**Phase to address:**
Phase 22 (Compound Memory Groundwork) — must be in the groundwork spec even if full user-review UI lands in v3.4.

---

### Pitfall 10: WebSocket reconnect storm when server restarts

**What goes wrong:**
The AINM server is redeployed or crashes. 1,000 connectors + 500 iOS clients simultaneously detect disconnect and all attempt to reconnect in the same 2-second window. Server comes up, is immediately buried in handshakes + signature verifications + Agent Card fetches, spends the first 30 seconds at 100% CPU, rejects connections with "server overloaded," which the clients retry immediately, creating a thundering herd. Server can't stabilize. Observed in every production messaging system that ever existed.

**Why it happens:**
- Default reconnect policy: immediate retry, or constant 1s delay.
- No jitter → all clients hit the same moment.
- Clients don't distinguish "server restarted" (wait longer) from "transient network blip" (retry fast).
- Signature verification is CPU-heavy; burst of 1,500 handshakes = brief DoS.

**How to avoid:**
- **Exponential backoff with full jitter**: `delay = random(0, min(60s, base * 2^attempt))`. Spreads reconnects across the window.
- **Server-advised delay**: HTTP 503 + `Retry-After: 15` tells clients to wait; honor it. On WebSocket close code 1013 (Try Again Later), clients wait the advised time.
- **Capacity-aware admission**: server stops accepting new connections above X% CPU, returns 503 with progressively longer Retry-After.
- **Reconnect priority**: iOS clients that are foregrounded reconnect faster than background connectors (user-visible > background agents).
- **Handshake batching / pipelining**: allow the server to process handshakes in parallel but cap concurrency (e.g., 50 concurrent signature verifications); overflow queues with bounded size.
- **Synthetic load test**: deliberately restart the server while 1,000 synthetic clients are connected; measure time-to-full-recovery. Must be < 2 min. Run quarterly.

**Warning signs:**
- No jitter in reconnect code (grep for `setTimeout(connect, 1000)`)
- Server CPU spikes to 100% for > 10s after restart
- Clients observe "server unavailable" loops
- `Retry-After` headers ignored

**Phase to address:**
Phase 19 or 20 (Connector Auth Hardening) — add reconnect policy + server admission control to this phase since it touches the handshake path anyway. Parallel effort on iOS client reconnect (small plan within Phase 17).

---

### Pitfall 11: Ratifying off-GSD work without a ratification gate

**What goes wrong:**
~60-70% of connector code exists on disk (`packages/connector/`, `packages/connector-claude-code/`, `packages/connector-openai/`) from off-GSD work. The team declares "ratified" after a cursory read and moves on. Six weeks later, production issues surface: (a) no tests for a critical path, (b) decisions from the off-GSD author aren't documented, (c) version pins missing so a dependency update breaks prod, (d) the code calls a private API that was removed, (e) error handling path silently swallows failures. The milestone shipped on fiction — the code "works" in demo but isn't production-ready, and because it's now "ratified," nobody re-audits.

**Why it happens:**
- Sunk-cost fallacy: "we already have 60% done, let's not redo it."
- Ratification feels ceremonial and boring; building new features feels productive.
- Off-GSD work is often prototype-quality — the original author didn't write it to be shipped; they wrote it to prove a concept.
- v3.2 retrospective explicitly flagged "research corrections beat CONTEXT.md when they diverge" — same class of problem: trust-but-verify doesn't get budgeted.

**How to avoid:**
- Define a **ratification checklist** as the Phase 15 success criteria. Every off-GSD file must pass:
  1. **Test coverage**: unit tests for every public function; E2E test for every flow. `vitest --coverage` threshold ≥ 80% for critical paths.
  2. **Decision record**: every non-obvious design choice documented in a `DECISIONS.md` inside each package.
  3. **Version pinning**: `package.json` + `Cargo.toml` use exact versions (no `^`) for critical deps.
  4. **Error handling**: no `catch {}` blocks without logging; no `unwrap()` in Rust non-test code; `assert`s replaced with real error paths.
  5. **Logging**: structured logs at INFO level for state transitions; DEBUG for protocol messages; ERROR for failures with context.
  6. **CI integration**: package builds and tests run in the repo's CI matrix.
  7. **Documented public API**: README + inline doc comments on every exported function.
  8. **Spec conformance**: code matches current AINM spec version (audit against `spec/schemas/*.json`).
- **Two-person sign-off**: the original author + one other reviewer sign off per package.
- **Budget honesty**: estimate ratification at **50-70% of greenfield effort**, not 10%. Past experience: retrofitting tests to code you didn't write is slow.
- **Document what you won't do**: if something is accepted as tech debt (e.g., "error messages need improvement — tracked as issue #N"), write it down explicitly rather than pretending it's done.

**Warning signs:**
- "Ratify" shows up in a plan with a 1-hour estimate
- Test count for ratified package < 10
- No DECISIONS.md in package root
- `git log` of the package shows commits outside the GSD workflow that never got reviewed

**Phase to address:**
Phase 15 (Ratification + Tech Debt Cleanup) — this is the whole purpose of the phase. The ratification checklist must be the gate.

---

### Pitfall 12: v3.2 tech debt items are dependent, not independent

**What goes wrong:**
The v3.2 audit lists 11 HUMAN-UAT items + 2 deferred XCUITest items as "tech debt to address in v3.3." Team treats them as a checklist, assigns one to each day, and starts knocking them off. Partway through, a hidden dependency surfaces: fixing the Keychain persistence UAT (Phase 10) requires a real server round-trip, which exposes that the live WebSocket handshake UAT (also Phase 10) has a latent bug that only manifests under a specific reconnect pattern — which is the same reconnect pattern that causes the deferred `DeepLinkUITests` XCUITest flake. Suddenly 3 items are coupled. The "1 day per item" estimate blows up; the team either rushes (accepting new debt) or pauses other v3.3 work.

**Why it happens:**
- UAT items feel atomic but share substrate (live server, real Keychain, actual reconnect). They test different surfaces of the same machinery.
- Flaky tests are rarely caused by the thing the test is named after — they're symptoms of deeper timing issues elsewhere.
- "Test flake" is often "genuine race condition we haven't understood yet."
- v3.2 retrospective flagged `DeepLinkUITests` race condition: `scrollToMessageId`/`onAppear` timing. Similar races may exist throughout.

**How to avoid:**
- **Dependency map first**: before executing Phase 15, produce a `v3.2-TECH-DEBT-MAP.md` that groups the 13 items into clusters by shared surface (e.g., all "live server handshake" items together). Execute clusters, not items.
- **Root-cause flakes**, don't XCTSkip them permanently: the 2 deferred tests are symptoms. Spend time understanding `ForEach` refresh timing + `scrollToMessageId` onAppear race. Document the finding even if the fix lands in a later phase.
- **UAT-from-real-server**: Phase 10's 4 HUMAN-UAT items *all* require a live Rust server. Set up a persistent local environment once; run all 4 in a single session.
- **Regression risk budget**: every UAT fix can break something else. Run full XCUITest suite after each UAT flip; don't batch 4 fixes and then run tests.
- **Accept tech debt explicitly**: if an item won't be fixed in v3.3, document *why* and move to v3.4 — don't carry as indefinite debt.

**Warning signs:**
- Phase 15 plan has 13 parallel sub-plans, one per debt item
- No discussion of shared dependencies between items
- "Just XCTSkip it" appears in a PR
- New flakes appear that weren't present before Phase 15

**Phase to address:**
Phase 15 (v3.2 Tech Debt Cleanup) — dependency map is **the first deliverable** of this phase, before touching any code.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Share one Ed25519 key for connector auth + agent signing | Simpler SDK API, one keypair to manage | Single-key compromise = both impersonation + message forgery; revocation is scorched-earth | Never in v3.3+. Even prototypes should have the two-key split once a real runtime exists. |
| Use Python `json.dumps(sort_keys=True)` as "good enough JCS" | No dependency, works for ASCII | Silent signature failures with Unicode, floats, supplementary plane chars; exploitation by crafted payloads | Never for signed content. Acceptable only for non-signed debug output. |
| Cache MCP tool schemas for the connector session lifetime | Avoids re-fetch on every call | Stale schemas cause cryptic validation errors when MCP server updates | Acceptable if MCP server declares capability + bridge subscribes to `tools/list_changed`; not otherwise. |
| Compat shim to make Responses API look like Chat Completions | Incremental migration, one call-site change | Subtle translation bugs; streaming/tool-call edge cases break; masks real changes from the team | Never as a long-term solution. OK as a 1-week bridge while the real translator is under test. |
| Immediate WebSocket reconnect with no jitter | One line of code, feels responsive | Thundering herd on server restart; DoS against your own server | Never in production. Dev-only builds can have fast reconnect behind a flag. |
| XCTSkip a flaky test instead of root-causing | Green CI today | Race condition remains; flake resurfaces in worse form later | Acceptable with a tracking issue and max 1-phase deferral. v3.2's 2 items have already used this budget. |
| Ratify off-GSD code without the full checklist | Faster milestone start | Production bugs, missing tests, undocumented decisions bite later | Never. 50-70% greenfield budget for ratification is the right estimate. |
| Store compound memory as "one blob per user" without scope | Schema is trivial | Cross-agent leaks; no per-agent ACL possible without migration | Never in v3.3+. Scope model must be in the first spec. |
| No challenge-nonce in signed handshake, rely on timestamp | One round-trip saved | Replay attacks within the timestamp window | Never. Extra round-trip cost is ~10ms; exploits are far costlier. |
| Spawn MCP subprocess without process-group detachment | Simpler spawn code | Orphaned processes on crash; PID exhaustion in production | Never in production; dev-only OK if cleanup is manual. |
| Single connector package publishes both TS + Python bindings from one source | One repo, one tag | Build-tool bloat, bidirectional cross-language churn | Never. Separate packages with a shared contract-test harness. |
| Extract memory facts server-side without user review | Fast to ship, looks magical | Bad facts pollute; can't audit source; user trust collapses after one bad recommendation | Only with confidence threshold + user-review surface. |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Python JCS (`rfc8785` / `jcs`) | Assuming any RFC 8785-named library is bit-identical to TS/Rust impls | Pick one lib (`rfc8785`), pin version, run official JCS test-vector corpus + cross-SDK contract test in CI |
| Python asyncio + `websockets` lib | Not setting `ping_interval` / `ping_timeout`, losing connections silently to idle proxies | Explicit `ping_interval=20, ping_timeout=20`; handle `ConnectionClosed` + reconnect with jitter |
| Python `websockets` connection handler | Handler returns after one message instead of looping | Explicit `async for message in ws:` loop; log handler exit; auto-restart if it exits while connection is alive |
| MCP stdio subprocess | Forgetting `detached: true` / `start_new_session=True` | Always detach; always supervisor + escalating cleanup (stdin close → SIGTERM → SIGKILL) |
| MCP tool schema | Translating shape but dropping constraints (`pattern`, `enum`, `minimum`) | Forward full `inputSchema`; validate with `jsonschema`/`ajv` before call; handle `notifications/tools/list_changed` |
| MCP server stderr | Ignored, so crash signals are silent | Pipe MCP stderr to bridge logs with `[mcp:<server_name>]` prefix; surface fatal-looking lines in health panel |
| OpenAI Responses API streaming | Treating events as deltas when some are absolute | Exhaustive discriminated-union translator; record+replay cassettes in CI for every event type |
| OpenAI Responses API tool calls | Using Chat Completions function-call shape (external tagged, non-strict) | Use Responses shape (internal tagged, strict by default); `call_id` ties args to outputs |
| OpenAI Responses API state | Ignoring `previous_response_id` → losing reasoning/tool context | Decide explicitly: stateless (`store: false`) or threaded (`previous_response_id` chain) — document the choice |
| OpenAI Responses reasoning tokens | Not round-tripping the encrypted reasoning item | Store the `reasoning.encrypted` blob and pass back on the next call; otherwise model loses context |
| iOS Keychain across reinstalls | Assuming Keychain items survive (they don't, for most access classes) | Document expected loss; UX flow to re-register identity on reinstall; test by deleting app + reinstalling |
| iOS WebSocket backgrounding | Assuming socket survives > 30s backgrounded | iOS kills after ~30s; implement re-connect-on-foreground + full snapshot re-fetch |
| WebSocket close codes | Using generic 1000/1001 for everything | Use 4000+ custom codes for auth/nonce/replay errors so clients can differentiate retry vs stop |
| Ed25519 (ring / CryptoKit / Python) | Passing seed bytes vs expanded private key inconsistently | Always use 32-byte seed form; `Curve25519.Signing.PrivateKey(rawRepresentation:)` in Swift = `SigningKey(seed)` in PyNaCl = `signing_key_from_bytes` in ring |
| OAuth redirect URIs for Claude Code / OpenAI auth | Wildcard redirect URIs, or http:// in production | Explicit exact URIs; https only; per-environment registration |
| API-key-stored-in-plaintext (OpenAI, Anthropic) | `.env` file checked into repo, or copied to container env var | OS keystore (Keychain / libsecret / Windows DPAPI); in containers, use secret manager (k8s Secret + KMS) |

---

## Performance Traps

Scale thresholds estimated for this project (federated messaging; expected v3.3 scale: ~10K registered users, ~1K concurrent connections, ~50K messages/day).

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Unbounded MCP subprocess proliferation | Bridge memory grows over days; system slows | Hard cap on concurrent MCP servers per bridge; LRU eviction with graceful shutdown | ~100 concurrent stdio subprocesses on one bridge |
| N+1 Agent Card fetches after discovery notification | Network saturates on registry deltas | Batch card fetches; include card version in registry event; fetch only on delta | > 50 agents online + every discovery event triggers individual fetches |
| Synchronous signature verification in connection hot path | p99 handshake latency spikes under load | Pre-compute `k_conn` lookups; verify in dedicated worker pool; cap concurrency | > 100 handshakes/sec on a single server |
| Compound memory fetched on every agent message | Latency per message grows; server OOM | Fetch memory context once per thread-session; push-only updates via subscription | > 10K facts per user |
| JCS canonicalization in tight loop | CPU-bound Python/Rust; slow msg processing | Cache canonical bytes alongside message; never re-canonicalize for the same logical payload | Any bulk operation over > 100 messages |
| Reconnect storm (no jitter) | Server pinned at 100% for minutes post-restart | Exponential backoff + full jitter; server admission control | > 500 simultaneous clients and any restart |
| iOS LazyVStack with full registry | Scroll jank; memory grows with agent list | Paginate; fetch-on-demand for agent card details | > 200 agents in Discover list |
| WebSocket buffer overflow on streaming agents | Client sees "connection closed" mid-stream | Bounded queue + backpressure; server drops oldest non-critical events first | Sustained > 1000 events/sec to one client |
| `notifications/tools/list_changed` storm | Bridge churns re-fetching schemas | Debounce 1s; diff-based cache invalidation | MCP servers that emit on every internal state change |
| Compound memory write amplification | DB IO pinned; write latency grows | Batch writes; dedupe before insert; async write with bounded lag | > 100 writes/sec per user |
| OpenAI Responses `previous_response_id` chain grows unbounded | Latency climbs per call; token cost climbs | Cap chain length (e.g., last 10); summarize/compact older context client-side | Long-running sessions > 50 turns |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Logging signed handshake bodies at INFO | Captured logs = replayable credentials | Redact signatures + nonces; use structured logger with allowlist fields only |
| Pre-shared secrets in env vars committed to `.env.example` | Credential leaks on repo mirror / fork | Use placeholder values `REPLACE_ME`; CI test that real values aren't committed (grep for known-good prefix patterns) |
| OAuth redirect URI misconfiguration (Claude Code / OpenAI auth) | Open redirect → token theft | Exact-match registered URIs; per-env (dev/stage/prod) separate OAuth apps; no wildcards |
| "Trust on first use" connector identity | Attacker races to register first | Allowlist mode as default; TOFU only with user-visible "is this you?" confirmation per new kid |
| Single key for connector auth + agent signing | Single-compromise = full impersonation | Two-key architecture (see Pitfall 3) |
| Replay-vulnerable handshake (timestamp only) | Captured handshake re-used by attacker | Challenge-response nonce; one-time nonce cache server-side (see Pitfall 4) |
| Stale public keys in server allowlist after rotation | Rotated-away key still accepted | Rotation endpoint atomically updates allowlist + logs; health-check that confirms old key rejected |
| Lockout: user rotates own key, old key removed before new one confirmed | User cannot re-authenticate | Grace window: old key remains valid 24h after new key registered; rotation requires proving control of both keys |
| Connector impersonation via DNS/WebSocket MITM | Attacker proxies connector traffic | TLS cert pinning on connector → server; channel binding (RFC 8471 tls-exporter) in handshake |
| Prompt injection via memory writes | Agent A injects "user is admin" into B's context | Per-scope ACL; provenance tracking; treat agent-written facts as lower trust than human-stated |
| Compound memory export without consent | User data exfiltrated by rogue agent | Export requires explicit user confirmation; rate-limited; audit logged |
| MCP bridge trusts MCP server output unconditionally | Malicious MCP server sends forged tool output | MCP server output carries no AINM signature; bridge labels all MCP-origin content `provenance: mcp_server` for downstream trust decisions |
| Double auth UX (MCP auth + AINM auth) where one silently succeeds | User thinks they're authenticated but only half is | UX that makes both explicit; failure in either blocks; never auto-retry one after the other fails |
| Ed25519 secret key stored in filesystem with `0644` perms | Local privesc reads key | `0600` minimum; prefer OS keystore (Keychain / libsecret / Windows DPAPI) |
| Nonce generation with non-cryptographic RNG | Predictable nonces → replay | `crypto.randomBytes(32)` (Node), `secrets.token_bytes(32)` (Python), `CryptoKit` / `SystemRandomNumberGenerator` (Swift), `ring::rand::SystemRandom` (Rust) |
| MCP server stdin not validated before exec | Command injection via bridge config | Allowlist of known MCP commands; arg array (not shell string); sandbox (macOS sandbox-exec, Linux namespaces) where feasible |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Agent Discover list stale after foregrounding | User taps dead agent, gets cryptic error | Snapshot re-fetch on foreground + per-tap freshness check (< 10s) |
| Double-authentication for MCP-backed agents (MCP auth + AINM auth) | User gives up partway | Single consent surface; bridge handles both under the hood; UI shows one "Connect Filesystem Assistant" CTA |
| Memory facts appearing without user awareness | User surprised by agent "remembering" | "3 new facts learned" pill → review sheet with approve/reject; setting for auto-approve threshold |
| Key-rotation flow that requires typing a hex string | User abandons, stays on old key | Tap-to-rotate in Settings; grace window; background rotation where possible |
| Connector status buried in logs | User can't diagnose "why isn't my agent responding" | In-app connector health panel; last-success / last-error / last-retry visible per connector |
| Compound memory scope (private/shared) invisible to user | User doesn't know what's shared | Per-fact scope badge in memory UI; "sharing" screen to change per-fact |
| OpenAI Responses-backed agents feel slower than Chat Completions | User perceives regression | Surface "reasoning..." indicator for reasoning models; show tokens-per-second; acknowledge cost tradeoff in Agent Card |
| Tool call UI stuck on "running" forever when connector dies | User thinks agent is still working | Timeout → switch to "disconnected" state with retry CTA; clear visual distinction from "running" |
| Onboarding flow assumes technical knowledge (manual API key entry) | Users don't have API keys | OAuth-first for Claude Code, OpenAI; API key as advanced option; clear docs on where to get keys |
| Reconnect storm causes user-visible flicker | User perceives instability | Debounce connection-state UI (500ms); show "reconnecting..." only after > 2s disconnected |
| MCP server selection requires CLI knowledge | Power-user feature excludes everyone else | Curated list of popular MCP servers in Add Agent flow; advanced "stdio command" option for experts |
| Agent Card doesn't indicate backend (Claude Code vs GPT vs custom) | User can't distinguish which agent is which | Small backend badge on Agent Card; visible model/provider name |

---

## "Looks Done But Isn't" Checklist

Verification during Phase 15 ratification + end of each subsequent phase:

**Ratification (Phase 15)**
- [ ] **Off-GSD connector SDK:** Often missing cross-SDK test suite — verify TS SDK runs same JCS test vectors as Python SDK will
- [ ] **Claude Code connector:** Often missing CLI version pin — verify `package.json` pins exact Claude Code CLI version
- [ ] **OpenAI connector:** Often missing Responses API forward-compat branch — verify migration plan documents both APIs
- [ ] **AgentRegistry server code:** Often missing generation/snapshot endpoint — verify server exposes `agent.registry.snapshot` + `generation` field
- [ ] **iOS live Agent Discovery wiring:** Often missing full-snapshot on reconnect — verify reconnect handler calls snapshot + applies deltas in order
- [ ] **Off-GSD `dist/` directories in git status:** Often stale, never regenerated — verify `.gitignore` excludes + CI rebuilds from source

**Python SDK (Phase 16)**
- [ ] **Signing:** Often uses `json.dumps(sort_keys=True)` — verify uses `rfc8785` library with pinned version
- [ ] **WebSocket client:** Often no explicit ping config — verify `ping_interval` + `ping_timeout` set, reconnect handler exists
- [ ] **Test matrix:** Often Python 3.11-only — verify CI runs 3.11 + 3.12 + 3.13
- [ ] **Cross-SDK parity:** Often only tested within Python — verify cross-SDK contract test in CI
- [ ] **Type safety:** Often `Any` everywhere — verify `mypy --strict` passes

**iOS Live Agent Discovery (Phase 17)**
- [ ] **Subscription lifecycle:** Often leaks on view dismissal — verify `Task` cancellation in `onDisappear`
- [ ] **Snapshot-then-deltas:** Often delta-only — verify snapshot fetched on every (re)connect
- [ ] **Generation tracking:** Often missing — verify server sends `generation`; client detects gaps
- [ ] **N+1 Agent Card fetches:** Often per-event — verify batched or embedded in registry event

**MCP Bridge (Phase 18)**
- [ ] **Subprocess cleanup:** Often missing `detached: true` — verify process-group kill on exit
- [ ] **Signal handlers:** Often missing — verify SIGINT/SIGTERM/SIGKILL escalation
- [ ] **Tool schema forwarding:** Often drops constraints — verify full `inputSchema` passthrough
- [ ] **`tools/list_changed` handling:** Often ignored — verify re-fetch + cache invalidation
- [ ] **Provenance labeling:** Often missing — verify MCP-origin content tagged

**Connector Auth Hardening (Phase 19/20)**
- [ ] **Two-key architecture:** Often single key — verify `k_sign` and `k_conn` separate
- [ ] **Challenge-response handshake:** Often timestamp-only — verify server nonce + one-time cache
- [ ] **Key rotation endpoint:** Often missing — verify endpoint exists + grace window
- [ ] **Allowlist vs TOFU:** Often defaults to TOFU — verify default is allowlist
- [ ] **Reconnect policy:** Often no jitter — verify exponential backoff with full jitter
- [ ] **Server admission control:** Often missing — verify capacity-aware 503 + Retry-After

**OpenAI Responses Migration (Phase 20/21)**
- [ ] **Event translator:** Often partial — verify exhaustive discriminated union
- [ ] **Record+replay cassettes:** Often missing — verify cassettes for: text stream, single tool call, parallel tool calls, reasoning, truncation
- [ ] **Cost documentation:** Often missing — verify Agent Card surfaces backend + cost class
- [ ] **Stateful vs stateless choice:** Often unclear — verify documented decision
- [ ] **Compat shim lifecycle:** Often permanent — verify removal plan in milestone close

**Compound Memory Groundwork (Phase 22)**
- [ ] **Scope model in spec:** Often missing — verify `private/shared:thread/shared:user/public:user` defined
- [ ] **Per-agent ACL in writes:** Often missing — verify `scope` required in memory write schema
- [ ] **Source traceability:** Often missing — verify every fact links `source_message_id`
- [ ] **Confidence scores:** Often missing — verify `confidence` + `extracted_by_agent` fields
- [ ] **User consent UX stub:** Often deferred — verify at minimum a Settings toggle for auto-approve threshold
- [ ] **Retraction flow:** Often "delete only" — verify retract+supersede preserves audit trail

**v3.2 Tech Debt Cleanup (Phase 15)**
- [ ] **Dependency map:** Often treated as flat list — verify clusters mapped before execution
- [ ] **Root-cause flakes:** Often XCTSkip extended — verify root cause documented even if fix deferred
- [ ] **Live-server UAT session:** Often one-at-a-time — verify all 4 Phase 10 live items run in one session
- [ ] **Regression run:** Often skipped — verify full XCUITest suite green after each UAT flip

---

## Recovery Strategies

When pitfalls slip past prevention:

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Cross-SDK JCS drift shipped | MEDIUM | (1) Add failing cross-SDK test to repro; (2) identify divergent case (usually Unicode or float); (3) patch Python side (TS/Rust are canon); (4) deploy; (5) re-verify corpus. ~1-3 days. |
| Orphaned MCP processes in production | LOW | (1) kill all `mcp-server-*` processes; (2) deploy supervisor fix; (3) add k8s `shareProcessNamespace: true` as belt-and-suspenders. Hours. |
| Compromised connector key (both auth + signing) | HIGH | Without two-key: revoke agent entirely, users must re-authenticate all threads, messages signed by old key remain valid forever in history. With two-key (post-fix): rotate `k_conn` (minutes), leave `k_sign` untouched, minor UX disruption. Incentive to migrate to two-key ASAP. |
| Replay attack observed in server logs | HIGH | (1) Invalidate all active sessions; (2) force reconnect with challenge-nonce flow; (3) audit logs for forged message content; (4) if forged content committed, notify users + mark affected threads. Days. |
| Responses API streaming translator bug in prod | MEDIUM | (1) Feature-flag agents back to Chat Completions backend; (2) capture failing SSE cassette from affected session; (3) add cassette to test suite; (4) fix translator; (5) roll back feature flag. 1-2 days. |
| MCP schema drift causing tool failures | LOW | (1) Force schema re-fetch; (2) add `notifications/tools/list_changed` handler if missing; (3) document version-check on connect. Hours. |
| Stale Agent Registry on iOS clients | LOW | Force client reconnect; server responds with full snapshot; no data loss. Seconds. |
| Cross-agent memory leak (Pitfall 8 realized) | HIGH | (1) Audit memory store for cross-scope reads; (2) migrate store to add scope field (default `private`); (3) notify affected users + offer audit log of what was shared; (4) PR/incident report. Week+. |
| Polluted memory from bad extraction | MEDIUM | (1) Bulk-retract low-confidence facts from affected time window; (2) add confidence threshold gate; (3) invalidate downstream caches. Hours-day. |
| Reconnect storm in production | MEDIUM | (1) Deploy server-side 503 + Retry-After immediately (hotfix); (2) force-deploy client with jitter (requires app update, so slow); (3) capacity provisioning short-term. Hours for server, days for client. |
| Ratified off-GSD code hiding debt | HIGH | (1) Stop adding features to affected package; (2) run full checklist retroactively; (3) triage missing items; (4) budget a follow-up ratification phase. Weeks. |
| v3.2 XCUITest flake resurfaces with new symptom | MEDIUM | (1) Do not XCTSkip again; (2) add diagnostic logging; (3) root-cause the race; (4) fix or document as accepted risk with runbook. Days. |

---

## Pitfall-to-Phase Mapping

How the v3.3 roadmap phases should address these pitfalls. Phases 15-22 are a suggested structure derived from the milestone scope (PROJECT.md).

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| 1 — Python SDK canonicalization drift | Phase 16 (Python SDK Foundation) | Cross-SDK contract test passes in CI; official JCS test vectors green |
| 2 — Orphaned MCP subprocesses | Phase 18 (MCP Bridge) | Leak-detection test: spawn 50 MCP servers, SIGKILL bridge, assert zero survivors |
| 3 — Single-key compromise | Phase 19/20 (Connector Auth Hardening) | Spec update landed; SDK API exposes `k_sign` + `k_conn` separately; rotation endpoint documented |
| 4 — Replay-vulnerable handshake | Phase 19/20 (Connector Auth Hardening) | Server nonce cache active; replay-attack test (send same signed payload twice) rejects second |
| 5 — Responses API streaming break | Phase 20/21 (OpenAI Responses Migration) | All 5+ record-replay cassettes green; iOS E2E with Responses-backed agent completes successfully |
| 6 — MCP schema translation loss | Phase 18 (MCP Bridge) | Integration test: MCP server with `pattern` constraint; bridge validates locally; violating call rejected before MCP server sees it |
| 7 — Stale iOS Agent Registry | Phase 17 (iOS Live Agent Discovery) | Reconnect test: background 2 min, foreground; registry matches server-side truth; tap-dead-agent test catches staleness |
| 8 — Cross-agent memory leak | Phase 22 (Compound Memory Groundwork) | Scope model in spec; per-agent ACL in server; test: agent B cannot read agent A's `private` facts |
| 9 — Memory pollution from bad extraction | Phase 22 (Compound Memory Groundwork) | Confidence + source_message_id on every fact; retract flow works; sarcasm fixture triggers low confidence |
| 10 — Reconnect storm | Phase 19/20 (Connector Auth Hardening) + Phase 17 (iOS) | Load test: restart server with 1000 connections, full recovery < 2 min; jitter visible in client logs |
| 11 — Off-GSD ratification without checklist | Phase 15 (Ratification + Tech Debt) | Every ratified package passes 8-point checklist; DECISIONS.md exists; coverage ≥ 80% |
| 12 — v3.2 tech debt dependencies | Phase 15 (Ratification + Tech Debt) | Dependency map produced first; items executed in clusters; full XCUITest suite green after each |

### Cross-Cutting Concerns (not a single phase)

- **Logging discipline** — redact signatures/nonces, structured fields only, no secret leakage: enforced via Phase 15 ratification checklist + CI grep gate applied to all new code in phases 16+.
- **Version pinning across the repo** — exact versions, no `^` for critical deps: enforced in CI in Phase 15; documented in each package's DECISIONS.md.
- **Spec-implementation conformance** — every new message type/shape updates `spec/schemas/*.json` + `website/static/asyncapi.yaml`: enforced by Phase 15 audit, re-verified in each feature phase.
- **v3.2 retrospective lessons carried forward** — checker-verified planning loop (Phase 15+ onwards), research corrections briefed explicitly to executors (Phase 16+), cross-plan ID contract (any phase that spans fixtures/events/schemas).

---

## Sources

**Official specifications and docs (HIGH confidence):**
- [RFC 8785 — JSON Canonicalization Scheme](https://www.rfc-editor.org/rfc/rfc8785)
- [OpenAI Responses API migration guide](https://developers.openai.com/api/docs/guides/migrate-to-responses)
- [OpenAI completions-responses-migration-pack](https://github.com/openai/completions-responses-migration-pack)
- [OpenAI Responses vs Chat Completions comparison](https://platform.openai.com/docs/guides/responses-vs-chat-completions)
- [Responses API streaming](https://developers.openai.com/api/docs/guides/streaming-responses)
- [Model Context Protocol Transports spec](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports)
- [MCP stdio transport lifecycle reference (DeepWiki)](https://deepwiki.com/instructkr/claw-code/5.1-mcp-server-lifecycle-and-stdio-transport)
- [python-websockets documentation 16.0](https://websockets.readthedocs.io/en/stable/reference/asyncio/client.html)
- [python-websockets common reconnect pattern (issue #414)](https://github.com/aaugustin/websockets/issues/414)
- [rfc8785 Python library (Trail of Bits)](https://github.com/trailofbits/rfc8785.py)
- [cyberphone/json-canonicalization test vectors](https://github.com/cyberphone/json-canonicalization)

**Security references (HIGH confidence):**
- [EdDSA (Wikipedia) — Ed25519 signing internals](https://en.wikipedia.org/wiki/EdDSA)
- [The Provable Security of Ed25519 (IACR)](https://eprint.iacr.org/2020/823.pdf)
- [Replay attack defenses (Wikipedia)](https://en.wikipedia.org/wiki/Replay_attack)
- [Radicle signed-reference replay vulnerability disclosure (2026-03)](https://radicle.xyz/2026/03/30/disclosure-of-vulnerability-in-signed-references)

**Project context (HIGH confidence):**
- `.planning/PROJECT.md` — v3.3 milestone scope, outstanding tech debt list, last-shipped artifacts
- `.planning/RETROSPECTIVE.md` — v3.1 + v3.2 lessons learned, especially cross-plan ID drift, worktree-merge friction, and checker-loop ROI
- `.planning/milestones/v3.2-MILESTONE-AUDIT.md` — 11-item tech debt inventory with phase attribution + the two XCUITest deferrals (scrollToMessageId timing race, ForEach refresh timing under bulk runs)
- `packages/connector/`, `packages/connector-claude-code/`, `packages/connector-openai/` — off-GSD work to be ratified in Phase 15

**Confidence notes:**
- HIGH: all claims verified against at least one official source or existing project documentation
- MEDIUM/LOW: none in this document — speculative patterns excluded

---

*Pitfalls research for v3.3 Universal Agent Client — connector ecosystem + iOS live discovery + compound memory groundwork + off-GSD ratification + v3.2 tech debt.*
*Researched: 2026-04-18.*
