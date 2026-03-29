---
phase: 01-protocol-kernel
plan: 01
subsystem: protocol-spec
tags: [ed25519, rfc8785, jcs, identity, security, signing, cryptography, base64url, json-rpc]

# Dependency graph
requires: []
provides:
  - "AINM identity specification: address format, resolution, registration, key rotation"
  - "AINM security specification: message signing (RFC 8785 + Ed25519), verification, trust annotations, deduplication"
  - "Test key pair and test vectors for implementer conformance testing"
affects: [01-02-PLAN, 01-03-PLAN, phase-02]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Spec document structure: Overview, Terminology, core sections, Security Considerations, Examples/References"
    - "RFC 8785 JCS canonicalization for deterministic signing input"
    - "Ed25519 (RFC 8032) for all message signatures"
    - "base64url without padding (RFC 4648 Section 5) for keys and signatures"
    - "JSON-RPC error codes in -32001 to -32013 range for identity and security errors"
    - "UUIDv7 (RFC 9562) for time-ordered message IDs and deduplication"
    - "Trust annotations via source field (user/agent/system) on content blocks"

key-files:
  created:
    - spec/identity.md
    - spec/security.md
  modified: []

key-decisions:
  - "Used RFC 8032 Section 7.1 test vector 1 key pair as the published test key pair for implementers"
  - "Defined 24-hour overlap period for key rotation grace, balancing security vs. in-flight message verification"
  - "JSON-RPC error code ranges: -32001 to -32007 for identity errors, -32010 to -32013 for security errors"
  - "Deduplication window: at least 10,000 IDs or 24 hours, whichever is larger"
  - "Trust annotation cross-checking: SHOULD cross-check source field against sender type from identity resolution"

patterns-established:
  - "Spec document template: Overview > Terminology > Core sections with inline JSON > Security Considerations > Examples/References"
  - "Error response format: JSON-RPC error body with code, message, optional data"
  - "Key encoding: base64url without padding throughout all specs"
  - "Endpoint pattern: .well-known/ainm/{resource} for all identity operations"

requirements-completed: [IDNT-01, IDNT-02, IDNT-03, IDNT-04, IDNT-05, SECU-01, SECU-02, SECU-03, SECU-04, SECU-05]

# Metrics
duration: 6min
completed: 2026-03-28
---

# Phase 1 Plan 1: Identity & Security Specification Summary

**AINM identity (name@provider, .well-known resolution, Ed25519 key binding) and security (RFC 8785 JCS + Ed25519 signing, trust annotations, deduplication) specifications with 3 test vectors**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-28T08:24:49Z
- **Completed:** 2026-03-28T08:31:06Z
- **Tasks:** 2
- **Files created:** 2

## Accomplishments
- Wrote complete identity specification covering address format (name@provider), identity resolution via .well-known/ainm/{name}, registration endpoint, and key rotation with history array and 24-hour overlap period
- Wrote complete security specification covering message envelope (7 fields), step-by-step signing and verification procedures using RFC 8785 JCS + Ed25519, trust annotations (user/agent/system), deduplication with UUIDv7, and JSON-RPC error codes
- Included 3 test vectors (simple text, Unicode, multi-block) with expected canonical forms and a published test Ed25519 key pair from RFC 8032

## Task Commits

Each task was committed atomically:

1. **Task 1: Write Identity Specification** - `596e5a3` (feat)
2. **Task 2: Write Security Specification** - `234715c` (feat)

## Files Created/Modified
- `spec/identity.md` - AINM identity specification: address format, resolution via .well-known, registration, key rotation with history
- `spec/security.md` - AINM security specification: message envelope, signing/verification procedures, trust annotations, deduplication, test vectors

## Decisions Made
- **Test key pair selection:** Used the well-known Ed25519 test vector 1 from RFC 8032 Section 7.1 as the published test key pair. This is a recognized, reproducible key pair that implementers can easily verify against the RFC.
- **Overlap period duration:** Defined a 24-hour grace period after key rotation where the old key remains valid for verification. This balances security (limiting exposure window) with practicality (allowing in-flight messages and cache expiry).
- **Error code ranges:** Allocated -32001 through -32007 for identity-related errors and -32010 through -32013 for security-related errors. Leaves room for future additions in each range.
- **Deduplication window:** Set at "10,000 IDs or 24 hours, whichever is larger" -- allows small implementations to use a bounded set while large deployments benefit from time-based eviction.
- **Trust annotation cross-checking:** Made cross-checking source vs sender type a SHOULD (not MUST). Clients benefit from the check but implementations can ship without it.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Identity and security specs are complete and ready to be consumed by the transport spec (01-02-PLAN), content/threading spec (01-03-PLAN), and Phase 2 agent capability discovery
- Both specs cross-reference each other and use consistent terminology/address formats
- The test vectors in security.md enable early conformance testing for future reference implementations
- The JSON-RPC error code scheme provides a foundation for error codes in transport.md

## Self-Check: PASSED

- spec/identity.md: FOUND
- spec/security.md: FOUND
- 01-01-SUMMARY.md: FOUND
- commit 596e5a3: FOUND
- commit 234715c: FOUND

---
*Phase: 01-protocol-kernel*
*Completed: 2026-03-28*
