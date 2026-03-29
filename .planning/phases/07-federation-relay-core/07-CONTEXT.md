# Phase 7: Federation Relay Core - Context

**Gathered:** 2026-03-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Two AINM servers discover each other via `.well-known/ainm/server`, authenticate with mutual TLS, and relay messages via HTTP POST with JSON-RPC 2.0 framing. The origin server is authoritative for thread state (participant list, ordering). A docker-compose demo proves cross-server human-agent conversation end-to-end.

Requirements: RELY-01, RELY-02, RELY-03, RELY-04, RELY-05, FAUT-01, FAUT-02, FAUT-03, DEMO-01, DEMO-02

</domain>

<decisions>
## Implementation Decisions

### Demo Interaction Model
- **D-01:** Two interactive terminals — human REPL on Server A, structured federation logs on Server B (headless agent). User types on A, watches federation activity on B.
- **D-02:** Participants: alice@server-a.local (human) on Server A, helper@server-b.local (agent) on Server B. Proves core value: human-agent conversation across federation.
- **D-03:** Server B runs headless — no interactive REPL. Its terminal shows relay receipt, signature validation, agent processing, and outgoing relay messages.

### Codebase Evolution
- **D-04:** Extend the existing `demo/src/` binary — add new modules (`relay.rs`, `federation.rs`, `tls.rs`), extend existing modules (`identity.rs`, `signing.rs`, `server.rs`). No new crate or workspace.
- **D-05:** Server mode configured via CLI flags: `--mode human|agent`, `--port`, `--federation-port`, `--identity`, `--cert`, `--key`. Docker-compose friendly.
- **D-06:** Backward compatible — running `ainm-demo` with no flags still launches the original single-server demo. Federation mode requires explicit flags.

### TLS & Certificate Setup
- **D-07:** Pre-generated self-signed CA + server certificates checked into `demo/certs/` with a README explaining how to regenerate. Deterministic, `docker-compose up` just works.
- **D-08:** Certificate structure: `ca.pem`, `ca-key.pem`, `server-a.pem`, `server-a-key.pem`, `server-b.pem`, `server-b-key.pem`.
- **D-09:** Use `rustls` (pure Rust, no OpenSSL) with `rustls-pki-types` 1.14+ for PEM loading (replaces `rustls-pemfile` per RUSTSEC-2025-0134). Use `tokio-rustls` + `hyper-util` for mTLS listener (replaces `axum-server` which doesn't expose `ServerConfig` for mTLS client-cert verification). Updated 2026-03-28 — user approved substitution.

### Thread Ownership & Routing
- **D-10:** First message in a thread creates the thread on the sender's home server. That server becomes the origin — sole authority for participant list and message ordering.
- **D-11:** All messages in a thread route through the origin server. Non-origin servers send replies to origin; origin assigns sequence numbers and fans out to all participating servers.
- **D-12:** Thread ID embeds origin domain: `server-a.local:01JQXYZ7K8M3N4P5Q6R7S8T9` (domain prefix + UUIDv7). Any server can parse the origin from the thread ID without a lookup.

### Claude's Discretion
- Send queue implementation details (in-memory for Phase 7, per-destination with exponential backoff)
- Exact CLI flag names and parsing library (clap recommended)
- Internal module boundaries between relay.rs and federation.rs
- Log formatting and verbosity levels for Server B's headless output

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Protocol Specification
- `spec/transport.md` -- Defines relay endpoint format (Section 5.1-5.4), `.well-known/ainm/server` response format, relay request/response JSON-RPC, 3-layer validation rules, error codes
- `spec/identity.md` -- Identity resolution model, `name@provider` format, public key distribution
- `spec/security.md` -- Ed25519 signing, JCS canonicalization, signature verification

### JSON Schemas
- `schemas/transport.schema.json` -- Formal schema for relay payloads and transport messages
- `schemas/identity.schema.json` -- Identity record schemas
- `schemas/security.schema.json` -- Signature and key schemas

### AsyncAPI
- `asyncapi.yaml` -- Transport binding definitions including federation relay operations

### Existing Implementation
- `demo/src/server.rs` -- Current Axum server setup, WebSocket handler, agent card endpoint
- `demo/src/identity.rs` -- Identity store, keypair management (extend for server-level identity)
- `demo/src/signing.rs` -- Ed25519 signing with ring (extend for server-level signatures)
- `demo/src/protocol.rs` -- JSON-RPC message construction and parsing
- `demo/src/agent.rs` -- Agent response logic (reuse as-is for Server B)
- `demo/src/main.rs` -- Entry point (refactor for config-driven mode selection)
- `demo/Cargo.toml` -- Dependencies (add rustls, rustls-pemfile, clap, reqwest/hyper for HTTP client)

### Project Context
- `.planning/REQUIREMENTS.md` -- Full requirement definitions (RELY-01 through DEMO-02 for Phase 7)
- `.planning/ROADMAP.md` -- Phase success criteria and dependency chain

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `signing.rs`: Ed25519 signing via `ring` crate — extend to sign relay payloads at the server level (currently only signs individual messages)
- `identity.rs`: `IdentityStore` with `register()` and lookup — extend to store server-level identities (not just participant identities)
- `protocol.rs`: JSON-RPC 2.0 message construction — reuse for `ainm.relay` method framing
- `server.rs`: Axum `Router` setup with `.well-known` endpoint — extend to add `/ainm/v1/relay` POST handler and `/.well-known/ainm/server` GET handler
- `agent.rs`: Full agent response logic (streaming, tool calls, interactive UI) — reuse as-is on Server B
- `repl.rs`: Interactive REPL with message display — reuse as-is on Server A

### Established Patterns
- JSON-RPC 2.0 framing for all protocol operations
- Ed25519 signing via `ring` with JCS canonicalization via `serde_json_canonicalizer`
- `broadcast::channel` for inter-component message passing within a server
- `Arc<Mutex<>>` for shared mutable state

### Integration Points
- `server.rs` Router: Add relay POST endpoint and server identity endpoint alongside existing WebSocket and agent card routes
- `main.rs` entrypoint: Add CLI argument parsing (clap) to select mode, then branch startup logic
- `ServerState`: Extend to include server-level keypair, federation config, and send queue handles
- Docker: New `demo/Dockerfile` and `demo/docker-compose.yml` to orchestrate two server instances

</code_context>

<specifics>
## Specific Ideas

- Demo UX preview: Terminal 1 shows `[alice@server-a.local] >` prompt, Terminal 2 shows structured logs with relay receipt, signature validation steps, agent processing
- Thread ID format explicitly: `{origin-domain}:{uuidv7}` — colon separator, domain first
- The fan-out model (origin assigns sequence, relays to all) is important for future 3+ server scenarios even though Phase 7 only demos 2 servers

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 07-federation-relay-core*
*Context gathered: 2026-03-28*
