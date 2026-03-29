---
phase: 07-federation-relay-core
plan: 03
subsystem: federation
tags: [relay, ed25519, json-rpc, mtls, exponential-backoff, thread-registry]

# Dependency graph
requires:
  - phase: 07-federation-relay-core/01
    provides: "CLI federation mode, clap args, federation dependencies"
  - phase: 07-federation-relay-core/02
    provides: "TLS module, ServerIdentity, RemoteServerCache, mTLS builders, certificates"
provides:
  - "Inbound relay handler with server + message signature verification"
  - "Thread registry with origin-server sequencing authority"
  - "Outbound relay sender with per-destination queues and backoff"
  - "Server-level sign_relay_params and verify_relay_signature"
affects: [07-federation-relay-core/04]

# Tech tracking
tech-stack:
  added: []
  patterns: ["server_signature field for relay-level signing", "thread ID format {domain}:{uuidv7}", "per-destination send queues with exponential backoff"]

key-files:
  created:
    - demo/src/federation.rs
    - demo/src/relay.rs
  modified:
    - demo/src/signing.rs
    - demo/src/protocol.rs
    - demo/src/main.rs

key-decisions:
  - "Relay handler validates layers 2+3 inline; mTLS layer 1 deferred to Plan 04 middleware"
  - "Thread ID format uses domain:uuidv7 for origin extraction without lookups"
  - "Background send loop polls every 500ms rather than event-driven (simpler for demo)"

patterns-established:
  - "server_signature: server-level Ed25519 signing distinct from per-message signature"
  - "ThreadRegistry: origin-only sequencing with register_remote_thread for non-origin servers"
  - "FederationState: shared state struct for all federation subsystem components"

requirements-completed: [RELY-02, RELY-03, RELY-04, RELY-05, FAUT-02, FAUT-03]

# Metrics
duration: 8min
completed: 2026-03-29
---

# Phase 7 Plan 3: Federation Relay Core Summary

**Inbound relay handler with triple-layer signature validation, thread registry with origin-server sequencing, and outbound relay sender with per-destination queues and exponential backoff**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-29T05:51:34Z
- **Completed:** 2026-03-29T05:59:56Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments
- Extended signing.rs with `sign_relay_params` and `verify_relay_signature` for server-level Ed25519 signing (FAUT-03)
- Created federation.rs with inbound relay handler performing server signature + per-message signature verification, thread registry with origin authority and sequence assignment
- Created relay.rs with outbound relay sender, per-destination send queues, exponential backoff (1s base, 60s max), and background send loop

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend signing.rs with server-level relay signing** - `576ed55` (feat)
2. **Task 2: Create federation.rs with relay handler and thread registry** - `a7b73d2` (feat)
3. **Task 3: Create relay.rs with outbound sender and send queues** - `efb261e` (feat)

## Files Created/Modified
- `demo/src/signing.rs` - Added sign_relay_params() and verify_relay_signature() for server-level relay signing
- `demo/src/federation.rs` - Inbound relay handler, ThreadRegistry, FederationState, parse_origin_domain
- `demo/src/relay.rs` - RelaySender with per-destination queues, BackoffState, flush_destination, start_send_loop
- `demo/src/protocol.rs` - Added new_error_value() helper for JSON-RPC error Value responses
- `demo/src/main.rs` - Added mod federation and mod relay declarations

## Decisions Made
- Relay handler validates FAUT-03 layers 2 (server signature) and 3 (per-message signature) inline; mTLS layer 1 check deferred to Plan 04 when the TLS acceptor loop is wired
- Thread ID format `{domain}:{uuidv7}` enables O(1) origin domain extraction via string split
- Send loop uses 500ms polling interval (simple timer) rather than event-driven notification (adequate for 2-server demo)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Merged main branch to get Plan 01/02 changes**
- **Found during:** Task 2 (federation.rs creation)
- **Issue:** Worktree was missing Plan 01/02 changes (tls.rs, identity ServerIdentity, clap CLI, Cargo.toml federation deps)
- **Fix:** Merged main branch into worktree to get required dependencies
- **Files modified:** Multiple (merge brought in tls.rs, updated identity.rs, main.rs, Cargo.toml, certs/)
- **Verification:** cargo check passes with all modules
- **Committed in:** merge commit

**2. [Rule 2 - Missing Critical] Added new_error_value to protocol.rs**
- **Found during:** Task 2 (federation.rs creation)
- **Issue:** Plan referenced protocol::new_error_value() but it didn't exist; relay handler returns Json<Value> not typed structs
- **Fix:** Added new_error_value() helper that builds JSON-RPC error as serde_json::Value
- **Files modified:** demo/src/protocol.rs
- **Verification:** cargo check passes
- **Committed in:** a7b73d2 (Task 2 commit)

**3. [Rule 3 - Blocking] Created relay.rs alongside federation.rs for compilation**
- **Found during:** Task 2 (federation.rs creation)
- **Issue:** federation.rs references crate::relay::RelaySender; cannot compile without relay module
- **Fix:** Created full relay.rs (Task 3 content) alongside federation.rs so both compile together
- **Files modified:** demo/src/relay.rs
- **Verification:** cargo check passes with all modules
- **Committed in:** efb261e (Task 3 commit)

---

**Total deviations:** 3 auto-fixed (1 blocking merge, 1 missing critical, 1 blocking dependency)
**Impact on plan:** All auto-fixes necessary for compilation. No scope creep.

## Issues Encountered
- Worktree was based on pre-Plan-01 state; required merge with main to get federation dependencies. Resolved by `git merge main`.

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - all functions are fully implemented. They are not yet wired into the server startup (that is Plan 04's responsibility).

## Next Phase Readiness
- federation.rs, relay.rs, and extended signing.rs are compiled and ready
- Plan 04 will wire these modules into the two-listener server architecture
- FederationState needs to be constructed in main.rs with actual server config
- relay_handler needs to be mounted as a POST route on the federation TLS listener

---
*Phase: 07-federation-relay-core*
*Completed: 2026-03-29*
