---
phase: 08-security-hardening-reliability-backfill
plan: 03
subsystem: federation
tags: [redb, persistence, relay, queue, backoff, docker]

requires:
  - phase: 08-01
    provides: PersistenceLayer with redb-backed offline queue, dedup cache, TOFU pins
  - phase: 08-02
    provides: Replay protection fields and TOFU key pinning in federation relay
provides:
  - Persistent offline queue replacing in-memory VecDeque in RelaySender
  - 10K per-destination bound with FIFO eviction
  - Hourly background cleanup for dedup cache (10-min) and queue (7-day retention)
  - Docker named volumes for redb data persistence across container restarts
affects: [08-04, 08-05]

tech-stack:
  added: []
  patterns: [redb-backed persistent queue with in-memory backoff, background cleanup task pattern]

key-files:
  created: []
  modified:
    - demo/src/relay.rs
    - demo/src/main.rs
    - demo/docker-compose.yml

key-decisions:
  - "Backoff state kept in-memory (resets on restart per D-14) while queue contents persist in redb"
  - "Batch drain of 100 messages per flush to balance throughput and memory"
  - "10-minute dedup cache expiry (generous beyond 5-min replay window) for cleanup"

patterns-established:
  - "Persistent queue pattern: persist first, enforce bounds after, re-enqueue on failure"
  - "Background cleanup pattern: tokio interval task with structured logging"

requirements-completed: [FREL-01, FREL-02, FREL-03, FREL-04]

duration: 4min
completed: 2026-03-29
---

# Phase 8 Plan 3: Persistent Offline Queue Summary

**Redb-backed persistent relay queue replacing VecDeque with 10K bounds, 7-day retention, and Docker volume support for partition healing**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-29T07:51:31Z
- **Completed:** 2026-03-29T07:55:14Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Replaced in-memory VecDeque with redb-backed persistent queue that survives server restarts
- Enforced 10K per-destination message bound with FIFO eviction of oldest messages
- Added hourly background cleanup task for dedup cache (10-min expiry) and queue (7-day retention)
- Added Docker named volumes so redb data persists across container restarts for partition healing demos

## Task Commits

Each task was committed atomically:

1. **Task 1: Replace VecDeque with redb-backed persistent queue in RelaySender** - `3dabcc5` (feat)
2. **Task 2: Add background cleanup task and Docker volume for persistent data** - `28f0916` (feat)

## Files Created/Modified
- `demo/src/relay.rs` - Replaced VecDeque with PersistenceLayer calls for enqueue/drain/eviction, kept BackoffState in-memory
- `demo/src/main.rs` - Pass persistence to RelaySender, spawn hourly cleanup task for dedup and queue
- `demo/docker-compose.yml` - Added named volumes (server-a-data, server-b-data) and --data-dir flags

## Decisions Made
- Kept BackoffState in-memory per D-14 (resets on restart), only queue contents persist
- Batch drain of 100 messages per flush balances throughput and memory usage
- 10-minute dedup cache cleanup expiry provides margin beyond the 5-minute replay window

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Persistent queue foundation is ready for 08-04 (thread message history backfill)
- Docker volumes support partition healing test scenarios in 08-05

---
*Phase: 08-security-hardening-reliability-backfill*
*Completed: 2026-03-29*
