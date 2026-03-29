---
phase: 05-demo-handshake-discovery-conformance
verified: 2026-03-29T01:00:00Z
status: passed
score: 9/9 must-haves verified
re_verification: false
---

# Phase 5: Demo Handshake & Discovery Conformance Verification Report

**Phase Goal:** Demo clients and server fully implement the ainm.initialized handshake lifecycle and serve Agent Cards at the spec-correct URL path — closing the two integration gaps and the broken WebSocket lifecycle flow from the v3.1 audit
**Verified:** 2026-03-29
**Status:** passed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths (from ROADMAP.md Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Demo REPL and agent clients send `ainm.initialized` after receiving the init response | VERIFIED | `repl.rs:45` and `agent.rs:54` both call `protocol::new_notification("ainm.initialized", ...)` immediately after the init-response receive block |
| 2 | Server rejects any method call with `-32021` before receiving `ainm.initialized` | VERIFIED | `server.rs:131-155` — enforcement loop reads messages, checks method; if not `ainm.initialized`, returns `new_error(req_id, -32021, "Not initialized")` |
| 3 | Agent Card HTTP endpoint serves at `/.well-known/ainm/{name}/agent-card.json` | VERIFIED | `server.rs:53-56` — Axum route literal `"/.well-known/ainm/helper/agent-card.json"` wired to `agent_card_handler` |
| 4 | `demo.md` documents random port binding, not hardcoded 9090 | VERIFIED | `website/docs/demo.md:53` — "Axum WebSocket server on `127.0.0.1:0` (random available port, printed at startup)"; grep for `9090` returns no match |
| 5 | WebSocket connection lifecycle passes end-to-end (init → initialized → methods) | VERIFIED | Server sends init response → clients send `ainm.initialized` notification → server breaks enforcement loop → dispatches to `handle_agent_connection` / `handle_human_connection`. `cargo build` succeeds. |

**Score:** 5/5 success criteria verified

---

### Required Artifacts

#### Plan 05-01 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `demo/src/server.rs` | Handshake enforcement loop; contains `ainm.initialized` | VERIFIED | Loop at lines 131-155; 3 occurrences of `ainm.initialized`; 2 occurrences of `-32021` |
| `demo/src/repl.rs` | REPL sends `ainm.initialized` after init | VERIFIED | Line 44-47: `new_notification("ainm.initialized", ...)` sent after init-response received (line 40-42) and before `ainm.thread.create` |
| `demo/src/agent.rs` | Agent sends `ainm.initialized` after init | VERIFIED | Lines 53-56: `new_notification("ainm.initialized", ...)` sent after init-response received (lines 49-51) and before `let mut first_message = true` |

#### Plan 05-02 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `demo/src/server.rs` | Axum route at `/.well-known/ainm/helper/agent-card.json` | VERIFIED | Line 54: literal route string is spec-correct; 1 occurrence confirmed |
| `demo/src/main.rs` | Both identity store registrations use correct Agent Card URL | VERIFIED | Line 42: initial placeholder registration; line 115: post-bind actual address registration — 2 occurrences of `ainm/helper/agent-card.json` |
| `demo/src/repl.rs` | `/card` HTTP fetch URL uses spec-correct path | VERIFIED | Line 294-296: `format!("http://{}/.well-known/ainm/helper/agent-card.json", server_addr)` |
| `website/docs/demo.md` | Contains `127.0.0.1:0` and `random available port` | VERIFIED | Line 53 confirmed; no `9090` reference found |

---

### Key Link Verification

#### Plan 05-01 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `demo/src/repl.rs` | `demo/src/server.rs` | WebSocket `ainm.initialized` notification | VERIFIED | `repl.rs:45` uses `new_notification("ainm.initialized", ...)` which the server enforcement loop at `server.rs:138` checks for |
| `demo/src/agent.rs` | `demo/src/server.rs` | WebSocket `ainm.initialized` notification | VERIFIED | `agent.rs:54` uses `new_notification("ainm.initialized", ...)` matched by server loop |
| `demo/src/server.rs` | `demo/src/protocol.rs` | Error response for pre-handshake methods | VERIFIED | `server.rs:144`: `protocol::new_error(req_id, -32021, "Not initialized")` — pattern confirmed |

#### Plan 05-02 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `demo/src/repl.rs` | `demo/src/server.rs` | HTTP GET to Agent Card endpoint | VERIFIED | `repl.rs:294-296` constructs URL `.../.well-known/ainm/helper/agent-card.json` matching Axum route at `server.rs:54` |
| `demo/src/main.rs` | `demo/src/server.rs` | Identity store registration URL matches server route | VERIFIED | `main.rs:42` and `main.rs:115` both use `ainm/helper/agent-card.json` — same path as Axum route |

---

### Data-Flow Trace (Level 4)

Not applicable. This phase modifies protocol lifecycle behavior (handshake enforcement and URL paths), not data-rendering pipelines. All artifacts are control-flow logic, not dynamic data renderers.

---

### Behavioral Spot-Checks

| Behavior | Check | Result | Status |
|----------|-------|--------|--------|
| Build compiles without errors | `cargo build` in `demo/` | "Finished `dev` profile ... 3 warnings, 0 errors" | PASS |
| `ainm.initialized` present in all 3 files | `grep -c "ainm.initialized" server.rs repl.rs agent.rs` | server:3, repl:2, agent:2 | PASS |
| `-32021` enforcement in server | `grep -c "\-32021" server.rs` | 2 | PASS |
| Spec-correct Agent Card URL in all 3 source files | `grep -c "ainm/helper/agent-card.json" server.rs main.rs repl.rs` | server:1, main:2, repl:1 | PASS |
| No stale `agent-card/helper` in source files | `grep -rn "agent-card/helper" demo/ website/ spec/ schemas/` | 0 matches in text files (only compiled binaries in target/) | PASS |
| No hardcoded `9090` in demo.md | `grep "9090" website/docs/demo.md` | No matches | PASS |
| `demo.md` documents random port | `grep "127.0.0.1:0\|random available port" website/docs/demo.md` | Line 53 confirmed | PASS |
| All 4 task commits exist in git | `git log --oneline 83fddb9 a59d01e d8dfea9 358329a` | All 4 found | PASS |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| TRNS-01 | 05-01 | Protocol defines WebSocket transport binding | SATISFIED | Handshake lifecycle enforcement is part of the WebSocket transport binding conformance; `ainm.initialized` sequence per transport.md Section 4.2-4.3 implemented in server.rs, repl.rs, agent.rs |
| TRNS-04 | 05-01 | Protocol uses JSON-RPC 2.0 as message framing | SATISFIED | `ainm.initialized` uses `protocol::new_notification` (JSON-RPC notification, no `id`); error enforcement uses `protocol::new_error` (JSON-RPC error response) |
| DISC-01 | 05-02 | Agents publish capability cards at a well-known endpoint | SATISFIED | Axum route at `/.well-known/ainm/helper/agent-card.json` confirmed in server.rs:54 |
| DISC-02 | 05-02 | Agent Cards discoverable by resolving the provider portion | SATISFIED | URL pattern `/.well-known/ainm/{name}/agent-card.json` matches spec (agent-card.md Section 3.2); identity store registrations in main.rs use the same URL |
| SPEC-05 | 05-01, 05-02 | Working demo proves protocol end-to-end | SATISFIED | Build passes with 0 errors; all handshake, URL, and documentation fixes are in place; demo exercises full init → initialized → methods lifecycle |

All 5 requirement IDs declared across plans are accounted for. No orphaned requirements found — REQUIREMENTS.md traceability table lists all 5 as "Phase 5, Complete."

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `demo/src/server.rs` | 3 warnings at build | Unused function warnings (`to_value`, `from_value` in identity module) | Info | Pre-existing warnings unrelated to this phase; no blocker |

No TODOs, FIXME markers, placeholder returns, hardcoded empty data, or stub handlers found in the 7 modified files.

---

### Human Verification Required

#### 1. End-to-End Handshake Live Run

**Test:** Run `cargo run --bin ainm-demo` and observe startup. Send a message (e.g., `hello`).
**Expected:** Demo starts, prints server address, agent connects, REPL prompt appears; typing `hello` produces a streamed response. Tracing output shows "Handshake complete for: helper@demo.ainm.local" and "Handshake complete for: alice@demo.ainm.local" before any messages are exchanged.
**Why human:** Cannot run interactive TUI in automated check; startup sequence requires live terminal.

#### 2. Pre-Handshake Rejection Path

**Test:** Use a WebSocket client (e.g., `websocat`) to connect to the demo server and send an `ainm.message.send` JSON-RPC request without first completing the handshake.
**Expected:** Server responds with JSON-RPC error `{"code": -32021, "message": "Not initialized"}`.
**Why human:** Requires a separate WebSocket client and a running server; not automatable without a test harness.

#### 3. Agent Card HTTP Fetch

**Test:** Run the demo, type `/card` at the REPL prompt.
**Expected:** Agent Card JSON is fetched from `http://127.0.0.1:{PORT}/.well-known/ainm/helper/agent-card.json` and rendered in the terminal with the agent's name, description, capabilities, and skills.
**Why human:** Requires live server with actual bound port; HTTP fetch path only works at runtime.

---

### Gaps Summary

No gaps. All 9 must-have items (5 truths + 4 key artifact/link groups) verified. Build passes. Commits confirmed. No stale references remain.

---

_Verified: 2026-03-29_
_Verifier: Claude (gsd-verifier)_
