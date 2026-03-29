---
phase: 07-federation-relay-core
verified: 2026-03-29T09:00:00Z
status: human_needed
score: 10/10 must-haves verified
re_verification: true
  previous_status: gaps_found
  previous_score: 7/10
  gaps_closed:
    - "REPL uses server-assigned federation thread_id ({domain}:{uuidv7}) — repl.rs now parses ainm.thread.create response and extracts result.thread_id"
    - "Server relay routing consults remote_cache.keys() instead of vec![] stub — server.rs pre-populates thread registry at creation and enqueues to known remotes on message send"
    - "Agent mode routing chain unblocked — chained failure from REPL thread_id gap is resolved"
  gaps_remaining: []
  regressions: []
human_verification:
  - test: "docker compose up end-to-end federation demo"
    expected: "Server A human REPL types a message; Server B agent logs show relay receipt, three-layer signature verification, echo response sent back; Server A REPL displays echo response from helper@server-b.local"
    why_human: "Requires Docker environment and interactive terminal input; cannot test programmatically without a running server pair"
  - test: "Legacy mode backward compatibility"
    expected: "Running ainm-demo with no flags launches existing single-server demo unchanged; all commands (hello, form, tool, button, /card, /memory, /evict, /bridge) function correctly"
    why_human: "Requires interactive REPL session to confirm full flow still works"
  - test: "mTLS certificate rejection"
    expected: "TLS handshake fails when connecting to federation port (8443) with a certificate NOT signed by the shared CA; server logs show TLS handshake failed"
    why_human: "Requires constructing a rogue certificate and initiating a TLS connection"
---

# Phase 7: Federation Relay Core Verification Report

**Phase Goal:** Any AINM server can discover a remote server, authenticate via mTLS, and relay messages with origin-server authority over thread state
**Verified:** 2026-03-29T09:00:00Z
**Status:** human_needed — all automated checks pass, 3 items require human testing
**Re-verification:** Yes — after gap closure (Plan 07-05)

---

## Goal Achievement

### Observable Truths

| #  | Truth | Status | Evidence |
|----|-------|--------|----------|
| 1  | Running `ainm-demo` with no flags launches legacy single-server demo | ✓ VERIFIED | `run_legacy_demo()` present; binary compiles; 8 federation flags exist only in `--mode` path |
| 2  | Running `ainm-demo --mode human/agent --identity ... --cert ... --key ... --ca-cert ... --federation-port 8443` launches federation mode | ✓ VERIFIED | `run_federation()` implemented; `--help` shows all 8 flags: `--mode`, `--port`, `--federation-port`, `--identity`, `--cert`, `--key`, `--ca-cert`, `--remote` |
| 3  | All federation crates compile without errors | ✓ VERIFIED | `cargo build` exits 0; 1 dead_code warning only (server_identity field in FederationState — cosmetic) |
| 4  | TLS module builds mTLS ServerConfig and reqwest Client from PEM files | ✓ VERIFIED | `tls.rs` present; `build_mtls_server_config`, `build_mtls_client`, `compute_cert_fingerprint`, `load_cert_der` all present and wired |
| 5  | Certificate fingerprint computed as sha256:hex; server identity served at GET /.well-known/ainm/server | ✓ VERIFIED | `compute_cert_fingerprint` uses `ring::digest::SHA256`; `server_identity_handler` mounted in TLS router |
| 6  | Relay handler validates three layers: mTLS cert fingerprint, server signature, per-message Ed25519 signature | ✓ VERIFIED | Layer 1: cert fingerprint in TLS accept loop (main.rs 338); Layer 2: `verify_relay_signature` in federation.rs; Layer 3: `verify_signature` per message in federation.rs |
| 7  | Relay sender constructs ainm.relay JSON-RPC 2.0 payload with server_signature | ✓ VERIFIED | relay.rs `flush_destination` builds payload with `sign_relay_params` before POST |
| 8  | REPL uses server-assigned federation thread_id for all ainm.message.send calls | ✓ VERIFIED | repl.rs lines 62-82: parses `resp.get("result").and_then(|r| r.get("thread_id"))` from ainm.thread.create response; `thread_id` variable is shadowed with server-assigned value; `thread_id_clone` (line 96) carries the federation-formatted ID into all subsequent message.send calls |
| 9  | Server pre-populates thread registry and relays to known remotes at message send | ✓ VERIFIED | server.rs line 673-679: `for remote_id in fed.remote_cache.keys()` pre-populates thread registry at thread creation; lines 750-766: `fed.remote_cache.keys()` consulted in ainm.message.send to enqueue relay to unregistered remotes; `vec![]` stub is gone |
| 10 | docker compose up orchestrates two servers with distinct certificates and volumes | ✓ VERIFIED | docker-compose.yml: server-a (human mode, 8080/8443), server-b (agent mode, 9080/9443), bridge network with `.local` aliases, `./certs:/certs:ro` mounts on both; server-a.pem and server-b.pem verified OK against ca.pem |

**Score:** 10/10 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `demo/Cargo.toml` | Federation dependencies | ✓ VERIFIED | reqwest 0.13 (rustls feature), rustls 0.23, tokio-rustls 0.26, rustls-pki-types 1.14, clap 4.6, hyper 1, hyper-util 0.1, tower 0.5 |
| `demo/src/main.rs` | CLI arg parsing + mode branching + run_federation | ✓ VERIFIED | Cli struct with 8 flags; `run_legacy_demo()`; `run_federation()` with full two-listener startup; `remote_cache: remote_cache.clone()` in FederationContext literal |
| `demo/src/tls.rs` | mTLS builders, fingerprint computation | ✓ VERIFIED | 114 lines; all 4 public functions present and wired |
| `demo/src/identity.rs` | ServerIdentity, RemoteServerCache, server_identity_handler, keys() | ✓ VERIFIED | All types present; `find_by_fingerprint()` for FAUT-02 present; `pub fn keys(&self) -> Vec<String>` added at line 178 |
| `demo/src/federation.rs` | Relay handler, ThreadRegistry, FederationState | ✓ VERIFIED | 327 lines; all required types and functions present |
| `demo/src/relay.rs` | RelaySender, send queues, exponential backoff | ✓ VERIFIED | 236 lines; BackoffState, DestinationQueue, RelaySender with all public methods |
| `demo/src/signing.rs` | sign_relay_params, verify_relay_signature | ✓ VERIFIED | Both functions present; use server_signature field with JCS canonicalization |
| `demo/src/server.rs` | FederationContext with remote_cache, federation-aware routing | ✓ VERIFIED | `pub remote_cache: crate::identity::RemoteServerCache` in FederationContext; thread registry pre-populated at creation; relay routing consults remote_cache.keys(); vec![] stub eliminated |
| `demo/src/repl.rs` | REPL uses server-returned federation thread_id | ✓ VERIFIED | Lines 62-82: parses ainm.thread.create response, extracts `result.thread_id`, shadows `thread_id` variable; thread_id_clone carries federation-formatted ID for all message.send |
| `demo/certs/ca.pem` | Self-signed CA certificate | ✓ VERIFIED | Present; valid PEM |
| `demo/certs/server-a.pem` | Server A cert signed by CA | ✓ VERIFIED | `openssl verify -CAfile ca.pem server-a.pem` → OK |
| `demo/certs/server-b.pem` | Server B cert signed by CA | ✓ VERIFIED | `openssl verify -CAfile ca.pem server-b.pem` → OK |
| `demo/certs/generate.sh` | Certificate generation script | ✓ VERIFIED | Present and executable |
| `demo/Dockerfile` | Multi-stage Rust build | ✓ VERIFIED | FROM rust:1.85-bookworm builder; ENTRYPOINT ainm-demo |
| `demo/docker-compose.yml` | Two-server orchestration with certs | ✓ VERIFIED | server-a (human mode 8080/8443), server-b (agent mode 9080/9443), federation bridge network, cert volumes |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `demo/src/main.rs` | `demo/Cargo.toml` | `use clap::Parser` | ✓ WIRED | Present |
| `demo/src/tls.rs` | `rustls-pki-types` | `CertificateDer::pem_file_iter` | ✓ WIRED | Present at tls.rs line 23 |
| `demo/src/tls.rs` | `ring::digest` | SHA-256 fingerprint | ✓ WIRED | `ring::digest::digest(&ring::digest::SHA256, ...)` |
| `demo/src/federation.rs` | `demo/src/signing.rs` | `signing::verify_relay_signature` | ✓ WIRED | Present in relay_handler |
| `demo/src/federation.rs` | `demo/src/identity.rs` | `RemoteServerCache` | ✓ WIRED | Used as `state.remote_cache` in relay_handler |
| `demo/src/relay.rs` | `demo/src/signing.rs` | `signing::sign_relay_params` | ✓ WIRED | Present at relay.rs line 146 |
| `demo/src/relay.rs` | `demo/src/tls.rs` | `tls::build_mtls_client` | ✓ WIRED | http_client built via tls::build_mtls_client in main.rs; passed into RelaySender::new |
| `demo/src/main.rs` | `demo/src/tls.rs` | `tls::build_mtls_server_config` | ✓ WIRED | Present at main.rs line 293 |
| `demo/src/main.rs` | `demo/src/federation.rs` | `federation::relay_handler` | ✓ WIRED | Mounted at POST /ainm/v1/relay |
| `demo/src/main.rs` | `demo/src/relay.rs` | `relay::RelaySender::new` | ✓ WIRED | Present at main.rs line 118 |
| `demo/docker-compose.yml` | `demo/certs/` | Volume mount `./certs:/certs` | ✓ WIRED | Both services mount `./certs:/certs:ro` |
| `demo/src/server.rs` | `demo/src/federation.rs` | `parse_origin_domain` | ✓ WIRED | Used at server.rs lines 313 and 719 |
| `demo/src/repl.rs` | `demo/src/server.rs` (thread_id response) | Thread_id from ainm.thread.create response | ✓ WIRED | repl.rs lines 62-82 parse `resp.get("result").and_then(|r| r.get("thread_id"))`; federation-formatted ID flows into thread_id_clone and all message.send calls |
| `demo/src/server.rs` | `demo/src/identity.rs` | `fed.remote_cache.keys()` | ✓ WIRED | Called at server.rs line 676 (thread create) and line 757-758 (message send relay routing) |

---

## Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|--------------|--------|-------------------|--------|
| `federation.rs` relay_handler | `messages` | `params.get("messages")` from HTTP POST body | Yes — validated JSON array from relay POST | ✓ FLOWING |
| `relay.rs` flush_destination | `messages` / `relay_endpoint` | messages from queue; endpoint from `remote_cache.get(destination)` | Yes — remote_cache populated by resolve_remote_identity at startup | ✓ FLOWING |
| `server.rs` handle_human_connection thread_id routing | `thread_id` for relay routing | Server-assigned federation thread_id extracted from ainm.thread.create response in repl.rs | Yes — `{server-a.local}:{uuidv7}` format; `parse_origin_domain` returns server domain; relay path taken | ✓ FLOWING |
| `server.rs` relay enqueue at message send | `remote_domains` | `fed.remote_cache.keys()` filtered by registered set | Yes — populated from resolve_remote_identity; returns known remote server IDs | ✓ FLOWING |

---

## Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Binary compiles | `cargo build` in demo/ | Finished dev profile; 1 dead_code warning only | ✓ PASS |
| Help shows all 8 federation flags | `./target/debug/ainm-demo --help` | Shows `--mode`, `--port`, `--federation-port`, `--identity`, `--cert`, `--key`, `--ca-cert`, `--remote` | ✓ PASS |
| vec![] relay stub is gone | `grep 'vec!\[\]' src/server.rs` in relay section | No matches | ✓ PASS |
| REPL parses result.thread_id | `grep 'get("result")' src/repl.rs` | Line 66 match | ✓ PASS |
| identity.rs keys() method present | `grep 'pub fn keys' src/identity.rs` | Line 178 match | ✓ PASS |
| remote_cache.keys() used in server.rs | `grep 'remote_cache' src/server.rs` | Lines 32, 676, 757 | ✓ PASS |
| Server-a cert signed by CA | `openssl verify -CAfile ca.pem server-a.pem` | OK | ✓ PASS |
| Server-b cert signed by CA | `openssl verify -CAfile ca.pem server-b.pem` | OK | ✓ PASS |
| End-to-end federation message exchange | `docker compose up` | Not testable without Docker | ? SKIP (human required) |

---

## Requirements Coverage

| Requirement | Source Plan(s) | Description | Status | Evidence |
|-------------|---------------|-------------|--------|---------|
| RELY-01 | 07-01, 07-02, 07-04 | Server can discover remote servers via .well-known endpoint | ✓ SATISFIED | `GET /.well-known/ainm/server` mounted in TLS router; `resolve_remote_identity()` fetches and parses it at startup |
| RELY-02 | 07-03, 07-04, 07-05 | Server can relay messages via HTTP POST with JSON-RPC 2.0 framing | ✓ SATISFIED | relay.rs builds and POSTs correct JSON-RPC payload; server.rs message.send handler enqueues via `fed.relay_sender.enqueue`; vec![] stub eliminated; thread_id uses federation format enabling correct origin routing |
| RELY-03 | 07-03, 07-04 | Origin server is authoritative for thread state | ✓ SATISFIED | ThreadRegistry.create_thread() embeds origin domain; next_sequence() only increments on origin; relay_handler assigns sequence when `origin == state.server_id` |
| RELY-04 | 07-03, 07-04 | Push-based federation, no polling | ✓ SATISFIED | RelaySender.enqueue() called immediately at message send; start_send_loop() flushes queues every 500ms |
| RELY-05 | 07-03 | Per-destination send queues with exponential backoff | ✓ SATISFIED | BackoffState with 1s base, 60s max, 2^n growth; per-destination DestinationQueue in HashMap |
| FAUT-01 | 07-01, 07-02, 07-04 | Servers authenticate via mutual TLS | ✓ SATISFIED | `build_mtls_server_config` uses `WebPkiClientVerifier`; TLS acceptor requires client cert; `build_mtls_client` presents client cert |
| FAUT-02 | 07-02, 07-03, 07-04 | Server identity verified by matching TLS cert fingerprint against .well-known | ✓ SATISFIED | `find_by_fingerprint()` on RemoteServerCache in TLS accept loop (main.rs 338); fingerprint stored in ServerIdentity and published at .well-known |
| FAUT-03 | 07-03 | Triple-layer validation: mTLS + server signature + message signature | ✓ SATISFIED | Layer 1: cert fingerprint in accept loop; Layer 2: `verify_relay_signature` in relay_handler; Layer 3: `verify_signature` per message in relay_handler |
| DEMO-01 | 07-04 | docker-compose spins up 2+ AINM servers with distinct identities and certificates | ✓ SATISFIED | docker-compose.yml: server-a (human/8080/8443), server-b (agent/9080/9443), separate certs, federation bridge network; both server certs verified against CA |
| DEMO-02 | 07-04, 07-05 | Demo shows cross-server message exchange between human on A and agent on B | ✓ SATISFIED (automated) / ? HUMAN NEEDED (runtime) | All routing code is correct: REPL uses federation thread_id, server.rs relays to remote_cache.keys(), relay.rs POSTs with mTLS. End-to-end runtime verification requires Docker environment |

**Requirements Not in Phase 7 (confirmed correctly deferred per REQUIREMENTS.md traceability):**
- FAUT-04, FAUT-05, FAUT-06: Phase 8
- FREL-01 through FREL-04: Phase 8
- BKFL-01 through BKFL-04: Phase 8
- AINF-01 through AINF-04: Phase 9
- DEMO-03 through DEMO-05: Phase 8+9

No orphaned requirements — all 10 Phase 7 requirement IDs (RELY-01 through DEMO-02) are claimed in plan frontmatter and verified above.

---

## Anti-Patterns Found

| File | Line(s) | Pattern | Severity | Impact |
|------|---------|---------|----------|--------|
| `demo/src/federation.rs` | 120-136 | `server_identity` field in FederationState is never read after construction — dead code warning in compiler | ℹ️ Info | No functional impact; cosmetic dead code warning. Struct field not used beyond initial construction |

No blockers or warnings remain. The two previous blocker anti-patterns (vec![] stub and discarded thread_id) are resolved.

---

## Human Verification Required

### 1. End-to-End Cross-Server Federation Demo

**Test:** Run `docker compose up` in demo/, attach to server-a container's stdin (or use `docker compose exec server-a ainm-demo --mode human ...`), type a message at the REPL prompt.
**Expected:** Server B logs show `[federation] Server signature verified`, `[federation] Message signature verified`, `[agent] Received from alice@server-a.local: "..."`, then server-a REPL displays `Echo from helper@server-b.local: You said "..."`.
**Why human:** Requires Docker environment, interactive terminal, and visual inspection of two terminal outputs simultaneously.

### 2. Legacy Mode Backward Compatibility

**Test:** Run `ainm-demo` with no flags; interact with the REPL using all commands: hello, form, approve, tool, button, /card, /memory, /evict, /bridge.
**Expected:** Legacy single-server demo works identically to pre-Phase-7 behavior; all commands function including streaming responses.
**Why human:** Requires interactive session and visual verification of streaming output.

### 3. mTLS Certificate Rejection

**Test:** Attempt to connect to the federation port (8443) with a certificate NOT signed by the shared CA.
**Expected:** TLS handshake fails with certificate verification error; server logs show "TLS handshake failed".
**Why human:** Requires constructing a rogue certificate and initiating a TLS connection.

---

## Re-Verification Summary

**Previous status:** gaps_found (7/10 truths verified)

**Gaps that were closed by Plan 07-05:**

**Gap 1 (REPL thread_id) — CLOSED:** `demo/src/repl.rs` lines 62-82 now parse the `ainm.thread.create` response, extract `result.thread_id`, and shadow the local `thread_id` variable with the server-assigned federation-formatted ID (`{server-a.local}:{uuidv7}`). The `thread_id_clone` at line 96 correctly carries this value into `run_repl_loop` for all subsequent `ainm.message.send` calls.

**Gap 2 (Initial relay stub) — CLOSED:** `demo/src/server.rs` no longer contains the `remote_domains = vec![]` stub. Two fixes were applied: (1) at thread creation (lines 673-679), `fed.remote_cache.keys()` pre-populates the thread registry with all known remotes immediately; (2) at message send (lines 750-766), `fed.remote_cache.keys()` is consulted to find and enqueue relay to any remotes not yet registered in the thread.

**Gap 3 (Agent routing chain) — CLOSED by consequence:** The chained failure from Gap 1 (empty origin from plain UUID thread_id) is resolved. With the federation-formatted thread_id, `parse_origin_domain()` now returns the correct origin domain, enabling the agent's relay response to find the correct remote in `remote_cache`.

**No regressions detected.** All 7 previously-verified truths still hold. Build: `cargo build` exits 0 with 1 dead_code warning (unchanged from before).

---

_Verified: 2026-03-29T09:00:00Z_
_Verifier: Claude (gsd-verifier)_
