---
phase: 05-demo-handshake-discovery-conformance
plan: 02
subsystem: demo
tags: [agent-card, well-known, discovery, url-path, documentation]

# Dependency graph
requires:
  - phase: 05-demo-handshake-discovery-conformance
    plan: 01
    provides: "Complete ainm.initialized handshake lifecycle in demo"
provides:
  - "Spec-correct Agent Card URL /.well-known/ainm/helper/agent-card.json in all demo locations"
  - "Accurate port binding documentation in demo.md"
affects: [demo, spec-conformance]

# Tech tracking
tech-stack:
  added: []
  patterns: []

key-files:
  created: []
  modified:
    - demo/src/server.rs
    - demo/src/main.rs
    - demo/src/repl.rs
    - website/docs/demo.md

key-decisions:
  - "Used literal path in Axum route (not :name parameter) since demo has exactly one agent"

patterns-established: []

requirements-completed: [DISC-01, DISC-02, SPEC-05]

# Metrics
duration: 2min
completed: 2026-03-29
---

# Phase 05 Plan 02: Agent Card URL and Port Documentation Fix Summary

**Corrected Agent Card URL from inverted agent-card/helper to spec-compliant /.well-known/ainm/helper/agent-card.json in all 4 demo locations, and fixed demo.md hardcoded port**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-29T00:09:36Z
- **Completed:** 2026-03-29T00:11:18Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Fixed Agent Card HTTP endpoint URL in server.rs Axum route from inverted `agent-card/helper` to spec-correct `helper/agent-card.json`
- Updated both identity store registrations in main.rs (initial placeholder + post-bind actual address) with correct URL
- Fixed REPL `/card` command HTTP fetch URL in repl.rs to use spec-correct path
- Corrected demo.md documentation from misleading hardcoded `127.0.0.1:9090` to accurate `127.0.0.1:0` (random port)
- Closes INT-02 from v3.1 milestone audit
- Zero stale references to old inverted URL path remain in source files

## Task Commits

Each task was committed atomically:

1. **Task 1: Update Agent Card URL path in all 4 demo locations** - `d8dfea9` (fix)
2. **Task 2: Fix demo.md port documentation** - `358329a` (fix)

## Files Created/Modified
- `demo/src/server.rs` - Axum route updated to `/.well-known/ainm/helper/agent-card.json`
- `demo/src/main.rs` - Both identity store registrations updated with correct Agent Card URL
- `demo/src/repl.rs` - /card HTTP fetch URL corrected to spec-compliant path
- `website/docs/demo.md` - Port documentation changed from hardcoded 9090 to random binding (127.0.0.1:0)

## Decisions Made
- Used literal path `/.well-known/ainm/helper/agent-card.json` in Axum route rather than a parameterized `:name` route, since the demo has exactly one agent and a literal path is simpler

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Known Stubs

None - all code is fully functional with no placeholder data.

## Next Phase Readiness
- Phase 05 complete: both handshake lifecycle (Plan 01) and discovery conformance (Plan 02) resolved
- Demo now exercises spec-correct Agent Card URL pattern per agent-card.md Section 3.2
- `cargo build` succeeds with no new warnings or errors

## Self-Check: PASSED

- All 4 modified files exist on disk
- Both task commits (d8dfea9, 358329a) found in git log
- SUMMARY.md created successfully

---
*Phase: 05-demo-handshake-discovery-conformance*
*Completed: 2026-03-29*
