---
phase: 08-security-hardening-reliability-backfill
plan: 01
subsystem: persistence, protocol, identity
tags: [redb, tls-fingerprints, replay-protection, backfill, federation]

# Dependency graph
requires:
  - phase: 07-federation-relay-core
    provides: ServerIdentity, federation relay, mTLS infrastructure
provides:
  - redb persistence layer with 4 tables (offline_queue, dedup_cache, tofu_pins, thread_messages)
  - tls_fingerprints array migration (supports certificate rotation)
  - relay_timestamp and relay_id fields for replay protection (spec + schema)
  - ainm.backfill method specification for thread history retrieval
  - -32026 ReplayDetected error code
affects: [08-02, 08-03, 08-04, 08-05]

# Tech tracking
tech-stack:
  added: [redb 3.1]
  patterns: [PersistenceLayer singleton via Arc<Database>, string-keyed tables with prefix-range scans, sequence-padded keys for ordered retrieval]

key-files:
  created: [demo/src/persistence.rs]
  modified: [spec/transport.md, schemas/transport.schema.json, demo/Cargo.toml, demo/src/identity.rs, demo/src/main.rs, demo/src/federation.rs]

key-decisions:
  - "Used -32026 for ReplayDetected instead of -32012 (already used for Expired key in security error codes)"
  - "Clean break on tls_fingerprint -> tls_fingerprints with no backwards compatibility shim (D-08, Phase 7 was demo-only)"
  - "Track sequence numbers from parsed keys in get_thread_messages to handle gaps correctly"

patterns-established:
  - "PersistenceLayer: Clone via Arc<Database>, single redb file, 4 tables"
  - "String-keyed tables with prefix:suffix pattern for range scans"
  - "Thread message keys: thread_id:sequence_padded (20-digit zero-padded)"
  - "Offline queue keys: destination:uuidv7 for FIFO ordering"

requirements-completed: [FAUT-05, FAUT-06]

# Metrics
duration: 10min
completed: 2026-03-29
---

# Phase 8 Plan 01: Foundation -- Spec Updates, Persistence Layer, Fingerprint Migration Summary

**Updated transport spec with replay protection/backfill, created redb persistence module with 4 tables, migrated ServerIdentity to tls_fingerprints array**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-29T07:28:14Z
- **Completed:** 2026-03-29T07:38:00Z
- **Tasks:** 2/2
- **Files modified:** 7 (1 created, 6 modified)

## Accomplishments

### Task 1: Update protocol spec and JSON schemas
- Updated spec/transport.md Section 5.2 to use `tls_fingerprints` array instead of singular `tls_fingerprint`
- Added `relay_timestamp` and `relay_id` fields to relay params (Section 5.3) for replay protection
- Added replay protection step 0 to relay validation (Section 5.4)
- Added Section 5.5 Backfill with `ainm.backfill` method specification
- Added `-32026` ReplayDetected error code to transport error codes (Section 3.6 and Section 7)
- Updated end-to-end federation example (Section 10) with all changes
- Added `BackfillParams`, `BackfillResult` definitions to transport.schema.json
- Added `relay_timestamp`, `relay_id` to RelayParams required fields in schema
- **Commit:** fd64616

### Task 2: Create redb persistence module, migrate ServerIdentity, wire into main.rs
- Added `redb = "3.1"` to Cargo.toml
- Created `demo/src/persistence.rs` with `PersistenceLayer` struct and 4 table definitions
- Implemented dedup cache (check_and_insert_relay_id, cleanup_expired_dedup)
- Implemented TOFU pin store (get_tofu_pin, set_tofu_pin)
- Implemented offline queue (enqueue_offline, drain_offline, queue_len, evict_oldest, cleanup_expired_queue)
- Implemented thread message store (store_thread_message, get_thread_messages with gap-safe cursor tracking)
- Migrated `ServerIdentity.tls_fingerprint: String` to `tls_fingerprints: Vec<String>`
- Updated `build_server_identity`, `find_by_fingerprint`, `server_identity_handler`
- Updated `resolve_remote_identity` to parse `tls_fingerprints` array (clean break, no fallback)
- Added `--data-dir` CLI argument with default `./data`
- Added `persistence` field to `FederationState`
- Codebase compiles cleanly
- **Commit:** c2de6d3

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Error code conflict: -32012 already used**
- **Found during:** Task 1
- **Issue:** Plan specified `-32012` for ReplayDetected, but `-32012` is already assigned to "Expired key" in AINM security error codes (Section 3.6)
- **Fix:** Used `-32026` (next available transport error code) instead
- **Files modified:** spec/transport.md, schemas/transport.schema.json

**2. [Rule 3 - Blocking] redb type inference failures**
- **Found during:** Task 2
- **Issue:** `map_err(|e| e.to_string())` closures failed type inference because redb returns different error types per operation. Also needed `ReadableDatabase` trait import for `begin_read()`.
- **Fix:** Created helper `fn s(e: impl Display) -> String` to provide concrete type for map_err, added `ReadableDatabase` to imports
- **Files modified:** demo/src/persistence.rs

## Known Stubs

None -- all persistence methods are fully implemented with working logic. Methods are currently unused (dead code warnings expected) until Plans 02-04 wire them in.

## Self-Check: PASSED

All created files verified present. All commit hashes verified in git log.
