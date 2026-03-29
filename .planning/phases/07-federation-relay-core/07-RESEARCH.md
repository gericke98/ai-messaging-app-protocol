# Phase 7: Federation Relay Core - Research

**Researched:** 2026-03-28
**Domain:** Server-to-server federation relay via HTTP POST with mTLS, origin-server thread authority, and Docker-based demo
**Confidence:** HIGH

## Summary

Phase 7 adds federation relay to the existing single-server AINM demo. Two servers discover each other via `.well-known/ainm/server` endpoints, authenticate with mutual TLS, and exchange messages via `POST /ainm/v1/relay` with JSON-RPC 2.0 framing. The origin server (where a thread was created) is sole authority for thread state. Messages are pushed immediately with per-destination send queues and exponential backoff.

The existing demo codebase (`demo/src/`) provides a solid foundation: Axum router, Ed25519 signing via `ring`, JSON-RPC 2.0 framing, identity store, and agent response logic are all reusable. The primary engineering work is: (1) adding CLI flags to configure server mode, identity, and certificates; (2) adding a TLS-enabled federation listener alongside the existing plain TCP listener; (3) implementing the relay sender (outbound) and relay handler (inbound) with three-layer signature validation; (4) building a thread registry with origin-server tracking and participant routing; and (5) creating a Docker Compose demo with pre-generated certificates.

The transport spec (Sections 5.1-5.4) fully defines the relay protocol. The identity and security specs define the signing/verification procedures. JSON schemas exist for `RelayParams`, `RelayResult`, and `WellKnownAINMConfig`. Prior project research (`.planning/research/`) covers architecture patterns, stack additions, and pitfalls in depth. This phase-level research synthesizes all of that into actionable guidance for the planner.

**Primary recommendation:** Extend the existing `demo/src/` binary with new modules (`relay.rs`, `federation.rs`, `tls.rs`) and CLI flags via clap. Run two listeners per server (plain TCP for local WebSocket, mTLS for federation relay). Pre-generate certificates into `demo/certs/` for deterministic Docker Compose startup.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Two interactive terminals -- human REPL on Server A, structured federation logs on Server B (headless agent). User types on A, watches federation activity on B.
- **D-02:** Participants: alice@server-a.local (human) on Server A, helper@server-b.local (agent) on Server B. Proves core value: human-agent conversation across federation.
- **D-03:** Server B runs headless -- no interactive REPL. Its terminal shows relay receipt, signature validation, agent processing, and outgoing relay messages.
- **D-04:** Extend the existing `demo/src/` binary -- add new modules (`relay.rs`, `federation.rs`, `tls.rs`), extend existing modules (`identity.rs`, `signing.rs`, `server.rs`). No new crate or workspace.
- **D-05:** Server mode configured via CLI flags: `--mode human|agent`, `--port`, `--federation-port`, `--identity`, `--cert`, `--key`. Docker-compose friendly.
- **D-06:** Backward compatible -- running `ainm-demo` with no flags still launches the original single-server demo. Federation mode requires explicit flags.
- **D-07:** Pre-generated self-signed CA + server certificates checked into `demo/certs/` with a README explaining how to regenerate. Deterministic, `docker-compose up` just works.
- **D-08:** Certificate structure: `ca.pem`, `ca-key.pem`, `server-a.pem`, `server-a-key.pem`, `server-b.pem`, `server-b-key.pem`.
- **D-09:** Use `rustls` (pure Rust, no OpenSSL) with `rustls-pemfile` for PEM loading. Matches CLAUDE.md technology stack. `axum-server` provides rustls integration.
- **D-10:** First message in a thread creates the thread on the sender's home server. That server becomes the origin -- sole authority for participant list and message ordering.
- **D-11:** All messages in a thread route through the origin server. Non-origin servers send replies to origin; origin assigns sequence numbers and fans out to all participating servers.
- **D-12:** Thread ID embeds origin domain: `server-a.local:01JQXYZ7K8M3N4P5Q6R7S8T9` (domain prefix + UUIDv7). Any server can parse the origin from the thread ID without a lookup.

### Claude's Discretion
- Send queue implementation details (in-memory for Phase 7, per-destination with exponential backoff)
- Exact CLI flag names and parsing library (clap recommended)
- Internal module boundaries between relay.rs and federation.rs
- Log formatting and verbosity levels for Server B's headless output

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| RELY-01 | Server can discover remote servers via .well-known endpoint resolution of the @provider domain | Transport spec Section 5.2 defines `GET /.well-known/ainm/server` response format. Identity schema defines `WellKnownAINMConfig`. reqwest handles the HTTP fetch with TLS. |
| RELY-02 | Server can relay messages to remote servers via HTTP POST with JSON-RPC 2.0 framing | Transport spec Section 5.3 defines `ainm.relay` method with `RelayParams`/`RelayResult` schemas. Existing `protocol.rs` JSON-RPC constructors reusable. |
| RELY-03 | Origin server is authoritative for thread state (participant list, ordering, metadata) | D-10/D-11/D-12 lock the design. Thread ID format `{domain}:{uuidv7}` enables origin parsing. Thread registry stores origin + participants. |
| RELY-04 | Server uses push-based federation model (no polling) -- messages forwarded immediately | Federation sender with per-destination queues. Relay triggered on message receipt, not on timer. |
| RELY-05 | Server maintains per-destination send queues with exponential backoff for unreachable servers | In-memory `VecDeque` per destination with backoff state. Not persistent (Phase 8 adds redb persistence). |
| FAUT-01 | Servers authenticate via mutual TLS (both sides present certificates) | rustls `ServerConfig` with `WebPkiClientVerifier` for inbound; reqwest `Identity::from_pem()` for outbound. Pre-generated certs in `demo/certs/`. |
| FAUT-02 | Server identity verified by matching TLS certificate fingerprint against .well-known published fingerprint | SHA-256 fingerprint computed from DER-encoded cert via `ring::digest`. Compared against `tls_fingerprint` from server identity response. |
| FAUT-03 | Relay messages include server-level signature in addition to sender's Ed25519 message signature (triple-layer validation) | Existing `signing.rs` `sign_envelope()` reused for server-level signing. Relay handler validates: mTLS cert fingerprint, server signature, per-message signature. |
| DEMO-01 | docker-compose spins up 2+ AINM servers with distinct identities and certificates | Docker Compose v2 with two services, shared cert volume, bridge network. Pre-generated certs enable `docker compose up` without setup. |
| DEMO-02 | Demo shows cross-server message exchange between a human on server A and an agent on server B | alice@server-a.local types in REPL on Server A; message relays to Server B; helper@server-b.local processes and responds; response relays back. |
</phase_requirements>

## Standard Stack

### Core (Existing -- DO NOT CHANGE)

| Library | Version | Purpose | Status |
|---------|---------|---------|--------|
| axum | 0.8.x | HTTP/WebSocket framework | Already in Cargo.toml |
| tokio | 1.x | Async runtime | Already in Cargo.toml |
| ring | 0.17.x | Ed25519 signing, SHA-256 hashing | Already in Cargo.toml |
| serde + serde_json | 1.x | Serialization | Already in Cargo.toml |
| serde_json_canonicalizer | 0.3.x | RFC 8785 JCS | Already in Cargo.toml |
| uuid | 1.x (v7 feature) | UUIDv7 message IDs | Already in Cargo.toml |
| chrono | 0.4.x | RFC 3339 timestamps | Already in Cargo.toml |
| tracing + tracing-subscriber | 0.1/0.3 | Structured logging | Already in Cargo.toml |
| base64 | 0.22.x | Key encoding | Already in Cargo.toml |

### New Dependencies for Phase 7

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| reqwest | 0.13.2 | HTTP client for outbound relay POST | Consensus Rust HTTP client. Supports mTLS via `Identity` type. Uses rustls by default. 414M+ downloads. |
| rustls | 0.23.x | Direct TLS dependency for mTLS ServerConfig | Already indirect dep. Direct access needed for `WebPkiClientVerifier` and `ClientConfig` builders. |
| tokio-rustls | 0.26.4 | Async TLS acceptor for federation listener | Wraps rustls `ServerConfig` into `TlsAcceptor` for Tokio `TcpListener`. Required because `axum::serve()` does not natively accept TLS. |
| rustls-pki-types | 1.14.0 | PEM parsing and PKI types | Shared types for rustls ecosystem. Provides `CertificateDer`, `PrivateKeyDer`, PEM reader iterators. Replaces unmaintained `rustls-pemfile`. |
| clap | 4.6.0 | CLI argument parsing | Standard Rust CLI parser. Derive API for clean flag definitions. Required for `--mode`, `--port`, `--federation-port`, etc. |

**NOTE on D-09:** The user decision mentions `rustls-pemfile` for PEM loading. However, `rustls-pemfile` is unmaintained since August 2025 (RUSTSEC-2025-0134). Its functionality is subsumed by `rustls-pki-types` since version 1.9.0. Use `rustls-pki-types` instead. The user also mentions `axum-server` for rustls integration, but for mTLS we need direct `ServerConfig` control via `tokio-rustls` -- `axum-server` abstracts this away. These are implementation details within Claude's discretion.

### NOT Adding (Phase 7)

| Library | Why Not |
|---------|---------|
| redb | Persistent offline queues are Phase 8 (FREL-01). Phase 7 uses in-memory queues only. |
| rcgen | D-07 specifies pre-generated certs checked into `demo/certs/`, not runtime generation. Use `openssl` CLI or a one-time script to generate certs. rcgen is not needed. |
| dashmap | `Arc<Mutex<HashMap>>` is sufficient for 2-server demo scale. |
| jsonrpsee | Existing hand-rolled JSON-RPC in `protocol.rs` is consistent. Don't introduce a second pattern. |
| PostgreSQL/sqlx | Demo is single-binary. No external database. |

**Cargo.toml additions:**
```toml
# Federation: HTTP client for server-to-server relay
reqwest = { version = "0.13", default-features = false, features = ["rustls-tls", "json"] }

# Federation: TLS configuration (direct dep for mTLS ServerConfig)
rustls = { version = "0.23", default-features = false, features = ["std"] }
tokio-rustls = "0.26"
rustls-pki-types = "1.14"

# CLI argument parsing for federation mode
clap = { version = "4.6", features = ["derive"] }
```

### Version Verification

| Crate | Verified Version | Source | Date Checked |
|-------|-----------------|--------|--------------|
| reqwest | 0.13.2 | `cargo search reqwest` | 2026-03-28 |
| rustls | 0.23.x (latest stable 0.23.36) | `cargo search rustls` | 2026-03-28 |
| tokio-rustls | 0.26.4 | `cargo search tokio-rustls` | 2026-03-28 |
| rustls-pki-types | 1.14.0 | `cargo search rustls-pki-types` | 2026-03-28 |
| clap | 4.6.0 | `cargo search clap` | 2026-03-28 |

## Architecture Patterns

### Two-Listener Architecture

The server runs two TCP listeners simultaneously:

```
AINM Server (single binary, federation mode)
=============================================

Listener 1: Plain TCP (existing)        Listener 2: mTLS TCP (new)
  Port: --port (e.g., 8080)               Port: --federation-port (e.g., 8443)
  Routes:                                  Routes:
    /ws (WebSocket upgrade)                  POST /ainm/v1/relay
    /.well-known/ainm/{name}                 GET /.well-known/ainm/server
    /.well-known/ainm/helper/agent-card.json
```

**Why two listeners:** The WebSocket endpoint for local clients does NOT need mTLS (uses Ed25519 handshake per transport.md Section 4.3). The federation relay REQUIRES mTLS (transport.md Section 5.1). Mixing them on one listener would require optional client certs, which complicates the security model.

**Implementation pattern:** Two `tokio::spawn` tasks, one for each listener. The mTLS listener wraps `TcpListener` with `tokio_rustls::TlsAcceptor` before handing connections to Axum via the low-level connection handling loop (not `axum::serve` which doesn't support TLS natively).

```rust
// Plain TCP listener (existing pattern)
let plain_listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
let plain_handle = tokio::spawn(async move {
    axum::serve(plain_listener, plain_app).await.ok();
});

// mTLS listener (new pattern, from axum low-level-rustls example)
let tls_listener = TcpListener::bind(format!("0.0.0.0:{}", federation_port)).await?;
let tls_acceptor = TlsAcceptor::from(server_tls_config);
let fed_handle = tokio::spawn(async move {
    loop {
        let (stream, _addr) = tls_listener.accept().await.unwrap();
        let tls_acceptor = tls_acceptor.clone();
        let app = fed_app.clone();
        tokio::spawn(async move {
            if let Ok(tls_stream) = tls_acceptor.accept(stream).await {
                // Serve with hyper, passing axum router as tower service
            }
        });
    }
});
```

### Module Structure

```
demo/src/
  main.rs          -- CLI parsing (clap), mode branching, startup orchestration
  server.rs        -- Existing WebSocket server (MODIFY: add identity endpoints)
  federation.rs    -- NEW: relay handler (inbound), relay sender (outbound)
  tls.rs           -- NEW: TLS config loading, ServerConfig with WebPkiClientVerifier
  relay.rs         -- NEW: send queue, per-destination backoff, relay payload construction
  identity.rs      -- MODIFY: add ServerIdentityRecord, remote identity resolution
  signing.rs       -- MODIFY: add server-level signing (reuse sign_envelope)
  protocol.rs      -- Existing JSON-RPC constructors (minimal changes)
  agent.rs         -- Existing agent logic (reuse as-is on Server B)
  repl.rs          -- Existing REPL (reuse as-is on Server A)
  render.rs        -- Existing rendering (no changes)
  streaming.rs     -- Existing streaming (no changes)
```

### Recommended Data Flow: Cross-Server Message

```
1. alice@server-a.local types message in REPL
2. REPL sends ainm.message.send via WebSocket to Server A
3. Server A: verify Alice's Ed25519 signature (existing)
4. Server A: look up thread participants
   - Thread ID: "server-a.local:01JQXYZ..."  => origin = server-a.local (this server)
   - Participants: alice@server-a.local (local), helper@server-b.local (remote)
5. Server A: deliver locally to alice (echo-back if needed)
6. Server A: enqueue relay to server-b.local
7. Federation Sender:
   a. Resolve server-b.local identity (GET /.well-known/ainm/server, cache result)
   b. Build relay payload: { messages: [...], from_server: "server-a.local" }
   c. Sign relay payload with Server A's Ed25519 key (server-level signature)
   d. POST https://server-b.local:{fed_port}/ainm/v1/relay (mTLS)
8. Server B: Federation Relay Handler
   a. Verify mTLS cert fingerprint matches server-a.local's published fingerprint
   b. Verify server_signature against server-a.local's public key
   c. Verify each message's individual Ed25519 signature
   d. Deliver to helper@server-b.local (local agent)
9. Agent on Server B processes message, generates response
10. Server B: response message for thread "server-a.local:01JQXYZ..."
    - Origin is server-a.local (remote) => relay response to origin
11. Server B relays response to Server A (same mTLS + signing flow)
12. Server A receives relay, verifies triple-layer, delivers to alice via WebSocket
```

### Thread ID Format

Per D-12: `{origin-domain}:{uuidv7}`

```rust
pub fn new_federated_thread_id(origin_domain: &str) -> String {
    format!("{}:{}", origin_domain, uuid::Uuid::now_v7())
}

pub fn parse_origin_domain(thread_id: &str) -> Option<&str> {
    thread_id.split_once(':').map(|(domain, _)| domain)
}
```

### CLI Argument Structure

```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "ainm-demo", about = "AINM Protocol Demo")]
struct Cli {
    /// Server mode: human (REPL) or agent (headless)
    #[arg(long)]
    mode: Option<String>,  // None = legacy single-server mode

    /// Local WebSocket port
    #[arg(long, default_value = "8080")]
    port: u16,

    /// Federation relay port (mTLS)
    #[arg(long, default_value = "8443")]
    federation_port: u16,

    /// Server identity domain (e.g., server-a.local)
    #[arg(long)]
    identity: Option<String>,

    /// Path to TLS certificate PEM
    #[arg(long)]
    cert: Option<String>,

    /// Path to TLS private key PEM
    #[arg(long)]
    key: Option<String>,

    /// Path to CA certificate PEM for mTLS verification
    #[arg(long)]
    ca_cert: Option<String>,

    /// Remote server to federate with (e.g., server-b.local:8443)
    #[arg(long)]
    remote: Option<String>,
}
```

When `mode` is `None`, launch legacy single-server demo (D-06 backward compatibility). When `mode` is set, launch federation mode.

### Anti-Patterns to Avoid

- **Single broadcast channel for routing:** The current `to_agent_tx`/`to_human_tx` pattern cannot route to remote servers. Replace with a message router that checks participant location (local vs remote) and dispatches accordingly.
- **Blocking identity resolution on relay hot path:** Cache remote server identities aggressively (1-hour TTL per spec). Never block relay processing on a synchronous HTTP fetch.
- **Sharing ServerConfig between listeners:** The plain TCP listener has no TLS. The federation listener uses mTLS. They are fundamentally different configurations. Don't try to unify them.
- **Hard-coding two participants:** The current server assumes exactly one human and one agent. Federation requires participant-addressed routing based on the thread registry.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP client with mTLS | Custom hyper client with manual TLS config | reqwest with `Identity::from_pem()` and `Certificate::from_pem()` | mTLS client configuration is error-prone. reqwest handles certificate chains, connection pooling, and retry correctly. |
| TLS acceptor | Manual TLS stream wrapping | tokio-rustls `TlsAcceptor` with rustls `ServerConfig` | The TLS handshake state machine is complex. tokio-rustls handles async TLS negotiation correctly. |
| PEM file parsing | Manual PEM parsing | `rustls-pki-types` `CertificateDer::pem_file_iter()` and `PrivateKeyDer::from_pem_file()` | PEM format has edge cases (multiple certs in one file, different key encodings). The rustls ecosystem handles all of them. |
| CLI argument parsing | Manual `std::env::args()` parsing | clap derive API | Flag validation, help text, error messages, default values -- all handled by clap. |
| JSON-RPC 2.0 framing | (already hand-rolled in protocol.rs) | Keep existing `protocol.rs` | The existing implementation is correct and tested. Don't add jsonrpsee for a second pattern. |
| Certificate fingerprinting | Manual DER encoding + SHA-256 | `ring::digest::digest(&ring::digest::SHA256, der_bytes)` | Already using ring. Certificate fingerprint is `sha256:` + hex of SHA-256 of DER-encoded cert. |
| Exponential backoff | Manual timer + retry counter | Simple struct with `attempt_count`, `base_delay`, `max_delay`, `next_retry_at` | This IS worth hand-rolling -- it's 20 lines of code and avoids a dependency for a trivial algorithm. |

**Key insight:** The existing codebase already hand-rolls JSON-RPC and signing. Continue that pattern. Only add dependencies for genuinely complex problems (TLS, HTTP client, CLI parsing).

## Common Pitfalls

### Pitfall 1: mTLS Fingerprint Verification is Not Just "TLS Works"

**What goes wrong:** A successful TLS handshake does NOT mean the remote server is who they claim to be. The mTLS handshake only verifies the certificate chain. You must ALSO verify the certificate's SHA-256 fingerprint against the value published at `/.well-known/ainm/server`.
**Why it happens:** Developers test with self-signed certs from a shared CA. The TLS handshake always succeeds because both servers trust the CA. They skip fingerprint verification and don't notice until a different CA-signed cert connects.
**How to avoid:** After accepting the TLS connection, extract the peer certificate from the TLS session, compute its SHA-256 fingerprint, and compare against the cached server identity's `tls_fingerprint` field. Reject if mismatch. Implement this as the FIRST check in the relay handler.
**Warning signs:** Relay requests accepted from any server with a cert signed by the trusted CA, regardless of which server they claim to be.

### Pitfall 2: Server Signature vs Message Signature Confusion

**What goes wrong:** The relay payload has two types of signatures: (1) `server_signature` on the relay params (signed by the sending server's Ed25519 key), and (2) individual `signature` on each message (signed by the message sender's Ed25519 key). Developers mix up which key verifies which signature.
**Why it happens:** Both use Ed25519. Both use JCS canonicalization. The only difference is which key pair signs which data.
**How to avoid:** Server signature: canonicalize `params` without `server_signature` field, verify against `from_server`'s public key (from `/.well-known/ainm/server`). Message signature: canonicalize the message envelope without `signature` field, verify against the message sender's public key (from `/.well-known/ainm/{name}@{provider}`).
**Warning signs:** Signature verification passing when it shouldn't, or failing with correct data.

### Pitfall 3: Thread Origin Routing Loop

**What goes wrong:** With D-11 (all messages route through origin), a reply from Server B to a thread owned by Server A must go to Server A first, then Server A fans out. If Server B tries to deliver the reply directly to local participants instead of routing through the origin, message ordering breaks.
**Why it happens:** It's tempting to optimize: "the agent is local to Server B, why relay through Server A?" But the origin assigns sequence numbers. Without routing through origin, the thread has inconsistent ordering across servers.
**How to avoid:** When a local participant sends a message to a thread where the origin is remote, ALWAYS relay to the origin server. Only the origin server assigns sequence numbers and fans out to all participating servers.
**Warning signs:** Messages appearing in different orders on Server A and Server B.

### Pitfall 4: Docker Networking vs localhost

**What goes wrong:** Inside Docker containers, `localhost` refers to the container itself, not the host or other containers. If Server A tries to connect to `localhost:8443` to reach Server B, it connects to itself.
**Why it happens:** The demo works locally without Docker because both servers share the host's network namespace. Docker isolates each service.
**How to avoid:** Use Docker service names (e.g., `server-a`, `server-b`) for inter-container communication. These resolve via Docker's built-in DNS. Configure server identity with the service name as the domain. The `--identity` flag should be `server-a.local` or `server-b.local`, and DNS should map to Docker service names.
**Warning signs:** Connection refused errors in Docker that don't occur locally.

### Pitfall 5: Blocking the Relay Handler on Identity Resolution

**What goes wrong:** When a relay arrives with messages from unknown senders, the handler tries to resolve each sender's identity via HTTP GET before processing the batch. This blocks the relay response, potentially timing out.
**Why it happens:** Phase 7 is a 2-server demo with known participants. The developer doesn't notice the performance issue because identities are always cached.
**How to avoid:** Pre-seed the identity cache for known participants during startup. For Phase 7's 2-server demo, this means each server resolves the other server's participants at boot time. For future phases, implement async identity resolution with background fetch.
**Warning signs:** Relay requests taking > 1 second when identity cache is cold.

## Code Examples

### mTLS Server Configuration (rustls)

```rust
// Source: rustls docs + axum low-level-rustls example
use rustls::server::WebPkiClientVerifier;
use rustls::ServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use std::sync::Arc;

fn build_mtls_server_config(
    ca_cert_path: &str,
    server_cert_path: &str,
    server_key_path: &str,
) -> Arc<ServerConfig> {
    // Load CA cert for client verification
    let ca_certs: Vec<CertificateDer> = CertificateDer::pem_file_iter(ca_cert_path)
        .unwrap()
        .map(|c| c.unwrap())
        .collect();

    let mut root_store = rustls::RootCertStore::empty();
    for cert in &ca_certs {
        root_store.add(cert.clone()).unwrap();
    }

    // Build client verifier (requires client cert from trusted CA)
    let client_verifier = WebPkiClientVerifier::builder(Arc::new(root_store))
        .build()
        .unwrap();

    // Load server cert and key
    let server_certs: Vec<CertificateDer> = CertificateDer::pem_file_iter(server_cert_path)
        .unwrap()
        .map(|c| c.unwrap())
        .collect();
    let server_key = PrivateKeyDer::from_pem_file(server_key_path).unwrap();

    let mut config = ServerConfig::builder()
        .with_client_cert_verifier(client_verifier)
        .with_single_cert(server_certs, server_key)
        .expect("bad certificate/key");

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Arc::new(config)
}
```

### mTLS Client Configuration (reqwest)

```rust
// Source: reqwest docs, seanmonstar blog on reqwest v0.13
use reqwest::{Certificate, Identity};
use std::fs;

fn build_mtls_client(
    ca_cert_path: &str,
    client_cert_path: &str,
    client_key_path: &str,
) -> reqwest::Client {
    // Load CA cert to verify the remote server
    let ca_pem = fs::read(ca_cert_path).unwrap();
    let ca_cert = Certificate::from_pem(&ca_pem).unwrap();

    // Load our cert + key for client authentication
    let mut client_pem = fs::read(client_cert_path).unwrap();
    let key_pem = fs::read(client_key_path).unwrap();
    client_pem.extend_from_slice(&key_pem);  // Identity::from_pem needs cert+key concatenated
    let identity = Identity::from_pem(&client_pem).unwrap();

    reqwest::Client::builder()
        .add_root_certificate(ca_cert)
        .identity(identity)
        .use_rustls_tls()
        .build()
        .unwrap()
}
```

### Relay Payload Construction and Signing

```rust
// Reuses existing signing.rs pattern
fn build_relay_payload(
    messages: Vec<serde_json::Value>,
    server_id: &str,
    server_keypair: &ring::signature::Ed25519KeyPair,
) -> serde_json::Value {
    let mut params = serde_json::json!({
        "messages": messages,
        "from_server": server_id,
    });

    // Sign params (same pattern as sign_envelope in signing.rs)
    let canonical = serde_json_canonicalizer::to_string(&params).unwrap();
    let sig = server_keypair.sign(canonical.as_bytes());
    let sig_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(sig.as_ref());

    params.as_object_mut().unwrap()
        .insert("server_signature".to_string(), serde_json::json!(sig_b64));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "ainm.relay",
        "params": params
    })
}
```

### Certificate Fingerprint Computation

```rust
// Source: ring docs
fn compute_cert_fingerprint(der_cert: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, der_cert);
    let hex: String = digest.as_ref().iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    format!("sha256:{}", hex)
}
```

### Per-Destination Send Queue with Exponential Backoff

```rust
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

struct SendQueue {
    queues: HashMap<String, DestinationQueue>,
}

struct DestinationQueue {
    messages: VecDeque<serde_json::Value>,
    backoff: BackoffState,
}

struct BackoffState {
    attempt: u32,
    base_delay: Duration,
    max_delay: Duration,
    next_retry: Option<Instant>,
}

impl BackoffState {
    fn new() -> Self {
        Self {
            attempt: 0,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            next_retry: None,
        }
    }

    fn record_failure(&mut self) {
        self.attempt += 1;
        let delay = self.base_delay * 2u32.pow(self.attempt.min(6));
        let capped = delay.min(self.max_delay);
        self.next_retry = Some(Instant::now() + capped);
    }

    fn record_success(&mut self) {
        self.attempt = 0;
        self.next_retry = None;
    }

    fn can_retry(&self) -> bool {
        self.next_retry.map_or(true, |t| Instant::now() >= t)
    }
}
```

### Server Identity Endpoint

```rust
// Source: transport.md Section 5.2 + identity.schema.json WellKnownAINMConfig
async fn server_identity_handler(
    State(state): State<FederationState>,
) -> impl IntoResponse {
    let body = serde_json::json!({
        "server_id": state.server_id,
        "public_key": state.server_public_key_b64,
        "relay_endpoint": format!("https://{}:{}/ainm/v1/relay",
            state.server_id, state.federation_port),
        "tls_fingerprint": state.tls_fingerprint,
        "supported_versions": ["0.1.0"]
    });

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json"),
         (header::CACHE_CONTROL, "public, max-age=3600")],
        serde_json::to_string(&body).unwrap(),
    )
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `rustls-pemfile` for PEM parsing | `rustls-pki-types` PEM functions | August 2025 (RUSTSEC-2025-0134) | Must use `rustls-pki-types` 1.9+ instead |
| `docker-compose` (hyphenated CLI) | `docker compose` (space, Compose v2 plugin) | 2023 | Use `docker compose` command in scripts and docs |
| `reqwest` native-tls default | `reqwest` 0.13+ rustls default | 2025 | No longer need to explicitly disable native-tls in most cases, but we still set `default-features = false` for clarity |

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | All compilation | Yes | 1.94.0 (2024 edition) | -- |
| cargo | Build system | Yes | 1.94.0 | -- |
| Docker | DEMO-01, DEMO-02 | **No** | -- | Local multi-process demo (see below) |
| Docker Compose | DEMO-01 | **No** | -- | Local multi-process demo (see below) |
| openssl CLI | Cert generation script | Yes | 3.6.1 | rcgen one-time binary (or use pre-committed certs) |

**Missing dependencies with no fallback:**
- None -- all blockers have fallbacks.

**Missing dependencies with fallback:**
- **Docker / Docker Compose:** Not installed on the development machine. The planner MUST account for this:
  - The Dockerfile and docker-compose.yml should still be created and committed (they work on any machine with Docker).
  - For local testing during development, the demo can be run as two separate processes in two terminal tabs: `ainm-demo --mode human --identity server-a.local ...` and `ainm-demo --mode agent --identity server-b.local ...`. This is actually the D-01 interaction model (two interactive terminals).
  - The Docker demo can be validated in CI (GitHub Actions) or by installing Docker Desktop later.

## Certificate Generation Strategy

D-07 specifies pre-generated certs checked into `demo/certs/`. Since `openssl` CLI is available (v3.6.1), the generation script uses openssl commands:

```bash
#!/bin/bash
# demo/certs/generate.sh

# Generate CA
openssl req -x509 -newkey ed25519 -days 3650 -nodes \
  -keyout ca-key.pem -out ca.pem \
  -subj "/CN=AINM Demo CA"

# Generate Server A
openssl req -newkey ed25519 -nodes \
  -keyout server-a-key.pem -out server-a.csr \
  -subj "/CN=server-a.local" \
  -addext "subjectAltName=DNS:server-a.local,DNS:server-a"
openssl x509 -req -in server-a.csr -CA ca.pem -CAkey ca-key.pem \
  -CAcreateserial -out server-a.pem -days 3650 \
  -copy_extensions copyall

# Generate Server B
openssl req -newkey ed25519 -nodes \
  -keyout server-b-key.pem -out server-b.csr \
  -subj "/CN=server-b.local" \
  -addext "subjectAltName=DNS:server-b.local,DNS:server-b"
openssl x509 -req -in server-b.csr -CA ca.pem -CAkey ca-key.pem \
  -CAcreateserial -out server-b.pem -days 3650 \
  -copy_extensions copyall

# Cleanup CSRs
rm -f *.csr *.srl
```

**Why Ed25519 TLS certs:** Matches the project's Ed25519-everywhere approach. Smaller certs, faster handshakes. Supported by OpenSSL 3.x and rustls 0.23+.

**Why 10-year expiry:** These are demo certs. They should not expire during development. Production certs would use shorter lifetimes.

## Open Questions

1. **Thread creation across federation**
   - What we know: D-10 says first message creates thread on sender's home server. D-12 says thread ID embeds origin domain.
   - What's unclear: When alice@server-a.local creates a thread mentioning helper@server-b.local, how does Server B learn about the thread? Does Server A send a `thread.create` relay, or does Server B infer it from the first relayed message?
   - Recommendation: Server B infers thread existence from the first relayed message. No separate `thread.create` relay needed for Phase 7. The thread registry on Server B creates an entry when it receives a relay for an unknown thread ID. This is simpler and sufficient for the 2-server demo.

2. **Server A and Server B peer configuration**
   - What we know: Each server needs to know about the other for initial discovery.
   - What's unclear: Does each server pre-configure its peer, or does it discover peers dynamically when a message targets a remote participant?
   - Recommendation: For Phase 7, each server pre-configures its peer via the `--remote` CLI flag (e.g., `--remote server-b.local:8443`). Dynamic discovery from `@provider` domain parsing can be added later. The demo has exactly two servers -- pre-configuration is simpler and avoids DNS complexity in Docker.

3. **Partition behavior during Phase 7**
   - What we know: STATE.md flags "Partition behavior not fully specified" as a research concern.
   - What's unclear: What happens when the origin server is down and a remote participant tries to send a message?
   - Recommendation: For Phase 7, keep it simple: messages to unreachable servers queue in the in-memory send queue with exponential backoff. If the origin is down, the non-origin server queues the message and retries. No degraded mode, no timeout. Phase 8 adds persistent queues and partition healing (FREL-01 through FREL-04).

## Project Constraints (from CLAUDE.md)

- **Protocol-first:** Spec must be complete and implementable before building. The transport spec (Sections 5.1-5.4) is already complete for federation relay.
- **Open standard:** All code is open source.
- **Federation:** No single point of control. This phase IS the federation implementation.
- **Transport:** WebSocket for client-server, HTTP POST for server-server (both confirmed in CLAUDE.md).
- **Security:** Cryptographic identity and message signing are non-negotiable. Triple-layer validation (mTLS + server signature + message signature) is the core security requirement.
- **Pure Rust TLS:** Use rustls, not OpenSSL. Confirmed by both CLAUDE.md and D-09.
- **Single binary:** Demo is `ainm-demo` binary. No external database, no separate processes for different components.
- **Axum 0.8:** Web framework is locked. Don't introduce alternatives.
- **ring 0.17:** Crypto library is locked. Don't introduce alternatives.

## Sources

### Primary (HIGH confidence)
- `spec/transport.md` Sections 5.1-5.4 -- Relay endpoint format, server identity, relay request/response, validation rules
- `spec/identity.md` Section 4 -- Identity resolution for cross-server participant lookup
- `spec/security.md` Section 4 -- Signing procedure for server-level signatures
- `schemas/transport.schema.json` -- `RelayParams`, `RelayResult` formal schemas
- `schemas/identity.schema.json` -- `WellKnownAINMConfig` formal schema
- `.planning/research/STACK.md` -- Verified crate versions and federation dependencies
- `.planning/research/ARCHITECTURE.md` -- Federation architecture design, component boundaries, data flow
- `.planning/research/PITFALLS.md` -- Federation-specific pitfalls and prevention strategies
- Existing codebase: `demo/src/server.rs`, `identity.rs`, `signing.rs`, `protocol.rs`, `main.rs`

### Secondary (MEDIUM confidence)
- [rustls WebPkiClientVerifier docs](https://docs.rs/rustls/0.23.12/rustls/server/struct.WebPkiClientVerifier.html) -- mTLS server configuration API
- [reqwest v0.13 blog post](https://seanmonstar.com/blog/reqwest-v013-rustls-default/) -- mTLS client configuration
- [axum low-level-rustls example](https://github.com/tokio-rs/axum/blob/main/examples/low-level-rustls/src/main.rs) -- TLS acceptor pattern with Axum
- [clap derive tutorial](https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html) -- CLI flag parsing
- `cargo search` results for crate version verification (2026-03-28)

### Tertiary (LOW confidence)
- None -- all findings verified against primary or secondary sources.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all crates verified via `cargo search`, existing codebase patterns well understood
- Architecture: HIGH -- transport spec fully defines the relay protocol; existing project research covers architecture in depth
- Pitfalls: HIGH -- grounded in project research (`.planning/research/PITFALLS.md`) which draws from Matrix, XMPP, ActivityPub real-world experience
- Environment: MEDIUM -- Docker not available on dev machine; fallback strategy is viable but untested

**Research date:** 2026-03-28
**Valid until:** 2026-04-28 (stable domain -- Rust crate ecosystem, established spec)
