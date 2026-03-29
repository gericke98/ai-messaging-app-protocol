# Requirements: AI-Native Messaging Protocol — Federation

**Defined:** 2026-03-29
**Core Value:** Enable mixed human-agent threads where any participant (human or agent) can seamlessly join, communicate, and coordinate — with the same quality of experience regardless of who's a person and who's an AI.

## v1 Requirements

Requirements for full multi-server federation with working demo.

### Relay Core

- [ ] **RELY-01**: Server can discover remote servers via .well-known endpoint resolution of the @provider domain
- [ ] **RELY-02**: Server can relay messages to remote servers via HTTP POST with JSON-RPC 2.0 framing
- [ ] **RELY-03**: Origin server is authoritative for thread state (participant list, ordering, metadata)
- [ ] **RELY-04**: Server uses push-based federation model (no polling) — messages are forwarded to remote servers immediately
- [ ] **RELY-05**: Server maintains per-destination send queues with exponential backoff for unreachable servers

### Authentication & Security

- [ ] **FAUT-01**: Servers authenticate via mutual TLS (both sides present certificates)
- [ ] **FAUT-02**: Server identity is verified by matching TLS certificate fingerprint against .well-known published fingerprint
- [ ] **FAUT-03**: Relay messages include server-level signature in addition to sender's Ed25519 message signature (triple-layer validation)
- [ ] **FAUT-04**: Server implements TOFU (Trust On First Use) key pinning for remote server certificates
- [ ] **FAUT-05**: Relay messages include relay_timestamp and relay_id to prevent replay attacks across federation boundaries
- [ ] **FAUT-06**: Server supports multiple TLS fingerprints (tls_fingerprints array) for certificate rotation without federation downtime

### Reliability

- [ ] **FREL-01**: Server persists offline relay queue to disk (survives server restart)
- [ ] **FREL-02**: Offline queue retries with exponential backoff when remote server becomes reachable
- [ ] **FREL-03**: Offline queue has bounded size and 7-day retention policy
- [ ] **FREL-04**: Server defines partition behavior — queues messages during origin-server unavailability and replays on reconnection

### Backfill & History

- [ ] **BKFL-01**: Remote server can request selective thread history via backfill endpoint
- [ ] **BKFL-02**: Backfill respects visibility levels (post-join messages only by default, pre-join with explicit permission)
- [ ] **BKFL-03**: Backfill endpoint requires authorization (requesting server must have a current participant in the thread)
- [ ] **BKFL-04**: Backfill supports pagination for large thread histories

### AI-Native Federation

- [ ] **AINF-01**: Agent Cards are discoverable across federation boundaries (resolve remote agent's @provider to fetch card)
- [ ] **AINF-02**: Tool call visibility events (started/progress/completed) are relayed across federated servers
- [ ] **AINF-03**: Memory context is propagated to agents on remote servers when they join a federated thread
- [ ] **AINF-04**: Streaming responses (start/delta/end) are relayed across federation in real-time

### Federation Demo

- [ ] **DEMO-01**: docker-compose spins up 2+ AINM servers with distinct identities and certificates
- [ ] **DEMO-02**: Demo shows cross-server message exchange between a human on server A and an agent on server B
- [ ] **DEMO-03**: Demo tests partition healing (stop one server, queue messages, restart, verify delivery)
- [ ] **DEMO-04**: Demo tests certificate rotation (rotate cert, verify federation continues)
- [ ] **DEMO-05**: Demo shows cross-server streaming (agent on server B streams response to human on server A)

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Federation Advanced

- **FADV-01**: Thread migration when origin server is permanently lost
- **FADV-02**: Federated moderation and abuse prevention
- **FADV-03**: Federation rate limiting with graduated trust for new servers
- **FADV-04**: Federation admin dashboard and monitoring

### iOS Reference Client

- **IOSC-01**: Native SwiftUI iOS app implementing the protocol
- **IOSC-02**: GRDB.swift local storage for offline message access
- **IOSC-03**: Apple CryptoKit for Ed25519 signing
- **IOSC-04**: APNs integration for background message delivery
- **IOSC-05**: Native rendering of all content block types

## Out of Scope

| Feature | Reason |
|---------|--------|
| Thread migration (origin server permanently lost) | Complex distributed consensus problem — defer to future milestone |
| Federated moderation / abuse prevention | Requires trust reputation system — defer |
| Federation rate limiting / graduated trust | Optimization — basic federation first |
| Browser/web federation client | iOS and CLI clients only for now |
| Federation admin dashboard | Operational tooling deferred |
| End-to-end encryption across federation | E2EE is a separate milestone concern |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| RELY-01 | - | Pending |
| RELY-02 | - | Pending |
| RELY-03 | - | Pending |
| RELY-04 | - | Pending |
| RELY-05 | - | Pending |
| FAUT-01 | - | Pending |
| FAUT-02 | - | Pending |
| FAUT-03 | - | Pending |
| FAUT-04 | - | Pending |
| FAUT-05 | - | Pending |
| FAUT-06 | - | Pending |
| FREL-01 | - | Pending |
| FREL-02 | - | Pending |
| FREL-03 | - | Pending |
| FREL-04 | - | Pending |
| BKFL-01 | - | Pending |
| BKFL-02 | - | Pending |
| BKFL-03 | - | Pending |
| BKFL-04 | - | Pending |
| AINF-01 | - | Pending |
| AINF-02 | - | Pending |
| AINF-03 | - | Pending |
| AINF-04 | - | Pending |
| DEMO-01 | - | Pending |
| DEMO-02 | - | Pending |
| DEMO-03 | - | Pending |
| DEMO-04 | - | Pending |
| DEMO-05 | - | Pending |

**Coverage:**
- v1 requirements: 28 total
- Mapped to phases: 0
- Unmapped: 28

---
*Requirements defined: 2026-03-29*
*Last updated: 2026-03-29 after initial definition*
