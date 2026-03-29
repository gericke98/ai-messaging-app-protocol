---
phase: 09-ai-native-federation-full-demo
plan: 01
subsystem: demo/federation
tags: [agent-card, streaming, federation, relay, discovery]
dependency_graph:
  requires: [07-03, 07-04, 08-01]
  provides: [dynamic-agent-card-route, streaming-event-relay, discover-command, agent-card-ttl-cache]
  affects: [demo/src/server.rs, demo/src/identity.rs, demo/src/main.rs, demo/src/repl.rs]
tech_stack:
  added: []
  patterns: [stream-to-thread-mapping, ttl-cache-with-expiry, dynamic-axum-routes]
key_files:
  created: []
  modified: [demo/src/identity.rs, demo/src/server.rs, demo/src/main.rs, demo/src/repl.rs]
decisions:
  - Tool call events already relayed via existing ainm.message.send path -- no additional relay code needed for AINF-02
  - Used closure-based handler for federation listener agent card (avoids adding field to FederationState)
  - Stream-to-thread mapping HashMap tracks stream_id -> thread_id for delta/end relay (delta/end lack thread_id)
metrics:
  duration: 3m43s
  completed: 2026-03-29T18:31:52Z
  tasks: 3
  files: 4
---

# Phase 9 Plan 1: Agent Card Discovery, Streaming/Tool Event Relay, /discover Command

Dynamic agent card endpoints on both listeners with TTL-cached resolution, federation-aware streaming/tool event relay, and /discover REPL command for cross-server agent discovery.

## What Was Done

### Task 1: Dynamic agent card route + TTL caching + mTLS listener route
- Added `AgentCardEntry` struct with TTL-based expiry (5-min per D-03) to `RemoteServerCache`
- Added `get_agent_card`/`insert_agent_card` methods with automatic expiry cleanup
- Changed agent card route from hardcoded `/helper/` to dynamic `/{agent_name}/` on both listeners
- Added agent card endpoint to mTLS federation listener with ETag support
- **Commit:** e9da261

### Task 2: Federation-aware streaming and tool event relay
- Added `stream_thread_map: HashMap<String, String>` for tracking stream_id to thread_id
- StreamStart events populate the map; StreamEnd events clean it up
- Delta/End events look up thread_id from stream_id via the map
- Relay logic: non-origin relays to origin; origin fans out to all participating remotes
- Tool call events (started/progress/completed) already travel as `ainm.message.send` with type field, so existing federation relay handles them
- **Commit:** 0e21c33

### Task 3: /discover REPL command for remote agent card resolution
- Added `/discover agent@domain` command that parses agent@domain format
- Constructs HTTPS URL to `/.well-known/ainm/{agent_name}/agent-card.json`
- Uses `fetch_remote_agent_card` with `danger_accept_invalid_certs(true)` for demo self-signed certs
- Added to /help output
- **Commit:** 340dd07

## Deviations from Plan

None -- plan executed exactly as written.

## Known Stubs

None. All functionality is wired end-to-end.

## Self-Check: PASSED
