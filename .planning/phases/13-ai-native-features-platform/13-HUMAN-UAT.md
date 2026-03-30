---
status: partial
phase: 13-ai-native-features-platform
source: [13-VERIFICATION.md]
started: 2026-03-30T16:05:00Z
updated: 2026-03-30T16:05:00Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. Tool call indicator visual states
expected: When agent executes tool calls, ToolCallIndicatorViews appear below message list, collapsed by default; tap expands to show input JSON, progress, result, or error; spinner for running, green checkmark for completed, red X for error, clock for timed out
result: [pending]

### 2. @-mention autocomplete UX
expected: Typing @ in composer shows filtered participant popup; selecting a participant inserts @DisplayName; backspacing past @ dismisses popup; rendered messages show mention spans in accent blue bold (agent bubbles) or white bold (human bubbles)
result: [pending]

### 3. Agent discovery full flow
expected: Discover Agents view shows cached agent cards with name, description, skill tags; search filters list; tapping agent shows full profile with Skills, Tools, capability badges; Start Conversation creates thread; Add Agent resolves AINM address
result: [pending]

### 4. Memory context sheet
expected: Context pill (brain icon) appears in thread toolbar when agent participant exists; tapping shows half-sheet with Summary, Active Context, Archival Facts preview, Participants; View All navigates to full archival view; search and swipe-to-delete work
result: [pending]

### 5. Sub-thread branch and navigate
expected: Long-pressing a message shows Branch Thread option; branching creates a new sub-thread and navigates to it; sub-thread shows banner 'Thread from {sender}' with back navigation; parent message shows reply count badge; Branch Thread hidden inside sub-threads
result: [pending]

### 6. ainm:// deep link via xcrun simctl
expected: Opening ainm://thread/{id} navigates to the thread; ainm://thread/{id}/message/{mid} also scrolls to the message; nonexistent thread shows 'Thread Not Found' alert; nonexistent message shows 'Message not found' toast
result: [pending]

## Summary

total: 6
passed: 0
issues: 0
pending: 6
skipped: 0
blocked: 0

## Gaps
