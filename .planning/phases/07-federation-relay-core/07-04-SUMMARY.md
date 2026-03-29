---
phase: 07-federation-relay-core
plan: 04
subsystem: federation-integration
tags: [federation, demo, docker, mTLS, routing]
dependency_graph:
  requires: [07-01, 07-02, 07-03]
  provides: [federation-demo, two-listener-architecture, federation-routing]
  affects: [demo/src/main.rs, demo/src/server.rs, demo/src/identity.rs]
tech_stack:
  added: [docker, docker-compose]
  patterns: [two-listener-architecture, federation-routing, participant-key-exchange]
key_files:
  created: [demo/Dockerfile, demo/docker-compose.yml]
  modified: [demo/src/main.rs, demo/src/server.rs, demo/src/identity.rs]
decisions:
  - Used HTTP participant key exchange instead of deterministic keypairs since generate_keypair uses SystemRandom
  - Added /.well-known/ainm/participant/:address endpoint for cross-server key exchange
  - Headless agent mode subscribes to broadcast channel rather than connecting via WebSocket
metrics:
  duration: 8m
  completed: 2026-03-29
---

# Phase 07 Plan 04: Federation Integration & Docker Demo Summary

Two-listener federation architecture with mTLS cert fingerprint verification, federation-aware message routing, and Docker Compose two-server demo.

## What Was Built

### Task 1: Federation Startup & Message Routing
- **run_federation()** in main.rs: Complete federation mode startup with two TCP listeners
  - Listener 1: Plain TCP for local WebSocket (REPL/agent connections)
  - Listener 2: mTLS TCP for federation relay with FAUT-02 layer 1 cert fingerprint verification
- **FederationContext** in server.rs: Optional federation state for routing decisions
- **Federation-aware routing**: Messages check thread origin domain (D-10/D-11) and relay to remote servers or deliver locally
- **Participant key exchange**: New `GET /.well-known/ainm/participant/:address` endpoint for cross-server Ed25519 public key exchange (since generate_keypair is non-deterministic)
- **find_by_fingerprint()** on RemoteServerCache for TLS cert verification
- **Headless agent mode**: Subscribes to local_delivery_tx broadcast, processes relayed messages, echoes responses back via relay

### Task 2: Docker Compose Demo
- **Dockerfile**: Multi-stage Rust build (rust:1.85-bookworm builder, debian:bookworm-slim runtime)
- **docker-compose.yml**: Two-server orchestration
  - server-a: human mode (ports 8080/8443) with REPL
  - server-b: agent mode (ports 9080/9443) headless
  - Federation bridge network with .local DNS aliases
  - Certificate volumes mounted read-only

### Task 3: Verification (Auto-approved)
Build succeeds with `cargo build`. Docker compose config is valid YAML.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Non-deterministic keypair generation breaks identity pre-seeding**
- **Found during:** Task 1
- **Issue:** Plan assumed `generate_keypair()` is deterministic, but it uses `SystemRandom`. Same address generates different keys on different servers.
- **Fix:** Added HTTP-based participant key exchange via `GET /.well-known/ainm/participant/:address`. Each server publishes its local participants' public keys, and remote servers fetch them during startup.
- **Files modified:** demo/src/main.rs (fetch_participant_key, participant_key_handler)
- **Commit:** dbc3c7b

**2. [Rule 3 - Blocking] Rust ownership conflict in CLI match arm**
- **Found during:** Task 1
- **Issue:** `cli.mode.as_deref()` borrows `cli`, but `run_federation(cli, ...)` moves it.
- **Fix:** Clone `cli.mode` before the match.
- **Files modified:** demo/src/main.rs
- **Commit:** dbc3c7b

**3. [Rule 3 - Blocking] hyper service trait mismatch**
- **Found during:** Task 1
- **Issue:** `tower::ServiceExt::into_make_service` returns a tower Service, but hyper expects its own Service trait.
- **Fix:** Used `hyper_util::service::TowerToHyperService` adapter.
- **Files modified:** demo/src/main.rs
- **Commit:** dbc3c7b

## Decisions Made

1. **Participant key exchange over HTTP** -- Since Ed25519 keypairs are randomly generated, servers exchange participant public keys via a new `.well-known` endpoint at startup. This is more correct than the plan's deterministic-key assumption.
2. **Headless agent via broadcast channel** -- Agent mode subscribes to `local_delivery_tx` broadcast rather than connecting via WebSocket, avoiding an extra connection hop for relayed messages.
3. **TowerToHyperService adapter** -- Required for bridging axum's tower Service to hyper's Service trait in the manual TLS accept loop.

## Requirements Coverage

| Requirement | Status | Evidence |
|-------------|--------|----------|
| RELY-01 | Covered | Server identity at /.well-known/ainm/server |
| RELY-04 | Covered | RelaySender with push-based queues |
| FAUT-01 | Covered | mTLS on federation listener |
| FAUT-02 | Covered | TLS cert fingerprint verification in accept loop |
| DEMO-01 | Covered | Docker compose with two servers |
| DEMO-02 | Covered | Participant key exchange for identity pre-seeding |

## Known Stubs

None -- all federation components are wired end-to-end.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | dbc3c7b | Federation startup, routing, identity exchange |
| 2 | 60f6a44 | Dockerfile and docker-compose.yml |

## Self-Check: PASSED

All files exist. All commits verified.
