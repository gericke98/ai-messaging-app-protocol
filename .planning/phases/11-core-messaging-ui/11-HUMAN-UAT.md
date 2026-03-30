---
status: partial
phase: 11-core-messaging-ui
source: [11-VERIFICATION.md]
started: 2026-03-29T00:00:00Z
updated: 2026-03-29T00:00:00Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. Offline Read Access
expected: Conversation list and message thread load from GRDB without network errors; composer shows "Offline" disabled state.
result: [pending]

### 2. Real-Time Conversation List Updates
expected: Conversation list row updates instantly without manual refresh — new preview text, timestamp, and unread dot appear.
result: [pending]

### 3. Agent Identity Badge Display
expected: Agent messages show cpu.fill orange avatar, sender name, and "Agent" badge label; human messages show person.fill blue avatar.
result: [pending]

### 4. ContentBlock Multi-Block Rendering
expected: All three blocks render sequentially within the same message bubble/container; code shows syntax highlighting; image shows with tap-to-fullscreen.
result: [pending]

## Summary

total: 4
passed: 0
issues: 0
pending: 4
skipped: 0
blocked: 0

## Gaps
