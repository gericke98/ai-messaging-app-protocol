# Roadmap: AI-Native Messaging Protocol

## Milestones

- ✅ **v3.1 AI-Native Messaging Protocol v1 Specification & Demo** — Phases 1-6 (shipped 2026-03-29) — [Archive](milestones/v3.1-ROADMAP.md)
- 🚧 **v3.2 Federation** — Phases 7-9 (in progress)

## Phases

- [ ] **Phase 7: Federation Relay Core** - Two servers can discover each other and exchange messages with mTLS authentication and origin-server authority
- [ ] **Phase 8: Security Hardening, Reliability & Backfill** - Federation is production-safe with replay protection, persistent offline queues, selective backfill, and partition healing
- [ ] **Phase 9: AI-Native Federation & Full Demo** - Agent capabilities, tool calls, memory, and streaming work across federation boundaries with comprehensive demo scenarios

## Phase Details

### Phase 7: Federation Relay Core
**Goal**: Any AINM server can discover a remote server, authenticate via mTLS, and relay messages with origin-server authority over thread state
**Depends on**: Phase 6 (v3.1 complete)
**Requirements**: RELY-01, RELY-02, RELY-03, RELY-04, RELY-05, FAUT-01, FAUT-02, FAUT-03, DEMO-01, DEMO-02
**Success Criteria** (what must be TRUE):
  1. Server A resolves server B's identity and public key via .well-known endpoint given only a @provider domain
  2. A message sent by a human on server A arrives at an agent on server B within seconds, with three-layer signature validation (mTLS + server signature + sender Ed25519)
  3. The server where a thread was created is the sole authority for that thread's participant list and message ordering — remote servers defer
  4. Messages are pushed immediately to remote servers (no polling) with per-destination send queues and exponential backoff for unreachable servers
  5. A docker-compose demo spins up 2 servers with distinct certificates and shows a cross-server human-agent conversation end-to-end
**Plans:** 5 plans

Plans:
- [x] 07-01-PLAN.md — Add federation dependencies (Cargo.toml) and CLI argument parsing (clap)
- [x] 07-02-PLAN.md — TLS module, server identity types, and pre-generated mTLS certificates
- [x] 07-03-PLAN.md — Federation relay module (handler, sender, send queues, thread registry)
- [x] 07-04-PLAN.md — Wire federation into server, two-listener architecture, Docker Compose demo
- [x] 07-05-PLAN.md — Gap closure: fix REPL thread_id propagation and relay routing stub

### Phase 8: Security Hardening, Reliability & Backfill
**Goal**: Federation is hardened against replay attacks and certificate lifecycle issues, survives server outages with persistent queues, and supports selective thread history backfill with authorization
**Depends on**: Phase 7
**Requirements**: FAUT-04, FAUT-05, FAUT-06, FREL-01, FREL-02, FREL-03, FREL-04, BKFL-01, BKFL-02, BKFL-03, BKFL-04, DEMO-03, DEMO-04
**Success Criteria** (what must be TRUE):
  1. A replayed relay payload older than 5 minutes is rejected, and a captured relay cannot be replayed after the deduplication window expires (relay_timestamp + relay_id enforcement)
  2. When a server's TLS certificate is rotated, federation continues without downtime because the tls_fingerprints array supports rotation overlap
  3. When a remote server goes offline, messages are persisted to disk and automatically delivered with exponential backoff when it comes back — surviving server restarts
  4. A remote server can request thread history via backfill, receiving only post-join messages by default, with pagination for large threads and authorization that rejects servers without a legitimate participant
  5. Demo scenarios prove partition healing (stop server, queue, restart, verify delivery) and certificate rotation (rotate cert, verify federation continues)
**Plans:** 5 plans

Plans:
- [x] 08-01-PLAN.md — Foundation: spec updates, redb persistence module, ServerIdentity fingerprint array migration
- [x] 08-02-PLAN.md — Replay protection (relay_timestamp + relay_id) and TOFU key pinning with cert rotation
- [x] 08-03-PLAN.md — Persistent offline queue replacing in-memory VecDeque, background cleanup
- [x] 08-04-PLAN.md — Backfill endpoint with authorization, post-join visibility, cursor pagination
- [x] 08-05-PLAN.md — Demo scripts for partition healing (DEMO-03) and cert rotation (DEMO-04)

### Phase 9: AI-Native Federation & Full Demo
**Goal**: AINM's unique AI-native features — agent discovery, tool call transparency, memory propagation, and streaming — work seamlessly across federation boundaries
**Depends on**: Phase 8
**Requirements**: AINF-01, AINF-02, AINF-03, AINF-04, DEMO-05
**Success Criteria** (what must be TRUE):
  1. An agent's capabilities (Agent Card) can be discovered by resolving its @provider domain across federation — a user on server A can see what an agent on server B can do
  2. When an agent on server B executes a tool call, the started/progress/completed events are visible to all thread participants on server A in real-time
  3. When an agent joins a federated thread, it receives accumulated memory context (active context frames, relevant conversation history) from the origin server
  4. An agent on server B streams a response token-by-token and a human on server A sees it render in real-time across the federation boundary — proven in the docker-compose demo
**Plans**: 4 plans
Plans:
- [x] 09-00-PLAN.md — Protocol spec for ainm.context-push (transport.md, JSON Schema, AsyncAPI)
- [ ] 09-01-PLAN.md — Agent card discovery, streaming/tool event relay, /discover command
- [ ] 09-02-PLAN.md — Memory context propagation via ainm.context-push
- [ ] 09-03-PLAN.md — Full-featured headless agent and comprehensive demo script

## Progress

**Execution Order:**
Phases execute in numeric order: 7 → 8 → 9

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 7. Federation Relay Core | v3.2 | 4/5 | In Progress | - |
| 8. Security Hardening, Reliability & Backfill | v3.2 | 1/5 | In Progress | - |
| 9. AI-Native Federation & Full Demo | v3.2 | 0/TBD | Not started | - |
