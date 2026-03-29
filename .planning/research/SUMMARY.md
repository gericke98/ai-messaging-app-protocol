# Project Research Summary

**Project:** AI-Native Messaging Protocol — v3.2 Federation
**Domain:** Multi-server federation for an existing AI-native messaging protocol
**Researched:** 2026-03-28
**Confidence:** HIGH

## Executive Summary

AINM v3.2 is a focused federation milestone that extends an already-working single-server AI-native messaging protocol to operate across multiple independent servers. The v3.1 foundation is solid: email-like addressing (`name@provider`), Ed25519 message signing, JSON-RPC 2.0 framing, WebSocket transport, and server identity at `.well-known/ainm/server` are all already specified and running in the demo. The v3.2 work is primarily implementation — making the relay endpoint actually function end-to-end, adding mTLS between servers, building a persistent offline relay queue, and proving the federation flow with a Docker Compose two-server demo. Six new Rust crate dependencies cover the entire surface area: `reqwest`, `rcgen`, `rustls` (direct dep), `tokio-rustls`, `rustls-pki-types`, and `redb`.

The single most consequential architectural decision — already made in the spec — is **origin-server authority** over Matrix-style DAG-based eventual consistency. This eliminates the most complex part of federated messaging: state resolution algorithms. Matrix's DAG approach produced a 2000-line state resolution algorithm, rooms that take 10-60 minutes to join, and databases reaching 45GB in production. Origin-server authority means the server where a thread was created is the sole authority for that thread's state; remote servers cache but defer. The trade-off is that threads become temporarily read-only when the origin is unreachable — acceptable for v1, analogous to how email works (if gmail.com goes down, you cannot send from it). Research from Matrix, XMPP, and ActivityPub post-mortems unanimously validates choosing simplicity here.

The key risks for v3.2 are operational and security-focused, not architectural. Certificate lifecycle management for mTLS breaks federation silently and invisibly when certificates expire. Replay attacks are feasible because Ed25519 signatures are permanent while the deduplication window is finite. Backfill can leak pre-join message history to unauthorized servers. All three must be addressed in an authentication hardening phase — they cannot be retrofitted without breaking federation compatibility. The recommended build order is: establish the relay core first (prove federation works), harden authentication second (make it secure), then add reliability and AI-native differentiation features third (offline queues, federated presence, selective backfill).

## Key Findings

### Recommended Stack

The existing stack (Axum 0.8, Tokio 1.x, ring 0.17, serde/serde_json, tokio-tungstenite 0.26) is unchanged. Six new direct dependencies cover the full federation surface area. All versions were verified against crates.io on 2026-03-28.

**Core new technologies:**
- `reqwest 0.13` (features: rustls-tls, json): mTLS HTTP client for server-to-server relay POST — consensus standard Rust HTTP client, 414M+ downloads, integrates with existing rustls stack
- `rcgen 0.14`: in-process X.509 certificate generation — pure Rust, maintained by the rustls team, eliminates external CA tooling for demo; init container generates shared CA + per-server leaf certs
- `rustls 0.23` (direct dep): `ServerConfig` with `WebPkiClientVerifier` for mTLS — already an indirect dep via tokio-tungstenite, making it direct pins the version and gives access to mTLS config builders
- `tokio-rustls 0.26`: async TLS acceptor — wraps rustls `ServerConfig` into Tokio-compatible `TlsAcceptor`, required because `axum::serve()` does not natively accept TLS
- `rustls-pki-types 1.14`: PEM parsing and PKI types — replaces unmaintained `rustls-pemfile` (RUSTSEC-2025-0134, archived August 2025)
- `redb 3`: persistent offline relay queue — pure Rust, ACID transactions, stable format since 1.0 (2023); correct choice over sled (pre-1.0, unstable format) and SQLite (C dependency breaks single-binary approach)

**Critical version note:** Do not add `rustls-pemfile`. It is unmaintained as of August 2025 (RUSTSEC-2025-0134). Use `rustls-pki-types 1.14` instead for all PEM parsing.

**Architecture for mTLS in demo:** Two TCP listeners — one plain (existing WebSocket for local clients, unchanged), one TLS-wrapped (new, for federation relay endpoint only). This keeps existing client code untouched while adding mTLS for S2S. A Docker Compose init container generates the shared CA + leaf certs via rcgen and writes to a shared volume.

### Expected Features

**Must have — P1 (v3.2 launch, demo works):**
- TS-2: Server discovery and identity verification — `/.well-known/ainm/server` endpoint, foundation for all federation
- TS-8: Cross-server identity resolution — HTTPS fetch to remote `.well-known/ainm/{name}` with 1-hour TTL cache
- TS-5: Origin-server authority for thread state — the critical architectural decision, must be locked in first
- TS-1: Cross-server message delivery — the relay round-trip end-to-end
- TS-10: Relay delivery confirmation/rejection — originating server must know delivery outcome per-message
- TS-4: Message deduplication at relay level — at-least-once delivery guarantees duplicates will arrive
- TS-6: Federated thread creation with remote participants
- TS-7: Federated thread join/leave propagation to all participating servers
- TS-9: Federation rate limiting — error code `-32022` with `retry_after`, per-source-server limits

**Should have — P2 (v3.2 complete, reliable):**
- TS-3: Offline relay queues (redb-backed, 7-day retention, per-source quotas)
- DF-1: Selective thread backfill (bounded history window + context frame, not full replication)
- DF-3: Federated presence with agent-specific states (thread-scoped only, best-effort relay)
- DF-5: Server allowlist/denylist (enterprise access control, enforced at relay endpoint)
- DF-7: Federated tool call visibility (should flow naturally if relay handles all message types)

**Defer to v3.3+:**
- DF-2: Agent capability federation — needs broader agent ecosystem before design is clear
- DF-4: Federated streaming responses — HTTP batch relay vs WebSocket S2S is unsettled; needs dedicated research
- DF-6: Cross-server memory queries — complex authorization model, defer until federation is stable

**Explicit anti-features (never build):**
- DAG-based eventual consistency — produces Matrix-level complexity (2000-line state resolution), eliminated by origin-server authority
- Persistent S2S connections (XMPP model) — O(n²) connection lifecycle complexity
- Global presence federation — causes 50,000+ HTTP requests/hour per user in Matrix at scale; thread-scoped only
- Multi-hop relay (transitive federation) — creates open relay vulnerability, simplify to direct S2S
- Full message history replication — unbounded data transfer; use selective backfill instead

### Architecture Approach

The existing single-server Axum architecture requires surgical extension, not redesign. The WebSocket handler, Ed25519 signing, and JSON-RPC framing are preserved. The current `broadcast::Sender` routing (hard-coded for exactly one human + one agent) must be replaced with a participant-aware `Message Router` backed by a `Session Manager` (DashMap of identity → WebSocket sender). Four new modules are required: `federation.rs` (inbound relay handler + outbound Federation Sender), `relay_queue.rs` (redb-backed persistent queue for both offline local participants and unreachable remote servers), `tls.rs` (mTLS `ServerConfig`/`ClientConfig` construction), and `cert_gen.rs` (demo-only certificate generation via rcgen). Two new Axum routes are added to the TLS-wrapped federation listener: `POST /ainm/v1/relay` and `GET /.well-known/ainm/server`.

**Major components:**
1. **Message Router** (new, replaces broadcast channels) — routes per-participant: local+online → Session Manager, local+offline → Offline Queue, remote → Federation Sender, grouped by destination server
2. **Federation Sender** (new) — per-destination outbound queues modeled on Synapse's `per_destination_queue.py`; exponential backoff (1s base, max 60s), batches up to 50 messages per relay request, signs with server Ed25519 key, sends via mTLS reqwest client
3. **Federation Relay Handler** (new) — inbound `POST /ainm/v1/relay`; validates three-layer security (mTLS fingerprint → server signature → per-message signatures), passes to Message Router
4. **Thread Registry** (new, replaces `Vec<String>`) — stores `FederatedThread` with `origin_server` and per-participant `home_server` fields; these fields drive all routing decisions
5. **Identity Resolver** (extended from IdentityStore) — local `DashMap` lookup first, then HTTPS fetch to remote `.well-known/ainm/{name}` with 1-hour TTL cache; two distinct key types: server Ed25519 keys (sign relay metadata) vs participant Ed25519 keys (sign message content)
6. **Offline Queue** (new) — redb-backed store-and-forward; per-participant queues for local offline users; drained in chronological order on reconnect; key schema: `(target_server_domain, message_timestamp_nanos) → JSON bytes`

**Two-listener architecture for demo:**
```
Plain TCP listener (existing, unchanged):
  WebSocket /ws  ←→  Local client connections

TLS TCP listener (new, mTLS for S2S):
  POST /ainm/v1/relay       ← Inbound relay from remote servers
  GET  /.well-known/ainm/server  ← Server identity endpoint
```

### Critical Pitfalls

1. **Split-brain thread state under network partitions** — origin-server authority works when the origin is available; under partition, concurrent activity on remote servers creates divergent histories. Fix: define explicit partition behavior in the spec before writing code (can remote servers accept messages when origin is down?); build a partition test harness in Phase 1 that partitions two servers, generates concurrent activity, heals the partition, and verifies message ordering. Do not ship federation without passing partition tests.

2. **mTLS certificate lifecycle breaks federation silently** — Let's Encrypt certs expire every 90 days; certificate rotation changes the TLS fingerprint; remote servers cache the old fingerprint for up to 1 hour after rotation, causing federation to fail with no local symptoms. Fix: extend `tls_fingerprint` to a `tls_fingerprints` array with `valid_from`/`valid_until` per entry to support rotation overlap; implement a `ainm-fed-check` CLI health tool; monitor expiry proactively. Must be designed in Phase 2 before the first production mTLS connection.

3. **Server identity spoofing via DNS poisoning** — well-known discovery trusts DNS + TLS; a DNS attack can serve an attacker's public key and intercept all relay messages. Fix: implement TOFU key pinning (once a server's Ed25519 key is first seen, pin it; require old key to sign rotation attestation for any key change). Implement in Phase 2 alongside mTLS, not as a later addition.

4. **Signature verification bottleneck at relay scale** — a relay batch with 50 messages from 30 different remote senders requires potentially 30 identity resolution HTTP calls on the hot path (200ms each = 6s of blocking I/O before any message is processed). Fix: aggressive identity caching (1-hour TTL covering 99%+ of lookups), accept relay with 202 and verify signatures asynchronously, cap batch size at 100 messages. Cache in Phase 1; async verification and batch resolution endpoint in Phase 3.

5. **Cross-federation replay attacks** — Ed25519 signatures are permanent; a captured relay payload can be replayed after the 24-hour deduplication window expires. Fix: add `relay_timestamp` to relay params (covered by server signature), reject payloads older than 5 minutes; add `relay_id` unique per relay request (distinct from message IDs). Must be in Phase 2 before production.

6. **Backfill leaks pre-join message history** — naive backfill sends full thread history to any requesting server, including messages from before the new participant joined. Fix: implement backfill visibility levels (`none`, `summary`, `since_invite`, `full`) at thread creation time; enforce authorization (only serve messages from the requesting participant's join timestamp onward; only serve to servers with a legitimate participant in the thread). Design before Phase 2 backfill implementation, not as a retrofit.

## Implications for Roadmap

Federation naturally decomposes into three phases following a strict dependency chain: establish the relay core (prove federation works), harden authentication and security (make it production-safe), then add reliability and AI-native differentiation features.

### Phase 1: Relay Core

**Rationale:** TS-2 (server discovery) is the root dependency for all other federation features. TS-5 (origin-server authority) must be locked in architecturally before any thread state is created across servers. TS-1 (cross-server message delivery) is the milestone that proves federation works. These three together produce a working two-server Docker Compose demo.

**Delivers:** Two AINM servers can exchange messages between users on different servers. Server discovery, identity resolution, mTLS handshake, three-layer signature validation, and basic thread creation/join all work end-to-end. Partition test harness validates behavior under network failure.

**Addresses features:** TS-2, TS-5, TS-8, TS-1, TS-10, TS-4, TS-6, TS-7, TS-9 (the full P1 feature set)

**New stack added:** reqwest 0.13, rcgen 0.14, rustls 0.23, tokio-rustls 0.26, rustls-pki-types 1.14

**New modules:** `federation.rs`, `tls.rs`, `cert_gen.rs`, `server_identity.rs`; Message Router, Session Manager, Thread Registry, Identity Resolver (extended)

**Must avoid:** Skipping the partition test harness (Pitfall 1 — define partition behavior in spec before implementing relay). Routing to individual recipients instead of grouping by destination server (performance trap). Accepting relay messages where `from` address domain does not match `from_server` (server message laundering security mistake).

### Phase 2: Authentication and Security Hardening

**Rationale:** Relay core proves federation works. Before adding any reliability features, complete the security model: replay protection, TOFU key pinning, certificate rotation protocol, and backfill authorization. These are cheaper to design now than to retrofit onto a production system.

**Delivers:** mTLS with `tls_fingerprints` array supporting rotation overlap; TOFU key pinning store that rejects unsigned key changes; `relay_timestamp` + `relay_id` replay protection; backfill endpoint with visibility levels and authorization enforcement; per-source federation rate limiting.

**Addresses features:** TS-9 (rate limiting, if not already in Phase 1), DF-1 (selective backfill with authorization), DF-5 (server allowlist/denylist)

**New modules:** Key pinning store, certificate rotation logic, backfill handler

**Must avoid:** Every item in the "technical debt shortcuts" table from PITFALLS.md: skipping mTLS, indefinite server identity caching, skipping `server_signature` verification, trusting all federated servers equally. Implementing backfill without authorization first (Pitfall 6).

### Phase 3: Reliability and AI-Native Differentiation

**Rationale:** Core federation is secure. Add features that make AINM's federation more reliable and more interesting than alternatives: persistent offline relay queues (messages survive server outages), federated presence (participants on remote servers see agent-specific states), and performance hardening for signature verification at scale.

**Delivers:** redb-backed offline relay queue (7-day retention, per-source quotas, size-bounded); federated presence relay (thread-scoped only, best-effort, no queue); federated tool call visibility events; async signature verification pool; batch identity resolution endpoint `POST /.well-known/ainm/resolve-batch`.

**Addresses features:** TS-3 (offline relay queues), DF-3 (federated presence), DF-7 (federated tool call visibility)

**New stack added:** redb 3.x

**New modules:** `relay_queue.rs`

**Must avoid:** Global presence federation (Anti-feature AF-3 — thread-scoped only); unbounded offline queues (add per-source quotas and size limits before testing); synchronous identity resolution on relay hot path (Pitfall 4 — address via async verification and batch resolution endpoint).

### Phase Ordering Rationale

- **Dependency chain is strict:** Server discovery (TS-2) → Identity resolution (TS-8) → Message delivery (TS-1) → all reliability features. This ordering matches the feature dependency graph in FEATURES.md exactly.
- **Security before reliability:** Offline queues without replay protection would queue attackable payloads. Backfill without authorization leaks historical data. Phase 2 must complete before Phase 3 ships anything production-facing.
- **Architecture decisions are irreversible after traffic accumulates:** Origin-server authority (Phase 1) and backfill visibility levels (Phase 2) cannot change without breaking federation compatibility. Lock these in first, implement them explicitly, test them aggressively.
- **Selective backfill (DF-1) belongs in Phase 2:** Although it might seem like a "nice to have," backfill without the authorization model (Pitfall 6) is a data leakage risk. Design it in Phase 2 so it ships with authorization from the start.
- **Streaming federation (DF-4) stays deferred:** The design choice between HTTP batch relay (pragmatic, higher latency) and WebSocket S2S (lower latency, more complex) is unsettled. Do not speculate — research it when it becomes the bottleneck.

### Research Flags

Phases likely needing deeper research during planning:

- **Phase 1 (partition behavior):** The spec says "origin-server authority" but does not fully define what happens when the origin is partitioned for an extended period. The exact ordering and merge strategy for queued messages on reconnect is a protocol-level decision that must be made before relay implementation begins. This is a gap in the current spec.
- **Phase 2 (certificate rotation protocol):** The `tls_fingerprints` array with `valid_from`/`valid_until` is not yet in the AINM spec. The exact format and behavior during the rotation overlap window must be spec'd before Phase 2 implementation.
- **Phase 2 (backfill visibility spec):** The four backfill levels (`none`, `summary`, `since_invite`, `full`) are recommended in FEATURES.md but not yet in the spec. The format of backfill requests and responses including visibility enforcement needs to be designed and added to transport.md.
- **Phase 3 (federated streaming, if re-prioritized):** DF-4 is deferred to v3.3+ but if it gets pulled forward, the HTTP batch relay vs WebSocket S2S design question is unsettled and needs dedicated research before implementation begins.

Phases with standard patterns (skip research-phase):

- **Phase 1 (relay sender):** Per-destination queues with exponential backoff are well-documented. Synapse's `per_destination_queue.py` is the direct reference. The ARCHITECTURE.md already contains the Rust struct design (`FederationSender`, `DestinationQueue`, `DestinationState`).
- **Phase 1 (mTLS setup):** The axum TLS example + tokio-rustls + rcgen pattern is documented in the official axum repository. STACK.md contains the exact Cargo.toml additions verified on 2026-03-28.
- **Phase 1 (Docker Compose orchestration):** Standard Docker Compose patterns with init container for cert bootstrapping; no new research needed.
- **Phase 3 (redb queue):** redb's API is straightforward; the key schema `(target_server_domain, message_timestamp_nanos) → JSON bytes` is already designed in STACK.md.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All 6 new crate versions verified against crates.io on 2026-03-28. Alternatives explicitly analyzed and rejected. RUSTSEC-2025-0134 (rustls-pemfile deprecation) caught and accounted for. redb rated MEDIUM during research but upgraded to HIGH — stable 1.0 since 2023, now at 3.1.1. |
| Features | HIGH | Grounded in Matrix Server-Server API v1.16, XMPP XEP-0045/0289, ActivityPub spec, and the existing AINM spec files (transport.md, identity.md, security.md). Feature dependency graph is fully mapped with explicit conflict identification (TS-5 conflicts with AF-1). |
| Architecture | HIGH | Based on existing codebase analysis (`demo/src/server.rs`) plus Matrix/Conduit/Synapse federation patterns. Component boundaries, data flows, and Rust struct signatures are concrete and implementable. Two-listener architecture is the right call for the demo. |
| Pitfalls | HIGH | Every pitfall is grounded in documented real-world incidents: Matrix state resolution CVEs (Project Hydra 2025), XMPP dialback domain spoofing (2012), ActivityPub HTTP signature fragmentation, email open relay history. Not speculative — these failures have happened in production federated systems. |

**Overall confidence:** HIGH

### Gaps to Address

- **Partition behavior specification:** The spec asserts "origin-server authority" but does not define what happens during extended origin unavailability. Options (read-only, accept with uncertain ordering, promote backup origin) each have different protocol implications. This must be resolved in Phase 1 planning before relay implementation begins.

- **`tls_fingerprints` array format:** The current AINM spec has a single `tls_fingerprint` string field. The research recommends extending this to an array with `valid_from`/`valid_until` per entry to support certificate rotation overlap. This spec change is required before Phase 2 implementation.

- **Backfill visibility level spec:** The four backfill levels (`none`, `summary`, `since_invite`, `full`) are recommended by research but not yet in transport.md. Spec addition required before Phase 2 backfill implementation.

- **Docker Compose demo acceptance criteria:** The demo must test more than the happy path. The PITFALLS.md "Looks Done But Isn't" checklist identifies partition simulation, certificate rotation, key rotation, and replay attack tests as required for the demo to be credible. Phase 1 acceptance criteria should explicitly include these test scenarios.

- **Batch identity resolution endpoint:** The research recommends `POST /.well-known/ainm/resolve-batch` for batching identity lookups during relay processing (Pitfall 4 mitigation). This endpoint is not in the current spec. Plan for Phase 3 addition when signature verification performance becomes a measured bottleneck.

## Sources

### Primary (HIGH confidence — official specs and crates.io verified)

- [reqwest on crates.io v0.13.2](https://crates.io/crates/reqwest) — mTLS HTTP client, rustls-tls feature
- [rcgen on GitHub (rustls org) v0.14.7](https://github.com/rustls/rcgen) — X.509 certificate generation
- [rustls on crates.io v0.23.36](https://crates.io/crates/rustls) — WebPkiClientVerifier for mTLS
- [tokio-rustls on crates.io v0.26.4](https://crates.io/crates/tokio-rustls) — async TLS acceptor
- [rustls-pki-types on crates.io v1.14.0](https://crates.io/crates/rustls-pki-types) — PEM parsing (replaces rustls-pemfile)
- [RUSTSEC-2025-0134: rustls-pemfile unmaintained](https://rustsec.org/advisories/RUSTSEC-2025-0134) — critical: do not use rustls-pemfile
- [redb on crates.io v3.1.1](https://crates.io/crates/redb) — pure-Rust embedded KV store
- [Matrix Server-Server API v1.16](https://spec.matrix.org/v1.16/server-server-api/) — federation endpoints, transactions, backfill patterns
- [Matrix State Resolution v2](https://matrix.org/docs/older/stateres-v2/) — why DAG complexity is real and must be avoided
- [W3C ActivityPub Specification](https://www.w3.org/TR/activitypub/) — inbox/outbox delivery model, async retry
- [axum TLS example (official repo)](https://github.com/tokio-rs/axum/blob/main/examples/tls-rustls/src/main.rs) — tokio-rustls integration pattern
- AINM Transport Specification v0.1.0 (`spec/transport.md`) — existing federation relay spec
- AINM Identity Specification v0.1.0 (`spec/identity.md`) — identity resolution and key management
- AINM Security Specification v0.1.0 (`spec/security.md`) — message signing, deduplication

### Secondary (MEDIUM confidence — community sources, multiple sources agree)

- [Matrix State Resolution CVE-2025-49090 (Project Hydra)](https://matrix.org/blog/2025/08/project-hydra-improving-state-res/) — state resets from crafted events after 7 years of v2; validates choosing origin-server authority
- [Auth Chain Difference Performance (Synapse #8622)](https://github.com/matrix-org/synapse/issues/8622) — hundreds of seconds of DB time; validates async verification requirement
- [Typing Notifications with 800 Homeservers](https://matrix.org/blog/2025/01/27/this-week-in-matrix-2025-01-27/) — EDU federation scaling failure; validates thread-scoped presence decision
- [Conduit (Rust Matrix homeserver)](https://conduit.rs/) — validates Rust for federated messaging at 32 MiB RAM vs Synapse's hundreds
- [XMPP Server Dialback Vulnerability](https://xmpp.org/community/security-notices/vulnerability-in-xmpp-server-dialback-implementations/) — DNS spoofing in practice; validates TOFU key pinning requirement
- [ActivityPub and HTTP Signatures fragmentation](https://swicg.github.io/activitypub-http-signature/) — implementation inconsistencies between Mastodon/Misskey/GoToSocial; validates single-implementation strategy
- [Step v0.8.3: Federation Root Rotation](https://smallstep.com/blog/step-v0.8.3-federation-root-rotation/) — certificate rotation in federated PKI; validates `tls_fingerprints` array approach
- [mTLS Complete Guide (GitGuardian)](https://blog.gitguardian.com/mutual-tls-mtls-authentication/) — certificate lifecycle operational complexity

### Tertiary (informational)

- [XEP-0045: Multi-User Chat](https://xmpp.org/extensions/xep-0045.html) — XMPP MUC as reference for origin-server authority model
- [XEP-0289: Federated MUC](https://xmpp.org/extensions/xep-0289.html) — distributed multi-user chat federation patterns
- [Wire: Federating Security or Spam](https://wire.com/en/blog/federating-security-or-spam) — open vs closed federation trust model analysis
- [Open Mail Relay History](https://en.wikipedia.org/wiki/Open_mail_relay) — validates direct-only S2S relay (no transitive hops)
- [Matrix Decomposition Academic Analysis](https://matrix.org/blog/2020/06/16/matrix-decomposition-an-independent-academic-analysis-of-matrix-state-resolution/) — formal correctness analysis of state resolution

---
*Research completed: 2026-03-28*
*Ready for roadmap: yes*
