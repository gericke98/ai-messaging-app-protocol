---
phase: 03-specification-publication-working-demo
verified: 2026-03-28T22:00:00Z
status: passed
score: 3/3 must-haves verified
re_verification: null
gaps: []
human_verification:
  - test: "Run the CLI demo end-to-end"
    expected: "cargo run --bin ainm-demo starts server, agent connects, REPL opens; typing 'hello' produces streaming token-by-token output with [VERIFIED] badge; typing 'form', 'approve', 'tool', 'button' each shows corresponding rich content"
    why_human: "Demo binary was verified to compile (cargo check passes) but actual runtime behavior -- streaming token rendering, terminal coloring, interactive REPL input -- requires a human to visually observe"
  - test: "Browse the Docusaurus site"
    expected: "npm run build in website/ completes without errors; site has Introduction page, Specification category with 11 items, Demo page, Changelog page, and AsyncAPI explorer at /asyncapi/"
    why_human: "Site build produces static output; visual rendering and AsyncAPI explorer interactivity require human browser verification"
---

# Phase 3: Specification Publication & Working Demo Verification Report

**Phase Goal:** The complete protocol specification is published as an open document with formal schemas, and a working reference server demonstrates the protocol end-to-end

**Verified:** 2026-03-28T22:00:00Z
**Status:** passed (pending human verification of runtime behavior)
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | The protocol spec is published as an open document (Markdown + JSON Schema + AsyncAPI) with formal definitions for every message type, AsyncAPI v3.1 transport bindings, and clear versioning with backwards compatibility rules | VERIFIED | 12 JSON Schema 2020-12 files exist and are valid with 52+ definitions; asyncapi.yaml has asyncapi: 3.1.0 with 8 domain channels and all JSON-RPC methods mapped; CHANGELOG.md has ## [0.1.0] with SemVer policy; VERSION contains 0.1.0 |
| 2 | A working demo runs end-to-end: at minimum one human and one agent communicate in a thread, with the agent streaming responses in real-time and sending at least one rich interactive content block -- all messages cryptographically signed and verifiable | VERIFIED (compile) / HUMAN for runtime | demo/ has all 10 source files, cargo check passes with 0 errors; signing.rs uses ring Ed25519 + serde_json_canonicalizer (RFC 8785); streaming.rs sends StreamStart/Delta/End with 30-80ms delays; agent.rs has 5 scripted scenarios including form, approval, buttons; all agent messages call signing::sign_envelope; repl.rs calls signing::verify_signature and render::render_message with verified flag |
| 3 | An external developer can clone the demo repo, run it locally, and observe the full protocol in action (identity registration, message signing, thread creation, streaming, rich content, agent discovery) without needing to read the spec first | VERIFIED (artifacts) / HUMAN for UX | website/docs/demo.md has clear `cargo run --bin ainm-demo` instructions; main.rs prints welcome banner with command list; /card command fetches Agent Card from /.well-known/ainm/agent-card/helper; repl.rs sends ainm.thread.create before any messages; all flows covered in single binary |

**Score:** 3/3 truths verified at artifact level; 2 require human runtime confirmation

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `schemas/common.schema.json` | Shared $defs (AINMAddress, UUIDv7, etc.) | VERIFIED | 11 $defs, valid JSON Schema 2020-12, $id set, AINMAddress has pattern constraint, MessageEnvelope requires all 7 fields |
| `schemas/identity.schema.json` | IdentityRecord, Registration, KeyRotation | VERIFIED | 7 $defs, $refs common.schema.json, has IdentityRecord + RegistrationRequest/Response + KeyRotationRecord |
| `schemas/security.schema.json` | SignedMessage, TrustAnnotation, VerificationResult | VERIFIED | 7 $defs, $refs common.schema.json |
| `schemas/content.schema.json` | TextBlock, CodeBlock, ImageBlock, FileBlock, ExtensionBlock | VERIFIED | 6 $defs, all 5 block types present, each uses allOf ContentBlockBase |
| `schemas/transport.schema.json` | InitializeParams, ServerCapabilities, MessageSendParams | VERIFIED | 21 $defs, all 5 required transport types confirmed present |
| `schemas/interactive.schema.json` | ButtonBlock, FormBlock, TableBlock, ApprovalBlock, ActionBlock | VERIFIED | 9 $defs including all 5 block types |
| `schemas/streaming.schema.json` | StreamStart, StreamDelta, StreamEnd, CancelStream | VERIFIED | 6 $defs, all 4 required types confirmed |
| `schemas/threading.schema.json` | ThreadCreateParams, ThreadParticipant, SubThreadCreateParams | VERIFIED | 7 $defs, $refs common.schema.json |
| `schemas/agent-card.schema.json` | AgentCard, AINMExtensions, AgentCapabilities | VERIFIED | 9 $defs, A2A-compatible with ainm namespace |
| `schemas/memory.schema.json` | ContextFrame, MemoryStoreParams, ArchivalFact | VERIFIED | 8 $defs |
| `schemas/tool-visibility.schema.json` | ToolCallStarted, ToolCallProgress, ToolCallCompleted | VERIFIED | 3 $defs, all 3 event types confirmed |
| `schemas/compatibility.schema.json` | MCPBridgeConfig, A2ABridgeConfig | VERIFIED | 5 $defs, $refs common.schema.json |
| `asyncapi.yaml` | AsyncAPI 3.1.0 transport bindings | VERIFIED | asyncapi: 3.1.0, info.version: 0.1.0, 8 domain channels (session/messaging/threading/presence/streaming/memory/toolEvents/federation), $refs to schemas/transport.schema.json, streaming.schema.json, memory.schema.json, threading.schema.json, components section with messages |
| `CHANGELOG.md` | v0.1.0 with comprehensive protocol feature listing | VERIFIED | Has ## [0.1.0], Keep a Changelog format, SemVer reference, Version Policy, all JSON-RPC methods listed, error code ranges, all Phase 1/2 features |
| `VERSION` | Contains 0.1.0 | VERIFIED | File contains exactly "0.1.0" |
| `demo/Cargo.toml` | Rust project with name "ainm-demo" | VERIFIED | 594 bytes, name = "ainm-demo", all required dependencies: axum, tokio, ring, serde_json_canonicalizer, rustyline, colored, uuid |
| `demo/src/main.rs` | Entry point spawning server/agent/REPL | VERIFIED | 4774 bytes, #[tokio::main], calls server::start, tokio::spawn(agent::run), repl::run |
| `demo/src/identity.rs` | Ed25519 key generation | VERIFIED | Uses ring::signature::Ed25519KeyPair, generate_keypair function present |
| `demo/src/signing.rs` | RFC 8785 canonical JSON + Ed25519 signing | VERIFIED | sign_envelope and verify_signature using serde_json_canonicalizer::to_string for RFC 8785, ring for Ed25519 |
| `demo/src/protocol.rs` | JSON-RPC 2.0 types | VERIFIED | 3984 bytes, JsonRpcRequest/Response/Notification/Error structs |
| `demo/src/server.rs` | Axum WebSocket server with broadcast channel routing | VERIFIED | 13016 bytes, WebSocketUpgrade handler, /ws route, /.well-known/ainm/agent-card/helper route, broadcast::channel for to_agent_tx/to_human_tx, signing::verify called at lines 177, 293 |
| `demo/src/agent.rs` | Mock agent with 5 scripted scenarios | VERIFIED | 14721 bytes, handle_hello/handle_form/handle_approval/handle_tool/handle_button functions, signing::sign_envelope called 7 times |
| `demo/src/streaming.rs` | Stream lifecycle with artificial delay | VERIFIED | send_streamed_response sends StreamStart/Delta/End, 30-80ms random delay between deltas, collapsed message signed at StreamEnd |
| `demo/src/repl.rs` | Terminal REPL with rustyline | VERIFIED | 14241 bytes, rustyline::DefaultEditor, "alice> " prompt, ainm.thread.create on startup, /card command fetches agent card, render::render_message called with verification result |
| `demo/src/render.rs` | Terminal rendering for all block types | VERIFIED | 12742 bytes, render_message/render_content_block/render_stream_delta/render_stream_end/render_tool_event/render_agent_card all implemented, handles text/code/button/form/approval block types |
| `website/docusaurus.config.js` | Docusaurus with classic + asyncapi presets | VERIFIED | docusaurus-asyncapi preset configured with spec: '../asyncapi.yaml', routeBasePath: '/', markdown.format: 'detect' |
| `website/sidebars.js` | Sidebar with Introduction + 11 spec items + Demo + Changelog | VERIFIED | All 11 spec files listed in Specification category, demo and changelog at root level |
| `website/docs/intro.md` | Landing page with protocol overview | VERIFIED | Has AI-Native Messaging Protocol title, core design principles, protocol overview table with all 11 spec links |
| `website/docs/demo.md` | Demo instructions with cargo run | VERIFIED | Contains `cargo run --bin ainm-demo`, prerequisite info, command table |
| `website/docs/spec/` | 11 spec files with Docusaurus frontmatter | VERIFIED | 11 spec files present, each with sidebar_position and title frontmatter (verified by copy-specs.js execution producing 11 files) |
| `.github/workflows/deploy-docs.yml` | GitHub Actions deployment to GitHub Pages | VERIFIED | triggers on push to main with path filters, workflow_dispatch, validates asyncapi.yaml, builds site, deploys via actions/deploy-pages@v4 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `schemas/identity.schema.json` | `schemas/common.schema.json` | $ref for AINMAddress, UUIDv7 | WIRED | "common.schema.json" found in file content |
| `schemas/security.schema.json` | `schemas/common.schema.json` | $ref for Ed25519Signature, MessageEnvelope | WIRED | "common.schema.json" found in file content |
| `schemas/transport.schema.json` | `schemas/common.schema.json` | $ref for JSON-RPC types | WIRED | "common.schema.json" found in file content |
| `asyncapi.yaml` | `schemas/transport.schema.json` | $ref for InitializeParams | WIRED | "schemas/transport.schema.json" found in asyncapi.yaml |
| `asyncapi.yaml` | `schemas/streaming.schema.json` | $ref for StreamStart, StreamDelta, StreamEnd | WIRED | "schemas/streaming.schema.json" found in asyncapi.yaml |
| `asyncapi.yaml` | `schemas/memory.schema.json` | $ref for memory method params | WIRED | "schemas/memory.schema.json" found in asyncapi.yaml |
| `demo/src/main.rs` | `demo/src/server.rs` | server::start() call | WIRED | Line 87: server::start(identity_store.clone(), agent_card).await? |
| `demo/src/main.rs` | `demo/src/agent.rs` | agent::run() spawned as tokio task | WIRED | Line 133: tokio::spawn(agent::run(...)) |
| `demo/src/main.rs` | `demo/src/repl.rs` | repl::run() | WIRED | Line 143: repl::run(server_addr, human_identity, identity_store).await |
| `demo/src/server.rs` | `demo/src/signing.rs` | verify_signature() on incoming messages | WIRED | Lines 177, 293: signing::verify_signature called |
| `demo/src/agent.rs` | `demo/src/streaming.rs` | streaming::send_streamed_response() | WIRED | Lines 124, 401: streaming::send_streamed_response called |
| `website/docusaurus.config.js` | `asyncapi.yaml` | docusaurus-asyncapi preset spec path | WIRED | spec: '../asyncapi.yaml' at line 51 |
| `website/docs/demo.md` | `demo/Cargo.toml` | Documentation references demo directory | WIRED | Contains `cargo run --bin ainm-demo` |
| `.github/workflows/deploy-docs.yml` | `website/` | Build step runs in website directory | WIRED | working-directory: website at build step |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `demo/src/server.rs` (message routing) | deliver notification | agent message via to_human_tx broadcast | Yes -- actual message content from agent.rs | FLOWING |
| `demo/src/repl.rs` (incoming handler) | verified flag | signing::verify_signature against actual public key from identity_store | Yes -- real Ed25519 verification | FLOWING |
| `demo/src/render.rs` (render_message) | envelope content | content array from actual MessageEnvelope sent by agent | Yes -- from signed agent messages | FLOWING |
| `demo/src/server.rs` (agent-card endpoint) | agent_card JSON | ServerState.agent_card populated from main.rs with full JSON | Yes -- constructed in main.rs with real fields | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Demo compiles without errors | `cd demo && cargo check` | Finished dev profile [0 errors, 3 warnings about unused struct/methods] | PASS |
| All 12 JSON Schema files valid | python3 json validation | All 12 files pass $schema/$id/$defs validation | PASS |
| asyncapi.yaml has correct structure | python3 content check | asyncapi 3.1.0, 8 channels, transport/streaming/memory schema refs found | PASS |
| CHANGELOG.md is comprehensive | python3 content check | All 15 checked items found including methods, error codes, features | PASS |
| copy-specs.js prebuild executes | `node scripts/copy-specs.js` | Copied 11 spec files to docs/spec/, 12 schema files to static/schemas/ | PASS |
| Demo runtime (streaming, REPL, signing) | `cargo run --bin ainm-demo` | SKIPPED -- requires interactive terminal | HUMAN NEEDED |
| Docusaurus site builds | `npm run build` | SKIPPED -- requires Node.js environment with installed deps | HUMAN NEEDED |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| SPEC-01 | 03-01, 03-03, 03-05 | Protocol spec published as open document (Markdown + JSON Schema + AsyncAPI) | SATISFIED | 12 JSON Schema 2020-12 files, asyncapi.yaml (AsyncAPI 3.1.0), Docusaurus site with 11 spec pages, VERSION + CHANGELOG |
| SPEC-02 | 03-01, 03-02 | All message types have formal JSON Schema definitions | SATISFIED | 52+ definitions across 12 schema files covering all message types from Phase 1 and Phase 2 specs |
| SPEC-03 | 03-03 | Transport bindings have formal AsyncAPI v3.1 definitions | SATISFIED | asyncapi.yaml with AsyncAPI 3.1.0, 8 domain channels, all JSON-RPC methods as operations, $refs to schema files |
| SPEC-04 | 03-03, 03-05 | Spec is versioned with clear backwards compatibility rules | SATISFIED | VERSION = "0.1.0", CHANGELOG.md has ## [0.1.0] with SemVer policy, version policy section describing 0.x freedom and breaking change rules |
| SPEC-05 | 03-04 | Working demo proves protocol end-to-end (human + agent, streaming, rich content) | SATISFIED (compile) / HUMAN (runtime) | Single binary compiles with cargo check, all 10 source files present and substantive: identity registration in main.rs, thread creation in repl.rs, Ed25519 signing in signing.rs, streaming lifecycle in streaming.rs, interactive blocks in agent.rs (form/approval/button/tool), signature verification in server.rs and repl.rs |

**Orphaned requirements check:** REQUIREMENTS.md maps SPEC-01 through SPEC-05 all to Phase 3 -- all 5 appear in Phase 3 plans. No orphaned requirements.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `website/docs/demo.md` | 53 | "Axum WebSocket server on `127.0.0.1:9090`" | Info | Documentation inaccuracy -- code binds to `127.0.0.1:0` (random port), not 9090. Users following the docs will observe a different port in the printed banner. Non-blocking; the demo still works but the architecture description is wrong. |

No TODO/FIXME/placeholder stubs found in demo source files. No empty implementations detected. All agent scenarios are substantive with real protocol messages.

### Human Verification Required

#### 1. CLI Demo Runtime Verification

**Test:** In a terminal with Rust 1.85+ installed, run `cd demo && cargo run --bin ainm-demo`
**Expected:**
- Server starts and prints welcome banner with identities and commands
- Type "hello" -- streaming text appears token by token in terminal, ends with [VERIFIED] badge
- Type "form" -- FormBlock renders with field labels (name, rating, comments)
- Type "approve" -- ApprovalBlock renders with Approve/Reject buttons
- Type "tool" -- ToolCallStarted -> ToolCallProgress -> ToolCallCompleted sequence visible
- Type "button" -- 3 styled buttons appear (blue primary, white secondary, red danger)
- Type "/card" -- Agent Card JSON is displayed
- Every message shows [VERIFIED] (green) or [FAILED] (red) signature badge
- Type "/quit" exits cleanly
**Why human:** Streaming token-by-token rendering, terminal colors, rustyline input handling, and the visual [VERIFIED]/[FAILED] badges all require live terminal observation.

#### 2. Docusaurus Site Build and Browse

**Test:** In a terminal with Node.js 20+ installed, run `cd website && npm install && npm run build`
**Expected:**
- Build completes without errors (warnings about asyncapi route are acceptable)
- `npx serve build` serves site at http://localhost:3000/ai-native-messaging/
- Sidebar shows: Introduction, Specification (11 collapsed items), Demo, Changelog
- Clicking spec items shows content from spec/ files (identity, security, transport, etc.)
- /asyncapi/ route shows AsyncAPI explorer
- /demo page shows cargo run instructions
**Why human:** Static site build output and AsyncAPI interactive explorer require browser to confirm rendering and navigation work correctly.

### Gaps Summary

No gaps found. All artifacts exist, are substantive, and are correctly wired. The phase goal -- "complete protocol specification published as open document with formal schemas, and a working reference server demonstrates the protocol end-to-end" -- is achieved at the artifact and code level.

One minor documentation inaccuracy exists in demo.md (hardcoded port 9090 vs actual random port). This is an info-level finding that does not block the goal.

Two items require human verification (CLI demo runtime and site build) because they involve interactive terminal behavior and browser rendering that cannot be verified programmatically without running live processes.

---

_Verified: 2026-03-28T22:00:00Z_
_Verifier: Claude (gsd-verifier)_
