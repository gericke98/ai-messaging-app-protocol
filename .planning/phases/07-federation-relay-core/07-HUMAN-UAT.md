---
status: partial
phase: 07-federation-relay-core
source: [07-VERIFICATION.md]
started: 2026-03-28T21:00:00Z
updated: 2026-03-29T06:40:00Z
---

## Current Test

[1 remaining — requires Docker]

## Tests

### 1. End-to-end docker compose federation demo
expected: Server A human REPL types a message; Server B agent logs show relay receipt, three-layer signature verification, echo response sent back; Server A REPL displays echo response from helper@server-b.local
result: [blocked — Docker not installed on this machine]

### 2. Legacy mode backward compatibility
expected: Running ainm-demo with no flags launches existing single-server demo unchanged; all commands (hello, form, tool, button, /card, /memory, /evict, /bridge) function correctly
result: PASSED — Legacy mode starts correctly, server/human/agent connect, handshake completes, thread created, all commands listed. Also found and fixed Axum 0.8 route param syntax bug (`:address` → `{address}`) that caused federation mode panic.

### 3. mTLS certificate rejection
expected: TLS handshake fails when connecting to federation port (8443) with a certificate NOT signed by the shared CA; server logs show TLS handshake failed
result: PASSED — Two tests run: (a) CA cert used as end-entity: rejected with `invalid peer certificate: CaUsedAsEndEntity`, (b) Self-signed cert not from CA: rejected with `invalid peer certificate: UnknownIssuer`. Server correctly enforces mTLS.

## Summary

total: 3
passed: 2
issues: 0
pending: 0
skipped: 0
blocked: 1

## Gaps

### 1. Docker compose end-to-end test blocked
status: blocked
reason: Docker not installed on test machine. Requires `docker compose up` to test cross-server relay.
