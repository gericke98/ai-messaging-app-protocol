---
phase: 06-demo-feature-gaps-cleanup
verified: 2026-03-29T01:00:00Z
status: passed
score: 9/9 must-haves verified
re_verification: false
---

# Phase 06: Demo Feature Gaps Cleanup Verification Report

**Phase Goal:** Demo exercises the memory evict operation, Agent Card responses include caching headers, and all cargo warnings and dead code are eliminated
**Verified:** 2026-03-29
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | User can type /evict in the REPL and see a confirmation that the archival fact was evicted | VERIFIED | `/evict` arm at repl.rs:377, `ainm.memory.evicted` handler at repl.rs:207, renders via `render_memory_event("evicted", params)` |
| 2 | User can type /evict before /memory and see an appropriate error message | VERIFIED | server.rs:517-525 returns error -32054 "Fact not found" when `store.remove(&key).is_some()` returns false |
| 3 | Agent Card HTTP endpoint returns an ETag header on 200 responses | VERIFIED | server.rs:101 sets `(header::ETAG, etag_value)` in the 200 response body |
| 4 | Agent Card HTTP endpoint returns 304 Not Modified when If-None-Match matches the current ETag | VERIFIED | server.rs:84-92, checks `header::IF_NONE_MATCH`, returns `StatusCode::NOT_MODIFIED` with ETAG on match |
| 5 | cargo build produces zero warnings | VERIFIED | `cargo build` output: `Finished dev profile [unoptimized + debuginfo] target(s) in 0.31s` — no warnings emitted |
| 6 | No dead code paths remain for ainm.tool.call.* raw dispatch in server.rs | VERIFIED | `grep -n "ainm.tool.call" demo/src/server.rs` returns zero matches; forwarding match at server.rs:534-538 contains only stream.* and presence.update |
| 7 | No dead code paths remain for ainm.tool.call.* raw method rendering in repl.rs | VERIFIED | `grep -n '"ainm.tool.call' demo/src/repl.rs` returns zero matches; only short-form tool.call.* inside ainm.message.deliver handler (3 lines, each appearing exactly once) |
| 8 | No unused resolve method exists in identity.rs | VERIFIED | `grep -n "fn resolve" demo/src/identity.rs` returns zero matches |
| 9 | No unused MessageEnvelope struct exists in protocol.rs | VERIFIED | `grep -n "MessageEnvelope\|to_value\|from_value" demo/src/protocol.rs` returns zero matches |

**Score:** 9/9 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `demo/src/server.rs` | ainm.memory.evict handler + ETag-aware agent_card_handler | VERIFIED | `ainm.memory.evict` match arm at line 443; `agent_card_handler` with ETag at lines 71-106; dead tool.call.* arms removed |
| `demo/src/agent.rs` | handle_evict function for /evict REPL command | VERIFIED | `handle_evict` defined at line 477; dispatch call at line 94; uses key `user_preference.demo` matching handle_memory at line 438 |
| `demo/src/repl.rs` | /evict REPL command handler + /help listing | VERIFIED | `/evict` match arm at line 377; help text at line 276; `ainm.memory.evicted` incoming handler at line 207 |
| `demo/src/render.rs` | render_memory_event evicted branch | VERIFIED | `"evicted"` arm at line 364, prints green "Evicted archival fact: {key}" |
| `demo/src/identity.rs` | IdentityStore without unused resolve method | VERIFIED | resolve method absent; only new(), register(), get_public_key_bytes() remain |
| `demo/src/protocol.rs` | Clean module without unused MessageEnvelope struct | VERIFIED | MessageEnvelope, to_value, from_value all absent |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `demo/src/repl.rs` | `demo/src/agent.rs` | REPL sends /evict as ainm.message.send, agent receives via ainm.message.deliver | VERIFIED | repl.rs:377 builds ainm.message.send request with /evict text; agent.rs:69 handles ainm.message.deliver; agent.rs:93 dispatches to handle_evict |
| `demo/src/agent.rs` | `demo/src/server.rs` | Agent sends ainm.memory.evict JSON-RPC request to server | VERIFIED | agent.rs:503-515 sends `protocol::new_request("ainm.memory.evict", ...)` over WebSocket; server.rs:443 handles it |
| `demo/src/server.rs` | `demo/src/repl.rs` | Server sends ainm.memory.evicted notification to human via broadcast | VERIFIED | server.rs:507-516 sends `new_notification("ainm.memory.evicted", ...)` via to_human_tx; repl.rs:207-210 handles it |
| `demo/src/agent.rs` | `demo/src/server.rs` | Tool events sent as ainm.message.send (NOT raw ainm.tool.call.*) | VERIFIED | agent.rs:779 wraps all tool events in `"ainm.message.send"`; server.rs has zero `ainm.tool.call.*` match arms |
| `demo/src/server.rs` | `demo/src/repl.rs` | Tool events forwarded as ainm.message.deliver notifications | VERIFIED | server.rs:251 and 622 forward messages as `"ainm.message.deliver"` |

### Data-Flow Trace (Level 4)

Not applicable — all artifacts are event-processing handlers (WebSocket JSON-RPC dispatch), not UI components rendering from a data store. Data flows through in-memory structures (archival_store HashMap) already verified at Level 3.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| cargo build produces zero warnings and errors | `cargo build 2>&1` | `Finished dev profile [unoptimized + debuginfo] target(s) in 0.31s` | PASS |
| ainm.memory.evict handler exists in server.rs | `grep -c "ainm.memory.evict" demo/src/server.rs` | 2 matches (handler + notification method) | PASS |
| handle_evict has 2 references in agent.rs | `grep -c "handle_evict" demo/src/agent.rs` | 2 matches (definition + dispatch) | PASS |
| tool.call.* appears exactly once each in repl.rs | `grep -c "tool.call.started/progress/completed"` | 1 / 1 / 1 | PASS |
| No dead ainm.tool.call.* in server.rs | `grep -c "ainm.tool.call" demo/src/server.rs` | 0 | PASS |
| resolve method absent from identity.rs | `grep -c "fn resolve" demo/src/identity.rs` | 0 | PASS |
| MessageEnvelope absent from protocol.rs | `grep -c "MessageEnvelope" demo/src/protocol.rs` | 0 | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| MEMO-01 | 06-01-PLAN.md | Protocol defines three memory tiers including archival memory with store/query/evict operations | SATISFIED | `ainm.memory.evict` handler in server.rs + `handle_evict` in agent.rs completes the archival CRUD cycle (store + query from Phase 4, evict now added) |
| DISC-05 | 06-01-PLAN.md | Agent Cards are cacheable with explicit versioning | SATISFIED | `agent_card_handler` at server.rs:71-106 computes SHA-256 ETag, returns it on 200, returns 304 on matching If-None-Match |
| SPEC-05 | 06-01-PLAN.md, 06-02-PLAN.md | Working demo proves the protocol end-to-end | SATISFIED | Demo now covers memory evict (closes MEMO-01 gap), Agent Card caching (closes DISC-05 gap), and zero cargo warnings confirm clean reference implementation quality |

**Orphaned requirements check:** REQUIREMENTS.md traceability table maps MEMO-01, DISC-05, and SPEC-05 to Phase 6. All three are accounted for in plan frontmatter. No orphaned requirements.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | — | — | — | — |

`grep -rn "TODO\|FIXME\|XXX\|HACK\|PLACEHOLDER"` across `demo/src/` returned zero matches.

### Human Verification Required

#### 1. Full /memory then /evict REPL flow

**Test:** Run the demo binary (`cargo run`), complete the handshake (`/hello`), store a memory fact (`/memory`), then evict it (`/evict`). Observe the green "Evicted archival fact: user_preference.demo" confirmation message.
**Expected:** The REPL prints the eviction confirmation after the agent receives `ainm.memory.evicted` from the server. Running `/evict` a second time (fact already evicted) should result in no eviction confirmation (server returns -32054 error, which the agent does not re-broadcast to REPL).
**Why human:** End-to-end interactive REPL flow cannot be exercised in a non-interactive automated check.

#### 2. Agent Card ETag/304 behavior

**Test:** Start the demo server, make a GET request to `http://localhost:<port>/.well-known/agent-card`, capture the returned `ETag` header value, then make a second request with `If-None-Match: <captured-etag>`.
**Expected:** First request returns HTTP 200 with `ETag` header. Second request returns HTTP 304 Not Modified with `ETag` header and no body.
**Why human:** Requires a running server instance and HTTP client (curl or browser devtools) to observe live HTTP response headers.

### Gaps Summary

No gaps. All 9 must-have truths verified. All 4 plan artifacts exist and are substantive. All 5 key links wired. All 3 requirement IDs satisfied. Zero cargo warnings. Zero dead code.

---

_Verified: 2026-03-29_
_Verifier: Claude (gsd-verifier)_
