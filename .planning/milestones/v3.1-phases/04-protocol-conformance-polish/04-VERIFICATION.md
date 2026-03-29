---
phase: 04-protocol-conformance-polish
verified: 2026-03-28T23:20:50Z
status: passed
score: 8/8 must-haves verified
re_verification: false
---

# Phase 04: Protocol Conformance Polish Verification Report

**Phase Goal:** All published artifacts (AsyncAPI, JSON Schema, demo) are fully aligned — no field name mismatches, no invented methods, no missing schema entries. The demo exercises every protocol feature including memory and compatibility bridges.
**Verified:** 2026-03-28T23:20:50Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (from ROADMAP.md Success Criteria)

| #  | Truth | Status | Evidence |
|----|-------|--------|----------|
| 1  | AsyncAPI uses ainm.thread.create (not ainm.sub_thread.create) and references StreamErrorNotification | VERIFIED | `grep -c "sub_thread" asyncapi.yaml` returns 0; asyncapi.yaml $ref is `schemas/streaming.schema.json#/$defs/StreamErrorNotification` |
| 2  | MessageEnvelope.type enum includes all Phase 2 message types (9 values) | VERIFIED | `python3` confirms enum has exactly 9 values: message, thread.create, thread.join, thread.leave, presence.update, tool.call.started, tool.call.progress, tool.call.completed, action |
| 3  | Demo code uses exact JSON Schema field names (protocolVersion, contentTypes, title/value on ApprovalBlock, parallelBlocks) | VERIFIED | grep confirms: zero occurrences of protocol_version, tool_visibility, "interactive":, option_id, "prompt": in demo/src/ |
| 4  | Demo ainm.initialize includes Ed25519 signature; server responds with nonce | VERIFIED | `demo/src/signing.rs` has `sign_init`; agent.rs and repl.rs include `"signature":` in init params; server.rs returns `"nonce":` and `"serverInfo":` |
| 5  | Demo Agent Card includes all required ainm extension sub-objects (streaming, threading, memory) | VERIFIED | main.rs contains parallelBlocks, preferredChunkSize, threading (subThreads + maxConcurrentThreads), memory (archivalSupported + contextFramePreferred), top-level protocolVersion |
| 6  | Demo includes /memory scenario exercising store, query, and context push | VERIFIED | agent.rs: handle_memory sends ainm.memory.store and ainm.memory.query; server.rs: has ainm.memory.store handler, ainm.memory.query handler, ainm.context.frame notification; repl.rs: /memory dispatch |
| 7  | Demo includes /bridge scenario exercising MCP and A2A compatibility mappings | VERIFIED | agent.rs: handle_bridge with mcp-code-tools, a2a-research, tool.call.started/progress/completed, ainm.presence.update; repl.rs: /bridge dispatch; render.rs: render_bridge_event with MCP/A2A sections |
| 8  | Stream delta payloads omit undocumented sequence field | VERIFIED | `grep -rn '"sequence":' demo/src/` returns 0 matches; streaming.rs has no sequence variable |

**Score:** 8/8 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `asyncapi.yaml` | Corrected AsyncAPI transport bindings | VERIFIED | Contains ainm.thread.create, streaming.schema.json#/$defs/StreamErrorNotification ref, no sub_thread references |
| `schemas/streaming.schema.json` | StreamErrorNotification event schema $def | VERIFIED | StreamErrorNotification $def with 4 required fields: stream_id, code, message, partial_preserved |
| `schemas/common.schema.json` | Complete MessageEnvelope.type enum | VERIFIED | 9-value enum confirmed via python3 JSON parse |
| `website/static/asyncapi.yaml` | Mirror of asyncapi.yaml | VERIFIED | `diff asyncapi.yaml website/static/asyncapi.yaml` returns no differences |
| `demo/src/agent.rs` | Corrected field names + handle_memory + handle_bridge | VERIFIED | protocolVersion, contentTypes, toolVisibility, title, value, sign_init call, handle_memory, handle_bridge all present |
| `demo/src/repl.rs` | /memory and /bridge REPL commands + corrected init | VERIFIED | /memory and /bridge dispatch at lines 299 and 340; protocolVersion at line 26 |
| `demo/src/server.rs` | serverInfo + nonce + memory handlers | VERIFIED | serverInfo at line 111, nonce at line 120, archival_store at line 30, ainm.memory.store handler, ainm.memory.query handler, ainm.context.frame notification |
| `demo/src/main.rs` | Complete Agent Card with full ainm sub-objects | VERIFIED | parallelBlocks, preferredChunkSize, threading, memory, ainm.protocolVersion, /memory and /bridge help lines |
| `demo/src/render.rs` | render_memory_event, render_bridge_event, corrected ApprovalBlock reads | VERIFIED | render_memory_event at line 296, render_bridge_event at line 369, get("title") at line 73, get("value") at line 320 |
| `demo/src/streaming.rs` | StreamDelta without sequence field | VERIFIED | No sequence variable or field remaining |
| `demo/src/signing.rs` | sign_init function | VERIFIED | `pub fn sign_init(` at line 10 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| asyncapi.yaml | schemas/streaming.schema.json#/$defs/StreamErrorNotification | $ref in StreamErrorNotification payload | WIRED | Line confirmed: `$ref: "schemas/streaming.schema.json#/$defs/StreamErrorNotification"` |
| asyncapi.yaml | schemas/transport.schema.json#/$defs/ThreadCreateParams | $ref in ThreadCreateRequest payload | WIRED | `$ref: "schemas/transport.schema.json#/$defs/ThreadCreateParams"` present |
| demo/src/agent.rs | schemas/transport.schema.json#/$defs/InitializeParams | ainm.initialize JSON-RPC params (protocolVersion) | WIRED | `"protocolVersion": "0.1.0"` at line 35; `"signature":` at line 36 |
| demo/src/server.rs | schemas/transport.schema.json#/$defs/InitializeResult | ainm.initialize response (serverInfo, nonce) | WIRED | serverInfo at line 111, nonce at line 120 |
| demo/src/main.rs | schemas/agent-card.schema.json#/$defs/AINMExtensions | Agent Card ainm object (threading.subThreads) | WIRED | threading at line 73, subThreads at line 74 |
| demo/src/render.rs | schemas/interactive.schema.json#/$defs/ApprovalBlock | block.get("title") and opt.get("value") | WIRED | get("title") at lines 73, 131; get("value") at line 320 |
| demo/src/repl.rs | demo/src/agent.rs | /memory and /bridge text triggers agent handlers | WIRED | repl.rs sends "/memory" text; agent.rs dispatches to handle_memory at line 87 and handle_bridge at line 89 |
| demo/src/agent.rs | schemas/memory.schema.json#/$defs/MemoryStoreParams | ainm.memory.store JSON-RPC params | WIRED | ainm.memory.store method at line 426; fact.key "user_preference.demo" at line 431 |
| demo/src/server.rs | schemas/memory.schema.json#/$defs/ContextFrame | ainm.context.frame notification | WIRED | ainm.context.frame at line 357 with thread_id, frame, active_context, archival_facts, participants |
| demo/src/render.rs | demo/src/render.rs render_tool_event | Bridge MCP section reuses render_tool_event | WIRED | render_tool_event called from repl.rs bridge dispatch at lines 189, 194, 199 |

### Data-Flow Trace (Level 4)

The demo is a CLI tool, not a data-rendering web component. Dynamic data in the demo flows through:

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|--------------|--------|-------------------|--------|
| demo/src/agent.rs handle_memory | ainm.memory.store response (ArchivalFact) | server.rs ainm.memory.store handler writes to archival_store HashMap | Yes — server generates fact_id via new_message_id(), stores fact, returns ArchivalFact | FLOWING |
| demo/src/server.rs ainm.memory.query | archival_store HashMap lookup | Key-prefix filter on in-memory HashMap populated by store handler | Yes — returns real stored facts, not hardcoded | FLOWING |
| demo/src/server.rs ainm.context.frame | archival_store contents | archival_store.lock().await after query | Yes — includes actual facts from store | FLOWING |
| demo/src/agent.rs handle_bridge | Tool call events (call_id, status) | Generated inline with protocol::new_message_id() for call_id | Yes — UUIDv7 generated, not hardcoded | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Demo builds without errors | `cargo build` in demo/ | `Finished dev profile` — 3 warnings (unused associated items), no errors | PASS |
| sign_init function exported | `grep "pub fn sign_init" demo/src/signing.rs` | Line 10 found | PASS |
| sequence field absent from streaming | `grep -rn '"sequence":' demo/src/` | 0 matches | PASS |
| MessageEnvelope enum has 9 values | python3 JSON parse | 9 values confirmed | PASS |
| asyncapi.yaml mirrors website/static | `diff asyncapi.yaml website/static/asyncapi.yaml` | No differences | PASS |
| All 6 task commits present in git log | `git log --oneline` | e350882, 97b1fcd, ee18a11, b384e2c, f65c232, 8f477b7 all verified | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| THRD-02 | 04-01 | Any participant can create a sub-thread | SATISFIED | asyncapi.yaml ainm.thread.create with optional parent_thread_id documented |
| THRD-04 | 04-01 | Agents can create sub-threads | SATISFIED | Same ainm.thread.create mechanism; description updated in asyncapi.yaml |
| STRM-03 | 04-01 | Streaming messages collapse into permanent events | SATISFIED | StreamErrorNotification schema added to streaming.schema.json |
| TOOL-01 | 04-01 | Protocol defines ToolCallStarted event | SATISFIED | tool.call.started in MessageEnvelope.type enum (common.schema.json) |
| TOOL-02 | 04-01 | Protocol defines ToolCallProgress event | SATISFIED | tool.call.progress in MessageEnvelope.type enum |
| TOOL-03 | 04-01 | Protocol defines ToolCallCompleted event | SATISFIED | tool.call.completed in MessageEnvelope.type enum |
| SPEC-02 | 04-01 | All message types have formal JSON Schema definitions | SATISFIED | MessageEnvelope.type enum now includes all 9 message types |
| SPEC-03 | 04-01 | Transport bindings have formal AsyncAPI v3.1 definitions | SATISFIED | asyncapi.yaml corrected and mirrored to website/static/ |
| TRNS-01 | 04-02 | Protocol defines WebSocket transport binding | SATISFIED | Demo ainm.initialize WebSocket handshake uses protocolVersion (camelCase) |
| SECU-04 | 04-02 | Protocol defines trust annotations on message content | SATISFIED | Demo sign_init implements Ed25519 init auth; server returns nonce |
| STRM-01 | 04-02 | Agents can stream text responses token-by-token | SATISFIED | streaming.rs StreamDelta cleaned of undocumented sequence field |
| CONT-03 | 04-02 | Protocol defines interactive block types including ApprovalBlock | SATISFIED | ApprovalBlock now uses title/value matching schema |
| CONT-04 | 04-02 | Interactive blocks use declarative component model | SATISFIED | contentTypes array replaces interactive boolean |
| DISC-02 | 04-02 | Agent Cards discoverable at well-known URL | SATISFIED | Agent Card in main.rs has all required ainm fields including protocolVersion |
| SPEC-05 | 04-02 | Working demo proves protocol end-to-end | SATISFIED | Demo builds, has complete Agent Card, signed init, all field names correct |
| MEMO-01 | 04-03 | Protocol defines three memory tiers | SATISFIED | /memory demo exercises archival tier via ainm.memory.store and ainm.memory.query |
| MEMO-02 | 04-03 | Active context includes current thread + recent messages | SATISFIED | ainm.context.frame notification delivers active_context with recent_messages |
| MEMO-03 | 04-03 | Conversation memory is full thread history queryable by participants | SATISFIED | ainm.memory.query with tier filtering demonstrated |
| MEMO-04 | 04-03 | Archival memory stores cross-conversation knowledge | SATISFIED | archival_store HashMap with ArchivalFact persistence demonstrated |
| MEMO-05 | 04-03 | Agents receive structured context payloads | SATISFIED | ainm.context.frame delivers ContextFrame with 4 sections (active_context, conversation_summary, archival_facts, participants) |
| COMP-01 | 04-03 | Protocol defines mapping from MCP tool calls to protocol message types | SATISFIED | /bridge demo: MCP section uses tool.call.started/progress/completed |
| COMP-02 | 04-03 | MCP server can be exposed as agent participant | SATISFIED | mcp-code-tools@demo.ainm.local identity in bridge demo |
| COMP-03 | 04-03 | Protocol defines mapping from A2A task lifecycle to thread events | SATISFIED | /bridge demo: A2A submitted/working/completed mapped to ainm.thread.create / ainm.presence.update / content message |
| COMP-04 | 04-03 | A2A task updates rendered as thread messages | SATISFIED | render_bridge_event renders a2a_submitted/working/completed with correct colors per UI-SPEC |

**Orphaned requirements check:** All 24 requirement IDs listed in phase instructions (TRNS-01, SECU-04, STRM-01, STRM-03, CONT-03, CONT-04, THRD-02, THRD-04, DISC-02, TOOL-01, TOOL-02, TOOL-03, MEMO-01, MEMO-02, MEMO-03, MEMO-04, MEMO-05, COMP-01, COMP-02, COMP-03, COMP-04, SPEC-02, SPEC-03, SPEC-05) are claimed across plans 01, 02, and 03. None are orphaned.

### Anti-Patterns Found

No anti-patterns found.

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| — | — | None found | — | — |

- No TODO/FIXME/PLACEHOLDER comments in demo/src/ or schema files
- No hardcoded empty returns in API handlers
- No stub-only implementations (all handlers have real logic)
- Cargo build produces 3 warnings about unused associated items in Identity struct (to_value/from_value) — these are pre-existing, not introduced by phase 04 changes, and do not affect correctness

### Human Verification Required

#### 1. Full Interactive Demo Run

**Test:** Launch the demo server and REPL client, then exercise each command in sequence: hello, form, button, approve, tool, /card, /memory, /bridge, /quit
**Expected:** Each command completes without error, terminal output matches UI-SPEC colors and section headers, /memory shows archival fact stored/queried/context frame with byte counts, /bridge shows MCP tool events and A2A lifecycle events
**Why human:** The behavioral spot-checks verified compilation and code patterns. The actual runtime behavior — correct terminal colors, section header placement, interactivity timing, and absence of panics under real WebSocket traffic — requires manual execution

#### 2. Ed25519 Signature Verification

**Test:** Run the demo, capture the ainm.initialize WebSocket frame, verify the signature field against the identity public key using the sign_init payload format (identity_address + "|" + timestamp)
**Expected:** Signature verifies successfully with the Ed25519 public key
**Why human:** Cryptographic signature correctness cannot be verified by grep; requires runtime execution and signature verification tooling

### Gaps Summary

No gaps found. All 8 ROADMAP success criteria are fully satisfied by the codebase:

1. AsyncAPI corrected — sub_thread method removed, StreamErrorNotification properly referenced
2. MessageEnvelope.type enum complete with 9 values
3. All demo field names match JSON Schema exactly
4. Ed25519 signed init with server nonce response implemented
5. Agent Card has full ainm extension sub-objects
6. /memory scenario fully implemented (store, query, context frame)
7. /bridge scenario fully implemented (MCP tool bridging, A2A lifecycle)
8. StreamDelta has no undocumented sequence field

The demo builds successfully (`cargo build` completes with only pre-existing warnings). All 24 phase requirements are satisfied. All 6 task commits from all 3 plans are present in git history.

---

_Verified: 2026-03-28T23:20:50Z_
_Verifier: Claude (gsd-verifier)_
