---
status: partial
phase: 03-specification-publication-working-demo
source: [03-VERIFICATION.md]
started: 2026-03-28T22:00:00Z
updated: 2026-03-28T22:00:00Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. Run the CLI demo end-to-end
expected: cargo run --bin ainm-demo starts server, agent connects, REPL opens; typing 'hello' produces streaming token-by-token output with [VERIFIED] badge; typing 'form', 'approve', 'tool', 'button' each shows corresponding rich content
result: [pending]

### 2. Browse the Docusaurus site
expected: npm run build in website/ completes without errors; site has Introduction page, Specification category with 11 items, Demo page, Changelog page, and AsyncAPI explorer at /asyncapi/
result: [pending]

## Summary

total: 2
passed: 0
issues: 0
pending: 2
skipped: 0
blocked: 0

## Gaps
