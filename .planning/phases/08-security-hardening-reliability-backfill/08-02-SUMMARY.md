---
phase: 08-security-hardening-reliability-backfill
plan: 02
subsystem: security
tags: [replay-protection, tofu, tls, certificate-pinning, federation, redb]

requires:
  - phase: 08-01
    provides: PersistenceLayer with check_and_insert_relay_id, get_tofu_pin, set_tofu_pin; ServerIdentity tls_fingerprints array
provides:
  - Replay protection on federation relay (relay_timestamp + relay_id validation)
  - TOFU key pinning for TLS certificate fingerprints
  - Certificate rotation auto-update via tls_fingerprints array
affects: [08-03, 08-04, 08-05]

tech-stack:
  added: []
  patterns: [TOFU-verify-then-serve in TLS accept loop, replay-reject-before-signature-verify]

key-files:
  created: []
  modified: [demo/src/relay.rs, demo/src/federation.rs, demo/src/identity.rs, demo/src/main.rs]

key-decisions:
  - "Replay protection runs before signature verification to reject stale/duplicate payloads cheaply"
  - "TOFU verify_tofu function placed in identity.rs since it operates on server identity concepts"

patterns-established:
  - "Replay protection: timestamp freshness (5-min past, 1-min future) + relay_id dedup via redb"
  - "TOFU pinning: first-contact pins fingerprint, rotation auto-updates when both old+new in published array"

requirements-completed: [FAUT-04, FAUT-05, FAUT-06]

duration: 8min
completed: 2026-03-29
---

# Phase 8 Plan 2: Replay Protection and TOFU Key Pinning Summary

**Federation relay hardened with replay rejection (timestamp + dedup), TOFU certificate pinning, and automatic cert rotation support**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-29T07:42:19Z
- **Completed:** 2026-03-29T07:50:19Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Outbound relay payloads now include relay_timestamp and relay_id fields, both covered by server signature
- Inbound relay handler validates timestamp freshness (5-min window) and deduplicates relay_ids via redb before signature verification
- TLS accept loop enforces TOFU key pinning with SSH-style first-contact behavior
- Certificate rotation auto-updates pins when new fingerprint appears alongside pinned one in tls_fingerprints array

## Task Commits

Each task was committed atomically:

1. **Task 1: Add replay protection fields and validation** - `719a4d8` (feat)
2. **Task 2: Implement TOFU key pinning and cert rotation** - `29fffbe` (feat)

## Files Created/Modified
- `demo/src/relay.rs` - Added relay_timestamp and relay_id to outbound relay params
- `demo/src/federation.rs` - Added replay protection validation (timestamp freshness + dedup) before signature verification
- `demo/src/identity.rs` - Added verify_tofu function for TOFU pinning with cert rotation support
- `demo/src/main.rs` - Replaced simple fingerprint check with TOFU-enhanced verification, passed persistence into TLS loop

## Decisions Made
- Replay protection runs before signature verification to reject stale/duplicate payloads cheaply without expensive crypto
- verify_tofu placed in identity.rs (not main.rs) since it operates on server identity and persistence concepts

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Replay protection and TOFU pinning active for all federation connections
- Plans 03-05 can build on the hardened federation layer (offline queue, backfill, cleanup)

---
*Phase: 08-security-hardening-reliability-backfill*
*Completed: 2026-03-29*

## Self-Check: PASSED
