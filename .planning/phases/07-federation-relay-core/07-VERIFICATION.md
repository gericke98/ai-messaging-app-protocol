---
phase: 07-federation-relay-core
verified: 2026-03-29T07:00:00Z
status: gaps_found
score: 7/10 must-haves verified
re_verification: false
gaps:
  - truth: "Human mode connects REPL and routes messages through thread registry to local or relay"
    status: failed
    reason: "REPL generates a plain UUID thread_id (via protocol::new_message_id()) and ignores the server's response which contains the federation-formatted {server-a.local}:{uuidv7} thread_id. All human messages are sent with the wrong thread_id, causing parse_origin_domain() to return empty string, which routes to the local-delivery path instead of relay."
    artifacts:
      - path: "demo/src/repl.rs"
        issue: "Line 50: thread_id = protocol::new_message_id() generates plain UUID. Line 63: thread create response is received but never parsed -- thread_id_clone retains original plain UUID forever."
      - path: "demo/src/server.rs"
        issue: "Lines 685-693: server returns correct federation thread_id in response, but REPL ignores it."
    missing:
      - "repl.rs must parse the thread_id from the ainm.thread.create response and use that federation-formatted ID for all subsequent ainm.message.send calls"
  - truth: "docker compose up starts two servers that exchange messages across federation"
    status: failed
    reason: "Initial message relay from Server A to Server B never fires: remote_domains is hardcoded as vec![] (server.rs line 751) and the thread registry participating_servers set is empty at first message time. No relay enqueue occurs on the first (or any) human-sent message unless Server B had already sent a relay to Server A first."
    artifacts:
      - path: "demo/src/server.rs"
        issue: "Lines 748-755: remote_domains = vec![] with comment 'Simple approach: relay to any known remote'. The relay_sender has the remote_cache but the code does not consult it. Thread registry servers set is empty because add_server() is only called from relay_handler (inbound), never at thread-creation time for known remotes."
    missing:
      - "server.rs handle_human_connection ainm.message.send branch must relay to known remote servers even when they are not yet in the thread registry -- consult remote_cache or pre-populate the thread registry with the known remote at thread creation time"
      - "Alternatively: server.rs ainm.thread.create handler should call thread_registry.add_server() for each known remote from remote_cache after creating the thread"
  - truth: "Agent mode runs headless agent, receives relayed messages, sends responses back via relay"
    status: partial
    reason: "Agent receives and processes relayed messages correctly, generates signed responses, and enqueues them via relay_sender. However, the response never arrives at Server A because: (1) the thread_id on relayed messages from Server A will be a plain UUID (from the REPL gap above), so parse_origin_domain returns empty string, and (2) the relay_sender.enqueue(origin, ...) call at main.rs line 521 uses the parsed origin which will be empty. Chained failure from REPL thread_id gap."
    artifacts:
      - path: "demo/src/main.rs"
        issue: "Lines 517-522: origin = parse_origin_domain(thread_id) returns empty string when thread_id is a plain UUID. The enqueue call fires but with empty destination string, which will fail to find a remote in remote_cache."
    missing:
      - "Fix the REPL thread_id propagation gap (see above). Once thread_id uses federation format, this agent routing path should work correctly."
human_verification:
  - test: "docker compose up end-to-end"
    expected: "Server A human REPL types a message; Server B agent logs show relay receipt, signature verification, echo response sent back; Server A REPL displays echo response from helper@server-b.local"
    why_human: "Requires Docker environment and interactive terminal input; cannot test programmatically without a running server pair"
  - test: "Legacy mode backward compatibility"
    expected: "Running ainm-demo with no flags launches the existing single-server demo unchanged"
    why_human: "Requires interactive REPL session to confirm full flow still works"
---

# Phase 7: Federation Relay Core Verification Report

**Phase Goal:** Any AINM server can discover a remote server, authenticate via mTLS, and relay messages with origin-server authority over thread state
**Verified:** 2026-03-29T07:00:00Z
**Status:** gaps_found — 2 blocking gaps prevent end-to-end federation message exchange
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| #  | Truth | Status | Evidence |
|----|-------|--------|----------|
| 1  | Running `ainm-demo` with no flags launches legacy single-server demo | ✓ VERIFIED | `run_legacy_demo()` present; binary tested with no flags |
| 2  | Running `ainm-demo --mode human/agent --identity ... --cert ... --key ... --ca-cert ... --federation-port 8443` launches federation mode | ✓ VERIFIED | `run_federation()` implemented; `--help` shows all 8 flags |
| 3  | All federation crates compile without errors | ✓ VERIFIED | `cargo build` exits 0 with 1 dead_code warning only |
| 4  | TLS module builds mTLS ServerConfig and reqwest Client from PEM files | ✓ VERIFIED | `tls.rs` 114 lines; all 4 public functions present and wired |
| 5  | Certificate fingerprint computed as sha256:hex; server identity served at GET /.well-known/ainm/server | ✓ VERIFIED | `compute_cert_fingerprint` uses ring SHA-256; `server_identity_handler` mounted in TLS router |
| 6  | Relay handler validates three layers: mTLS cert fingerprint, server signature, per-message Ed25519 signature | ✓ VERIFIED | Layer 1 in TLS accept loop (main.rs 329-367); Layer 2 `verify_relay_signature` in federation.rs 212; Layer 3 `verify_signature` in federation.rs 238 |
| 7  | Relay sender constructs ainm.relay JSON-RPC 2.0 payload with server_signature | ✓ VERIFIED | relay.rs flush_destination builds payload with `sign_relay_params` before POST |
| 8  | Human mode connects REPL and routes messages through thread registry to local or relay | ✗ FAILED | REPL ignores server's federation thread_id in response; all messages sent with plain UUID thread_id; `parse_origin_domain` returns empty; relay never enqueued |
| 9  | docker compose up starts two servers that exchange messages across federation | ✗ FAILED | Docker Compose file is valid YAML with correct structure, but initial relay from Server A never fires: `remote_domains = vec![]` in server.rs is an unfinished stub |
| 10 | Agent mode receives relayed messages and sends responses back via relay | ✗ FAILED | Chained failure: relay from Server A never reaches Server B due to gaps in #8 and #9; agent response logic is correct in isolation but unreachable |

**Score:** 7/10 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `demo/Cargo.toml` | Federation dependencies | ✓ VERIFIED | reqwest 0.13 (rustls feature), rustls 0.23, tokio-rustls 0.26, rustls-pki-types 1.14, clap 4.6, hyper 1, hyper-util 0.1, tower 0.5 |
| `demo/src/main.rs` | CLI arg parsing + mode branching + run_federation | ✓ VERIFIED | Cli struct with 8 flags; run_legacy_demo(); run_federation() with full two-listener startup |
| `demo/src/tls.rs` | mTLS builders, fingerprint computation | ✓ VERIFIED | 114 lines; build_mtls_server_config, build_mtls_client, compute_cert_fingerprint, load_cert_der |
| `demo/src/identity.rs` | ServerIdentity, RemoteServerCache, server_identity_handler | ✓ VERIFIED | All types present; find_by_fingerprint() for FAUT-02 layer 1 present |
| `demo/src/federation.rs` | Relay handler, ThreadRegistry, FederationState | ✓ VERIFIED | 327 lines; all required types and functions present |
| `demo/src/relay.rs` | RelaySender, send queues, exponential backoff | ✓ VERIFIED | 236 lines; BackoffState, DestinationQueue, RelaySender with all public methods |
| `demo/src/signing.rs` | Extended with sign_relay_params, verify_relay_signature | ✓ VERIFIED | Both functions present; use server_signature field with JCS canonicalization |
| `demo/src/server.rs` | FederationContext, federation-aware routing | ✓ VERIFIED (partial) | FederationContext present; routing logic present but initial relay stub (vec![]) is an unfinished placeholder |
| `demo/certs/ca.pem` | Self-signed CA certificate | ✓ VERIFIED | Present; valid PEM |
| `demo/certs/server-a.pem` | Server A cert signed by CA with correct SANs | ✓ VERIFIED | CN=server-a.local; SANs: server-a.local, server-a, localhost; issuer: AINM Demo CA |
| `demo/certs/server-b.pem` | Server B cert signed by CA with correct SANs | ✓ VERIFIED | CN=server-b.local; issuer: AINM Demo CA |
| `demo/certs/generate.sh` | Certificate generation script | ✓ VERIFIED | Present and executable |
| `demo/Dockerfile` | Multi-stage Rust build | ✓ VERIFIED | FROM rust:1.85-bookworm builder; ENTRYPOINT ainm-demo |
| `demo/docker-compose.yml` | Two-server orchestration with certs | ✓ VERIFIED | server-a (human mode 8080/8443), server-b (agent mode 9080/9443), federation bridge network with .local aliases, cert volumes |
| `demo/src/repl.rs` | REPL uses server-returned federation thread_id | ✗ STUB | Line 50 generates a plain UUID; line 63 discards the server's thread_id response; thread_id_clone stays as plain UUID for all messages |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `demo/src/main.rs` | `demo/Cargo.toml` | `use clap::Parser` | ✓ WIRED | Present at line 14 |
| `demo/src/tls.rs` | `rustls-pki-types` | `CertificateDer::pem_file_iter` | ✓ WIRED | Present at tls.rs line 23 |
| `demo/src/tls.rs` | `ring::digest` | SHA-256 fingerprint | ✓ WIRED | `ring::digest::digest(&ring::digest::SHA256, ...)` at line 95 |
| `demo/src/identity.rs` | `demo/src/tls.rs` | `crate::tls::compute_cert_fingerprint` | ✗ NOT_WIRED | identity.rs does not import tls; fingerprint is passed as a string parameter to `build_server_identity` — this is acceptable design but plan's key_link assumption was wrong |
| `demo/src/federation.rs` | `demo/src/signing.rs` | `signing::verify_relay_signature` | ✓ WIRED | Present at federation.rs line 212 |
| `demo/src/federation.rs` | `demo/src/identity.rs` | `RemoteServerCache` | ✓ WIRED | Used as `state.remote_cache` in relay_handler |
| `demo/src/relay.rs` | `demo/src/signing.rs` | `signing::sign_relay_params` | ✓ WIRED | Present at relay.rs line 146 |
| `demo/src/relay.rs` | `demo/src/tls.rs` | `tls::build_mtls_client` | ✓ WIRED | http_client passed into RelaySender::new; built via tls::build_mtls_client in main.rs line 116 |
| `demo/src/main.rs` | `demo/src/tls.rs` | `tls::build_mtls_server_config` | ✓ WIRED | Present at main.rs line 293 |
| `demo/src/main.rs` | `demo/src/federation.rs` | `federation::relay_handler` | ✓ WIRED | Mounted at POST /ainm/v1/relay (main.rs line 302) |
| `demo/src/main.rs` | `demo/src/relay.rs` | `relay::RelaySender::new` | ✓ WIRED | Present at main.rs line 118 |
| `demo/docker-compose.yml` | `demo/certs/` | Volume mount `./certs:/certs` | ✓ WIRED | Both services mount `./certs:/certs:ro` |
| `demo/src/server.rs` | `demo/src/federation.rs` | `parse_origin_domain` | ✓ WIRED | Used at server.rs lines 313 and 719 |
| `demo/src/repl.rs` | `demo/src/server.rs` (thread_id response) | Thread_id from ainm.thread.create response | ✗ NOT_WIRED | REPL does not read thread_id from server response — stays with self-generated plain UUID |

---

## Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|--------------|--------|-------------------|--------|
| `federation.rs` relay_handler | `messages` | `params.get("messages")` from HTTP POST body | Yes — validated JSON array from relay POST | ✓ FLOWING |
| `relay.rs` flush_destination | `messages` / `relay_endpoint` | messages from queue; endpoint from `remote_cache.get(destination)` | Yes — remote_cache populated by resolve_remote_identity at startup | ✓ FLOWING |
| `server.rs` handle_human_connection | `thread_id` for relay routing | REPL-provided thread_id | No — plain UUID with no origin prefix; `parse_origin_domain` returns empty string | ✗ HOLLOW — routing logic correct but input thread_id breaks origin detection |
| `server.rs` initial relay enqueue | `remote_domains` | `vec![]` hardcoded stub | No — always empty | ✗ DISCONNECTED — relay to remote on first message never fires |

---

## Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Binary compiles | `cargo build` in demo/ | Finished dev profile with 1 dead_code warning | ✓ PASS |
| Help shows all 8 federation flags | `./target/debug/ainm-demo --help` | Shows --mode, --port, --federation-port, --identity, --cert, --key, --ca-cert, --remote | ✓ PASS |
| Unknown mode exits 1 | `./target/debug/ainm-demo --mode invalid` | "Unknown mode: 'invalid'. Valid modes: human, agent" (exit 1) | ✓ PASS |
| Server-a cert signed by CA | `openssl verify -CAfile ca.pem server-a.pem` | OK | ✓ PASS |
| Server-b cert signed by CA | `openssl verify -CAfile ca.pem server-b.pem` | OK | ✓ PASS |
| Docker Compose config valid | `docker compose config` | docker not available in environment | ? SKIP |
| End-to-end federation message exchange | `docker compose up` | Not testable without Docker | ? SKIP (human required) |

---

## Requirements Coverage

| Requirement | Source Plan(s) | Description | Status | Evidence |
|-------------|---------------|-------------|--------|---------|
| RELY-01 | 07-01, 07-02, 07-04 | Server can discover remote servers via .well-known endpoint | ✓ SATISFIED | `GET /.well-known/ainm/server` mounted in TLS router; `resolve_remote_identity()` fetches and parses it at startup |
| RELY-02 | 07-03, 07-04 | Server can relay messages via HTTP POST with JSON-RPC 2.0 framing | ✗ BLOCKED | relay.rs builds and POSTs correct JSON-RPC payload, but the path to enqueue from server.rs is broken (remote_domains stub) — relay POST logic exists but is unreachable for initial messages |
| RELY-03 | 07-03, 07-04 | Origin server is authoritative for thread state | ✓ SATISFIED | ThreadRegistry.create_thread() embeds origin domain; next_sequence() only increments on origin; relay_handler assigns sequence when `origin == state.server_id` |
| RELY-04 | 07-03, 07-04 | Push-based federation, no polling | ✓ SATISFIED | RelaySender.enqueue() is called immediately; start_send_loop() flushes queues every 500ms |
| RELY-05 | 07-03 | Per-destination send queues with exponential backoff | ✓ SATISFIED | BackoffState with 1s base, 60s max, 2^n growth; per-destination DestinationQueue in HashMap |
| FAUT-01 | 07-01, 07-02, 07-04 | Servers authenticate via mutual TLS | ✓ SATISFIED | `build_mtls_server_config` uses `WebPkiClientVerifier`; TLS acceptor requires client cert; `build_mtls_client` presents client cert |
| FAUT-02 | 07-02, 07-03, 07-04 | Server identity verified by matching TLS cert fingerprint against .well-known | ✓ SATISFIED | `find_by_fingerprint()` on RemoteServerCache in TLS accept loop (main.rs 338); fingerprint stored in ServerIdentity and published at .well-known |
| FAUT-03 | 07-03 | Triple-layer validation: mTLS + server signature + message signature | ✓ SATISFIED | Layer 1: cert fingerprint in accept loop; Layer 2: `verify_relay_signature` in relay_handler; Layer 3: `verify_signature` per message in relay_handler |
| DEMO-01 | 07-04 | docker-compose spins up 2+ AINM servers with distinct identities and certificates | ✓ SATISFIED | docker-compose.yml is valid YAML with server-a (human/8080/8443) and server-b (agent/9080/9443), separate certs, federation bridge network |
| DEMO-02 | 07-04 | Demo shows cross-server message exchange between human on A and agent on B | ✗ BLOCKED | Infrastructure exists but two routing bugs prevent actual message exchange: (1) REPL uses wrong thread_id format, (2) initial relay enqueue is a stub |

**Requirements Not in Phase 7 (per REQUIREMENTS.md traceability — confirmed correctly deferred):**
- FAUT-04, FAUT-05, FAUT-06: Phase 8
- FREL-01 through FREL-04: Phase 8
- BKFL-01 through BKFL-04: Phase 8
- AINF-01 through AINF-04: Phase 9
- DEMO-03 through DEMO-05: Phase 8+9

No orphaned requirements found — all 10 Phase 7 requirement IDs (RELY-01 through DEMO-02) are claimed in plan frontmatter and verified above.

---

## Anti-Patterns Found

| File | Line(s) | Pattern | Severity | Impact |
|------|---------|---------|----------|--------|
| `demo/src/server.rs` | 748-755 | `remote_domains = vec![]` with comment "Simple approach: relay to any known remote" — dead code below it | 🛑 Blocker | Initial relay from Server A to Server B never fires; DEMO-02 and RELY-02 blocked |
| `demo/src/repl.rs` | 50, 63 | REPL generates own thread_id UUID, receives server response but never parses `thread_id` field from it | 🛑 Blocker | Federation routing in server.rs depends on thread_id prefix `{origin-domain}:{uuidv7}` which REPL never uses |
| `demo/src/federation.rs` | 120-136 | `server_identity` field in FederationState is never read after construction — dead code warning in compiler | ℹ️ Info | No functional impact; struct field not used beyond initial construction |

---

## Human Verification Required

### 1. End-to-End Cross-Server Federation Demo

**Test:** After fixing the two blocking gaps: run `docker compose up` in demo/, attach to server-a container's stdin, type a message at the REPL prompt.
**Expected:** Server B logs show `[federation] Server signature verified`, `[federation] Message signature verified`, `[agent] Received from alice@server-a.local: "..."`, then server-a REPL displays `Echo from helper@server-b.local: You said "..."`.
**Why human:** Requires Docker environment, interactive terminal, and visual inspection of two terminal outputs simultaneously.

### 2. Legacy Mode Backward Compatibility

**Test:** Run `ainm-demo` with no flags; interact with the REPL using existing commands (hello, form, tool, etc.).
**Expected:** Legacy single-server demo works identically to pre-Phase-7 behavior; all commands function.
**Why human:** Requires interactive session and visual verification of streaming output.

### 3. mTLS Certificate Rejection

**Test:** Attempt to connect to the federation port (8443) with a certificate NOT signed by the shared CA.
**Expected:** TLS handshake fails with certificate verification error; server logs show "TLS handshake failed".
**Why human:** Requires constructing a rogue certificate and initiating a TLS connection.

---

## Gaps Summary

Two blocking gaps prevent Phase 7 goal achievement. The infrastructure — mTLS, signing, relay module, thread registry, Docker Compose, certificate chain — is all correctly implemented and compiles cleanly. The failure is in the final routing wiring:

**Gap 1 (REPL thread_id):** `demo/src/repl.rs` line 50 generates a random UUID for thread_id and lines 62-65 receive the `ainm.thread.create` response but discard the body without reading the server-assigned federation thread_id (`{server-a.local}:{uuidv7}`). Every subsequent `ainm.message.send` uses the plain UUID, so `parse_origin_domain()` in server.rs returns empty string, matching the `origin.is_empty()` branch which routes to local delivery and the broken `remote_domains = vec![]` stub — never to the relay enqueue.

**Gap 2 (Initial relay stub):** `demo/src/server.rs` lines 744-755 have a comment "Simple approach: relay to any known remote" followed by `remote_domains = vec![]`. This is an acknowledged unfinished stub. Even with a correct federation thread_id, the first message from the human only gets relayed if the remote server is already in the thread registry — which never happens at thread-creation time. The fix is either to call `thread_registry.add_server()` for all known remotes when creating a thread, or to consult `remote_cache` keys in the relay enqueue path.

These two gaps are tightly coupled: fixing Gap 1 (REPL uses federation thread_id) reveals Gap 2 (no initial relay enqueue); fixing both enables DEMO-02 and RELY-02.

Root cause grouping: Both failures are in the "human-sends-message → relay-to-remote" path. All other federation directions (Server B receives relay, Server B agent processes, Server B agent sends response back) appear correctly implemented.

---

_Verified: 2026-03-29T07:00:00Z_
_Verifier: Claude (gsd-verifier)_
