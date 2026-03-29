---
status: partial
phase: 07-federation-relay-core
source: [07-VERIFICATION.md]
started: 2026-03-28T21:00:00Z
updated: 2026-03-28T21:00:00Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. End-to-end docker compose federation demo
expected: Server A human REPL types a message; Server B agent logs show relay receipt, three-layer signature verification, echo response sent back; Server A REPL displays echo response from helper@server-b.local
result: [pending]

### 2. Legacy mode backward compatibility
expected: Running ainm-demo with no flags launches existing single-server demo unchanged; all commands (hello, form, tool, button, /card, /memory, /evict, /bridge) function correctly
result: [pending]

### 3. mTLS certificate rejection
expected: TLS handshake fails when connecting to federation port (8443) with a certificate NOT signed by the shared CA; server logs show TLS handshake failed
result: [pending]

## Summary

total: 3
passed: 0
issues: 0
pending: 3
skipped: 0
blocked: 0

## Gaps
