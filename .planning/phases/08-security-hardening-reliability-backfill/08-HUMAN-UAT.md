---
status: partial
phase: 08-security-hardening-reliability-backfill
source: [08-VERIFICATION.md]
started: 2026-03-29T08:15:00Z
updated: 2026-03-29T08:15:00Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. Partition healing end-to-end
expected: Stop server-b, send messages from server-a (they queue), restart server-b, verify queued messages appear in server-b logs as delivered
result: [pending]

### 2. Certificate rotation end-to-end
expected: Rotate server-b cert, restart with --additional-fingerprint=<old_fp>, federation from server-a continues without error
result: [pending]

## Summary

total: 2
passed: 0
issues: 0
pending: 2
skipped: 0
blocked: 0

## Gaps
