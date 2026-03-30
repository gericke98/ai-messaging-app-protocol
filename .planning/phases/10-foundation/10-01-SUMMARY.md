---
phase: 10-foundation
plan: 01
subsystem: ios, crypto, models
tags: [swift6, swiftui, jcs, rfc8785, base64url, json-rpc, codable, xcode, grdb, keychainaccess]

requires:
  - phase: v3.1 protocol spec
    provides: JSON schemas (transport, security, threading, common), Rust protocol.rs reference
provides:
  - iOS Xcode project with Swift 6 strict concurrency enabled
  - Complete Codable model layer matching AINM JSON schemas
  - RFC 8785 JCS canonicalizer for cryptographic signing
  - Base64url encoding/decoding for Ed25519 signatures
  - GRDB.swift and KeychainAccess SPM dependencies resolved
affects: [10-02 (Ed25519 signing uses JCS + base64url), 10-03 (GRDB persistence uses models), 10-04 (WebSocket uses JsonRpc types), 10-05 (UI uses models)]

tech-stack:
  added: [SwiftUI iOS 18, GRDB.swift 7.10.0, KeychainAccess 4.2.2, Apple CryptoKit, Swift Testing]
  patterns: [MVVM folder structure, Codable+Sendable models, snake_case CodingKeys, JCS for signing input, TDD for crypto]

key-files:
  created:
    - ios/AINMMessenger/AINMMessenger.xcodeproj/project.pbxproj
    - ios/AINMMessenger/AINMMessenger/App/AINMMessengerApp.swift
    - ios/AINMMessenger/AINMMessenger/Models/JsonRpc.swift
    - ios/AINMMessenger/AINMMessenger/Models/MessageEnvelope.swift
    - ios/AINMMessenger/AINMMessenger/Models/AnyCodableValue.swift
    - ios/AINMMessenger/AINMMessenger/Models/InitializeParams.swift
    - ios/AINMMessenger/AINMMessenger/Models/InitializeResult.swift
    - ios/AINMMessenger/AINMMessenger/Models/ThreadMetadata.swift
    - ios/AINMMessenger/AINMMessenger/Models/Participant.swift
    - ios/AINMMessenger/AINMMessenger/Crypto/JCSCanonicalizer.swift
    - ios/AINMMessenger/AINMMessenger/Crypto/Base64url.swift
    - ios/AINMMessenger/AINMMessengerTests/CryptoTests/JCSCanonicalizerTests.swift
    - ios/AINMMessenger/AINMMessengerTests/CryptoTests/Base64urlTests.swift
    - ios/AINMMessenger/AINMMessengerTests/NetworkingTests/JsonRpcTests.swift
  modified:
    - .gitignore

key-decisions:
  - "Used PBXFileSystemSynchronizedRootGroup (Xcode 16+ format) for automatic source file discovery"
  - "Used CFBooleanGetTypeID() for reliable NSNumber bool/int distinction in JCS canonicalizer"
  - "iPhone 17 Pro simulator target (Xcode 26.3 does not ship iPhone 16 simulator)"

patterns-established:
  - "Codable+Sendable: All protocol model types conform to both Codable and Sendable for Swift 6"
  - "CodingKeys snake_case: Swift camelCase properties map to JSON snake_case via CodingKeys"
  - "Optional field omission: nil fields omitted in JSON encoding (matching Rust skip_serializing_if)"
  - "JCS for signing: Canonicalize -> UTF-8 bytes -> Ed25519 sign -> base64url encode"
  - "TDD for crypto: Tests written before implementation for correctness-critical code"

requirements-completed: [NET-06]

duration: 32min
completed: 2026-03-30
---

# Phase 10 Plan 01: Xcode Project, Protocol Models, JCS Canonicalization Summary

**iOS Xcode project with Swift 6 strict concurrency, 7 Codable model types matching AINM JSON schemas, and RFC 8785 JCS canonicalizer with base64url encoding -- 28 passing tests**

## Performance

- **Duration:** 32 min
- **Started:** 2026-03-30T01:11:13Z
- **Completed:** 2026-03-30T01:43:29Z
- **Tasks:** 3
- **Files modified:** 15

## Accomplishments
- Xcode project builds with Swift 6 strict concurrency (SWIFT_STRICT_CONCURRENCY=complete) and iOS 18.0 target
- All AINM protocol types (JsonRpc, MessageEnvelope, InitializeParams/Result, ThreadMetadata, Participant) as Codable+Sendable structs with snake_case CodingKeys
- RFC 8785 JCS canonicalizer with UTF-16 code unit key sorting, ECMAScript number serialization, and proper string escaping
- Base64url encoding/decoding per RFC 4648 Section 5 with Curve25519 public key extension
- 28 unit tests passing across 3 test suites (JsonRpcTests, JCSCanonicalizerTests, Base64urlTests)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create Xcode project** - `d90b210` (feat)
2. **Task 2: Define protocol Codable models** - `50c3ef6` (feat)
3. **Task 3: JCS canonicalizer + base64url** - `a238d43` (test/RED), `8fc681d` (feat/GREEN)
4. **Gitignore update** - `4d79872` (chore)

## Files Created/Modified
- `ios/AINMMessenger/AINMMessenger.xcodeproj/project.pbxproj` - Xcode project with Swift 6, GRDB, KeychainAccess
- `ios/AINMMessenger/AINMMessenger/App/AINMMessengerApp.swift` - Minimal @main entry with NavigationStack
- `ios/AINMMessenger/AINMMessenger/Models/JsonRpc.swift` - JSON-RPC 2.0 request/response/notification/error
- `ios/AINMMessenger/AINMMessenger/Models/MessageEnvelope.swift` - Signed message envelope + ContentBlock
- `ios/AINMMessenger/AINMMessenger/Models/AnyCodableValue.swift` - Type-erased JSON value enum
- `ios/AINMMessenger/AINMMessenger/Models/InitializeParams.swift` - ainm.initialize params + ClientCapabilities
- `ios/AINMMessenger/AINMMessenger/Models/InitializeResult.swift` - Server info + ServerCapabilities
- `ios/AINMMessenger/AINMMessenger/Models/ThreadMetadata.swift` - Thread metadata from threading schema
- `ios/AINMMessenger/AINMMessenger/Models/Participant.swift` - Participant with type/role enums
- `ios/AINMMessenger/AINMMessenger/Crypto/JCSCanonicalizer.swift` - RFC 8785 JSON canonicalization
- `ios/AINMMessenger/AINMMessenger/Crypto/Base64url.swift` - Base64url Data extension + PublicKey extension
- `ios/AINMMessenger/AINMMessengerTests/CryptoTests/JCSCanonicalizerTests.swift` - 12 JCS tests
- `ios/AINMMessenger/AINMMessengerTests/CryptoTests/Base64urlTests.swift` - 8 base64url tests
- `ios/AINMMessenger/AINMMessengerTests/NetworkingTests/JsonRpcTests.swift` - 8 JSON-RPC tests
- `.gitignore` - Added Xcode workspace/userdata patterns

## Decisions Made
- Used PBXFileSystemSynchronizedRootGroup (Xcode 16+ format) instead of explicit file references -- automatic source file discovery simplifies project maintenance
- Used CFBooleanGetTypeID() for NSNumber bool detection in JCS -- more reliable than objCType checks since Swift Bool and Int both bridge to NSNumber
- Test target uses iPhone 17 Pro simulator (Xcode 26.3 ships iOS 26.2 simulators, not iOS 18 ones -- but deployment target remains iOS 18.0)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added build phases to Xcode project**
- **Found during:** Task 1 (Xcode project creation)
- **Issue:** Initial project had empty buildPhases arrays, causing no binary to be produced (app bundle missing executable)
- **Fix:** Added Sources, Frameworks, and Resources build phases to both app and test targets
- **Files modified:** ios/AINMMessenger/AINMMessenger.xcodeproj/project.pbxproj
- **Verification:** xcodebuild build produces AINMMessenger binary in .app bundle
- **Committed in:** 50c3ef6 (part of Task 2 commit)

**2. [Rule 1 - Bug] Fixed JCS test for MessageEnvelope shape**
- **Found during:** Task 3 (TDD GREEN phase)
- **Issue:** Original test used position-based key ordering assertions that failed due to nested content block key positions overlapping with outer keys
- **Fix:** Changed to exact string comparison against known canonical output
- **Files modified:** ios/AINMMessenger/AINMMessengerTests/CryptoTests/JCSCanonicalizerTests.swift
- **Verification:** All 12 JCS tests pass
- **Committed in:** 8fc681d (Task 3 GREEN commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both fixes necessary for correctness. No scope creep.

## Issues Encountered
- iPhone 16 simulator not available in Xcode 26.3 -- used iPhone 17 Pro instead. No functional impact.

## User Setup Required
None - no external service configuration required.

## Known Stubs
None -- all code is functional and tested.

## Next Phase Readiness
- Model layer complete: Plans 02-05 can import all Codable types
- JCS + base64url ready: Plan 02 (Ed25519 signing) depends on these crypto primitives
- GRDB dependency resolved: Plan 03 (persistence) can start immediately
- Folder structure in place: Networking/, Crypto/, Persistence/, UI/ directories ready

## Self-Check: PASSED

All 10 key files verified present. All 5 commit hashes verified in git log.

---
*Phase: 10-foundation*
*Completed: 2026-03-30*
