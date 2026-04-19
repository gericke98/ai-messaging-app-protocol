---
gsd_state_version: 1.0
milestone: v3.3
milestone_name: Universal Agent Client
status: defining_requirements
stopped_at: Milestone started, defining requirements
last_updated: "2026-04-18T00:00:00.000Z"
last_activity: 2026-04-18
progress:
  total_phases: 0
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-18)

**Core value:** Enable mixed human-agent threads where any participant (human or agent) can seamlessly join, communicate, and coordinate with the same quality of experience.
**Current focus:** Defining v3.3 requirements — Universal Agent Client

## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-04-18 — Milestone v3.3 started

Progress: [░░░░░░░░░░] 0%

## Accumulated Context

### Decisions

- Callback closures for stream UI subscription instead of AsyncStream (simpler for phase 12 scope)
- PresenceNotifyParams co-located in StreamEvent.swift (shared notification pattern)

See PROJECT.md Key Decisions table for full history.

- [Phase 12]: setStreamCallbacks method on AINMClient actor for safe callback wiring from MainActor AppState
- [Phase 12]: Used AnyCodableValue.fromAny static method to bridge [String: Any] from onAction closures to Codable types
- [Phase 13]: @MainActor @Observable on ToolCallState since it only drives UI views
- [Phase 13]: Tool call indicators placed in MessageThreadView between permanent messages and streaming messages
- [Phase 13]: GRDB upsert pattern for tool call records (insert on started, update on progress/completed)
- [Phase 13]: Used enum namespace (MentionTextRenderer) for stateless highlight utility
- [Phase 13]: Actor-based AgentCardService for thread-safe HTTP + GRDB Agent Card caching
- [Phase 13]: CheckedContinuation-based pending response pattern for request-response methods during active receive loop
- [Phase 13]: Context frames stored as dictionary keyed by threadId in AppState for O(1) lookup
- [Phase 13]: Info.plist placed at project root to avoid Xcode auto-resource-copy conflict
- [Phase 13]: Sub-thread cache loaded eagerly in MessageThreadViewModel for O(1) indicator lookups
- [Phase 14-uat-automation-harness]: Programmatic pbxproj editing for new UI test target scaffold (UUIDs A2000003/A3000003/A8000003)
- [Phase 14-uat-automation-harness]: UITestIdentifiers.swift cross-target membership via explicit PBXFileReference+PBXBuildFile (SOURCE_ROOT path) rather than membershipExceptions — file lives outside UI test sync group
- [Phase 14-uat-automation-harness]: parallelizable=NO on UI test scheme TestableReference (shared simulator+server port 8080)
- [Phase 14-uat-automation-harness]: Named TODO(14-03)/TODO(14-04) anchor comments in AINMMessengerApp.swift for Wave 2 parallel editing without merge conflicts
- [Phase 14-uat-automation-harness]: UAT: ainm-demo --uat uses plain /health on legacy (non-federation) path; start_with_port registers both /health and /health/live for symmetry
- [Phase 14-uat-automation-harness]: Binary name authoritative: ainm-demo (CONTEXT.md D-05 incorrectly said ainm-server; every artifact uses ainm-demo)
- [Phase 14-uat-automation-harness]: tempfile = 3 added to demo dependencies for ephemeral UAT tempdir (guard lifetime tied to tokio::signal::ctrl_c await)
- [Phase 14-uat-automation-harness]: Makefile trap 'kill $PID' EXIT INT TERM covers normal exit + Ctrl-C + xcodebuild SIGTERM
- [Phase 14-uat-automation-harness]: Strong self + static activeInjectors pin keeps TestEventInjector alive across the launch-time async boundary (weak self would drop before events fire)
- [Phase 14-uat-automation-harness]: Codable ScriptEntry as flat field-union (all optional, selected by type) — JSON ergonomics; unknown types decode + skip at fire time
- [Phase 14-uat-automation-harness]: Injector translates flat ScriptFramePayload into nested ContextFrame.Frame at dispatch time (not in JSON) — reduces authoring friction while still exercising the real wire-shape code path
- [Phase 14-uat-automation-harness]: Storage-parameterized UITestIdentityBootstrap.resetIdentity(storage:) + KeyManager.InMemoryStorage injection in SeedLoaderIntegrationTests — works around iOS Simulator errSecMissingEntitlement (-34018) on SecItemDelete from unit-test hosts
- [Phase 14-uat-automation-harness]: ScenarioFixture schema is flat-per-record (one JSON field per GRDB column) — zero impedance mismatch between fixtures and ThreadRecord/ParticipantRecord/MessageRecord/ToolCallRecord/MemoryFactRecord/AgentCardRecord
- [Phase 14-uat-automation-harness]: Deterministic public key for uat-user@localhost is tFIlTgLccxHnpbEMNTDC39a8iCzbP1+kJW2Ii5WJWQI= (base64); downstream tests assert signatures against this constant without recomputing
- [Phase 14-uat-automation-harness]: CryptoKit Curve25519 signatures use randomized nonces (not RFC 8032 deterministic) — integration tests assert sign+verify round-trip instead of signature-byte equality
- [Phase 14-uat-automation-harness]: Fixtures live in app target at AINMMessenger/Testing/Fixtures/ and auto-bundle into .app root via synchronized root group — no pbxproj changes needed; Bundle.main.url(forResource:withExtension:) resolves them
- [Phase 14]: 14-05: MessageBubbleView gained optional messageId prop for UI test targeting; dual-attached SubThread.messageRow on bubble and on MessageGroupView per-message VStack (context menu host)
- [Phase 14]: 14-05: SubThreadIndicatorView gained parentMessageId prop so replyBadge identifier is per-message-specific
- [Phase 14]: 14-05: Alert OK buttons accept .accessibilityIdentifier directly in SwiftUI — no fallback path needed
- [Phase 14-uat-automation-harness]: Force-connect in DEBUG bootstrap (skip live server handshake) for hermetic UAT — tests don't need a real WebSocket, DB seed is sufficient
- [Phase 14-uat-automation-harness]: -UITestDeepLink launch arg replays URL through handleDeepLink (more reliable than simctl openurl on iOS 26 sims)
- [Phase 14-uat-automation-harness]: DeepLink.threadView identifier moved to zero-size overlay marker — attaching on body VStack was overriding all descendant identifiers via SwiftUI accessibility inheritance

### Roadmap Evolution

- Phase 14 added (2026-04-18): UAT automation harness — XCUITest coverage for 6 Phase 13 acceptance tests with hermetic seed bootstrap so HUMAN-UAT can run via `xcodebuild test`
- Milestone v3.3 started (2026-04-18): Universal Agent Client — ratify Phase A connector work into GSD, add Python SDK + MCP Bridge, upgrade OpenAI to Responses API, wire iOS live Agent Discovery, lay compound memory groundwork, close v3.2 tech debt as Phase 1

### Pending Todos

None.

### Blockers/Concerns

None active.

## Session Continuity

Last session: 2026-04-18T00:00:00.000Z
Stopped at: Milestone v3.3 defining requirements
Resume file: None
