---
phase: 09-ai-native-federation-full-demo
plan: 03
subsystem: demo
tags: [federation, streaming, tool-calls, memory, demo, headless-agent]
dependency_graph:
  requires: [09-01, 09-02]
  provides: [full-featured-headless-agent, ai-native-federation-demo-script]
  affects: [demo/src/main.rs, demo/scripts/]
tech_stack:
  added: []
  patterns: [keyword-dispatch, relay-based-streaming, tool-call-lifecycle]
key_files:
  created:
    - demo/scripts/demo-ai-native-federation.sh
  modified:
    - demo/src/main.rs
decisions:
  - "Streaming events serialized via serde_json::to_value for relay compatibility (JsonRpcNotification -> Value)"
  - "Tool call events signed individually for cross-server verification"
  - "Demo script uses manual two-terminal approach (consistent with existing scripts)"
metrics:
  duration: ~4min
  completed_date: "2026-03-29 (checkpoint pending)"
  tasks_completed: 2/3
  status: checkpoint-pending
---

# Phase 09 Plan 03: Full-Featured Headless Agent & AI-Native Federation Demo Summary

Keyword-dispatching headless agent with streaming, tool call, and memory responses plus comprehensive demo script for all four AI-native federation features.

## Status: CHECKPOINT PENDING

Tasks 1-2 complete. Task 3 (human-verify checkpoint) awaits manual verification of the end-to-end demo.

## Changes

### Task 1: Full-featured headless agent with streaming, tool, and memory dispatch

Added keyword dispatch to the headless agent in `demo/src/main.rs`:

- **"stream"/"hello"** triggers `build_stream_events()` -- StreamStart, Deltas (3-word chunks), StreamEnd with collapsed signed message. All events relayed via relay_sender for federated threads.
- **"weather"/"tool"** triggers `build_tool_call_events()` -- ToolCallStarted, ToolCallProgress, ToolCallCompleted, followed by a result message. Events signed individually and relayed.
- **"memory"/"context"** triggers a memory acknowledgment response with context frame information.
- **Default** preserves existing echo behavior.

Two helper functions added:
- `build_stream_events(identity, thread_id, text) -> Vec<Value>` -- creates JSON-RPC streaming notifications
- `build_tool_call_events(identity, thread_id) -> Vec<(Value, u64)>` -- creates signed tool lifecycle events with delays

### Task 2: Comprehensive AI-native federation demo script

Created `demo/scripts/demo-ai-native-federation.sh`:

- Five-step demo scenario covering AINF-01 through AINF-04 and DEMO-05
- Two-terminal launch instructions with correct CLI flags
- Expected output descriptions for each step
- Success criteria checklist

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] JsonRpcNotification serialization**
- **Found during:** Task 1
- **Issue:** Plan used `protocol::new_notification()` as if it returns `serde_json::Value`, but it returns `JsonRpcNotification` struct
- **Fix:** Used `serde_json::to_value(&notification).unwrap()` to convert
- **Files modified:** demo/src/main.rs

**2. [Rule 3 - Blocking] CLI flag names in demo script**
- **Found during:** Task 2
- **Issue:** Plan template used `--server-id` flag but actual CLI uses `--identity`
- **Fix:** Used correct `--identity` flag matching docker-compose.yml and Cli struct
- **Files modified:** demo/scripts/demo-ai-native-federation.sh

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | aa96975 | feat(09-03): full-featured headless agent with streaming, tool, and memory dispatch |
| 2 | dd263b0 | feat(09-03): comprehensive AI-native federation demo script |
| 3 | - | PENDING: checkpoint:human-verify |

## Known Stubs

None -- all keyword dispatch paths produce real signed events with correct protocol structure.

## Self-Check: PASSED

- FOUND: demo/src/main.rs
- FOUND: demo/scripts/demo-ai-native-federation.sh
- FOUND: commit aa96975
- FOUND: commit dd263b0
