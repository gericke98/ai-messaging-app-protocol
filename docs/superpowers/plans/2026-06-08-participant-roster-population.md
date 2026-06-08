# Live Participant-Roster Population Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Populate the `participant` table on the live path (thread creation, message delivery, lazy backfill on open) so SP3's `isMultiParticipant` and per-sender type are correct in production.

**Architecture:** A pure `ParticipantClassifier` (type decision) + an idempotent `ParticipantRepository.upsert` (earliest-joinedAt-wins, creator-sticky role, human→agent-only type upgrade), called from three write points. No schema migration — the `participant` table already exists with composite PK `(threadId, identity)`. Display/data-only: no protocol, server, or network change.

**Tech Stack:** Swift 6 / SwiftUI (iOS 18+), GRDB 7.x, XCTest. Build/test via `xcodebuild`. iOS project at `ios/AINMMessenger`.

---

## Conventions (read once before starting)

- **Git:** ALL git operations use `rtk proxy git ...` (never bare `git`). Commit directly to `main` (project's established pattern — no feature branch). Do **not** push to origin; that is a separate user-gated step after the whole plan completes.
- **Build (unit tests):**
  ```
  xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
    -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test
  ```
  Scope one class with `-only-testing:AINMMessengerTests/<ClassName>`. UITests use `-scheme AINMMessengerUITests`.
- **pbxproj hygiene:** New `.swift` files under `AINMMessenger/` and `AINMMessengerTests/` auto-include via the synchronized folder group — **zero** pbxproj edits. After any `xcodebuild` run, if `project.pbxproj` or any `*.xcscheme` was rewritten by Xcode, revert it: `rtk proxy git checkout -- <path>`. Never commit those auto-rewrites.
- **SourceKit noise:** in-editor "Cannot find 'X' in scope" / "No such module 'GRDB'/'XCTest'" are stale-index false positives. Trust only `xcodebuild`.
- **Existing shapes (do not redefine):**
  - `ParticipantRecord` (`Persistence/ParticipantRecord.swift`) — `FetchableRecord, PersistableRecord`, table `participant`, composite PK `(threadId, identity)`. Memberwise init order: `(threadId:identity:type:joinedAt:role:)`.
  - `ThreadRecord` has `createdBy: String`, `agentAddress: String?`.
  - `MessageRecord` has `fromAddress`, `timestamp`, `contentJson`, `toAddress` (SP3).
  - `MessageEnvelope` has `from`, `timestamp`, `threadId`, `content: [ContentBlock]`. `ContentBlock.text(TextContent)` where `TextContent.source: String`.
  - `savedAgent` table (PK `address`). `AppState.savedAgents: [SavedAgentRecord]` (`.address`).
  - `AppDatabase.makeInMemory()` runs all migrations; `db.dbPool` is a `DatabasePool`.
  - GRDB: fetch by composite PK = `ParticipantRecord.fetchOne(db, key: ["threadId": t, "identity": i])`.

---

## File Structure

**New files:**
- `ios/AINMMessenger/AINMMessenger/Models/ParticipantClassifier.swift` — pure type decision.
- `ios/AINMMessenger/AINMMessenger/Persistence/ParticipantRepository.swift` — idempotent upsert (sync in-`db` static + async wrapper).
- `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/ParticipantClassifierTests.swift`
- `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/ParticipantRepositoryTests.swift`
- `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/ParticipantPopulationTests.swift` (delivery + backfill integration)

**Modified files:**
- `ios/AINMMessenger/AINMMessenger/Networking/AINMClient.swift` — store `connectedIdentity`; upsert sender on delivery (best-effort, isolated).
- `ios/AINMMessenger/AINMMessenger/App/AppState.swift` — `startThread(withAgent:)` seeds user + agent.
- `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadViewModel.swift` — lazy backfill in `loadInitial`.

---

### Task PRP-1: `ParticipantClassifier` (pure)

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/Models/ParticipantClassifier.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/ParticipantClassifierTests.swift`

- [ ] **Step 1: Write the failing test**

```swift
import XCTest
@testable import AINMMessenger

final class ParticipantClassifierTests: XCTestCase {

    func testSelfIsAlwaysHuman() {
        let t = ParticipantClassifier.classify(
            address: "me@localhost", isSelf: true, agentAddress: "me@localhost",
            savedAgentAddresses: ["me@localhost"], contentSource: "agent")
        XCTAssertEqual(t, "human")   // self precedence beats every agent signal
    }

    func testThreadAgentAddressIsAgent() {
        let t = ParticipantClassifier.classify(
            address: "assistant@localhost", isSelf: false, agentAddress: "assistant@localhost",
            savedAgentAddresses: [], contentSource: nil)
        XCTAssertEqual(t, "agent")
    }

    func testSavedAgentIsAgent() {
        let t = ParticipantClassifier.classify(
            address: "researcher@acme.test", isSelf: false, agentAddress: nil,
            savedAgentAddresses: ["researcher@acme.test"], contentSource: nil)
        XCTAssertEqual(t, "agent")
    }

    func testContentSourceAgentIsAgent() {
        let t = ParticipantClassifier.classify(
            address: "x@host", isSelf: false, agentAddress: nil,
            savedAgentAddresses: [], contentSource: "agent")
        XCTAssertEqual(t, "agent")
    }

    func testUnknownDefaultsHuman() {
        let t = ParticipantClassifier.classify(
            address: "stranger@host", isSelf: false, agentAddress: "assistant@localhost",
            savedAgentAddresses: ["researcher@acme.test"], contentSource: "user")
        XCTAssertEqual(t, "human")
    }

    func testNilContentSourceStillResolves() {
        let t = ParticipantClassifier.classify(
            address: "stranger@host", isSelf: false, agentAddress: nil,
            savedAgentAddresses: [], contentSource: nil)
        XCTAssertEqual(t, "human")
    }
}
```

- [ ] **Step 2: Run it; expect FAIL** (`ParticipantClassifier` undefined).

Run:
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/ParticipantClassifierTests
```

- [ ] **Step 3: Write the implementation**

```swift
import Foundation

/// Pure decision: classify a thread participant as "human" or "agent" using the only
/// signals available client-side (there is no authoritative sender-type on the wire).
/// Precedence: self → human; then thread's agent address; then a card-resolved saved
/// agent; then the per-message content `source`; else default human.
enum ParticipantClassifier {

    static func classify(
        address: String,
        isSelf: Bool,
        agentAddress: String?,
        savedAgentAddresses: Set<String>,
        contentSource: String?
    ) -> String {
        if isSelf { return "human" }
        if let agentAddress, address == agentAddress { return "agent" }
        if savedAgentAddresses.contains(address) { return "agent" }
        if contentSource == "agent" { return "agent" }
        return "human"
    }
}
```

- [ ] **Step 4: Run the test; expect PASS (6 tests).** Revert any pbxproj rewrite.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Models/ParticipantClassifier.swift \
  ios/AINMMessenger/AINMMessengerTests/PersistenceTests/ParticipantClassifierTests.swift
rtk proxy git commit -m "feat(roster): ParticipantClassifier pure type decision + tests"
```

---

### Task PRP-2: `ParticipantRepository.upsert` (idempotent merge)

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/Persistence/ParticipantRepository.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/ParticipantRepositoryTests.swift`

- [ ] **Step 1: Write the failing test**

```swift
import XCTest
import GRDB
@testable import AINMMessenger

final class ParticipantRepositoryTests: XCTestCase {
    var db: AppDatabase!

    override func setUp() async throws {
        db = try AppDatabase.makeInMemory()
        // FK parent: participant.threadId references thread.id
        try await db.dbPool.write { dbx in
            try ThreadRecord(
                id: "t1", title: nil, createdAt: "2026-06-08T09:00:00.000Z",
                createdBy: "me@localhost", parentThreadId: nil, parentMessageId: nil
            ).save(dbx)
        }
    }

    private func fetch(_ identity: String) throws -> ParticipantRecord? {
        try db.dbPool.read { dbx in
            try ParticipantRecord.fetchOne(dbx, key: ["threadId": "t1", "identity": identity])
        }
    }

    func testInsertsWhenAbsent() async throws {
        try await db.dbPool.write { dbx in
            try ParticipantRepository.upsert(dbx, threadId: "t1", identity: "a@h",
                type: "human", role: "member", joinedAt: "2026-06-08T10:00:00.000Z")
        }
        XCTAssertEqual(try fetch("a@h")?.type, "human")
    }

    func testReUpsertIsIdempotentNoDuplicate() async throws {
        try await db.dbPool.write { dbx in
            try ParticipantRepository.upsert(dbx, threadId: "t1", identity: "a@h",
                type: "human", role: "member", joinedAt: "2026-06-08T10:00:00.000Z")
            try ParticipantRepository.upsert(dbx, threadId: "t1", identity: "a@h",
                type: "human", role: "member", joinedAt: "2026-06-08T10:05:00.000Z")
        }
        let count = try db.dbPool.read { dbx in
            try ParticipantRecord.filter(Column("threadId") == "t1").fetchCount(dbx)
        }
        XCTAssertEqual(count, 1)
    }

    func testEarliestJoinedAtWins() async throws {
        try await db.dbPool.write { dbx in
            try ParticipantRepository.upsert(dbx, threadId: "t1", identity: "a@h",
                type: "human", role: "member", joinedAt: "2026-06-08T10:00:00.000Z")
            try ParticipantRepository.upsert(dbx, threadId: "t1", identity: "a@h",
                type: "human", role: "member", joinedAt: "2026-06-08T09:30:00.000Z")
        }
        XCTAssertEqual(try fetch("a@h")?.joinedAt, "2026-06-08T09:30:00.000Z")
    }

    func testTypeUpgradesHumanToAgent() async throws {
        try await db.dbPool.write { dbx in
            try ParticipantRepository.upsert(dbx, threadId: "t1", identity: "a@h",
                type: "human", role: "member", joinedAt: "2026-06-08T10:00:00.000Z")
            try ParticipantRepository.upsert(dbx, threadId: "t1", identity: "a@h",
                type: "agent", role: "member", joinedAt: "2026-06-08T10:01:00.000Z")
        }
        XCTAssertEqual(try fetch("a@h")?.type, "agent")
    }

    func testAgentIsNotDowngraded() async throws {
        try await db.dbPool.write { dbx in
            try ParticipantRepository.upsert(dbx, threadId: "t1", identity: "a@h",
                type: "agent", role: "member", joinedAt: "2026-06-08T10:00:00.000Z")
            try ParticipantRepository.upsert(dbx, threadId: "t1", identity: "a@h",
                type: "human", role: "member", joinedAt: "2026-06-08T10:01:00.000Z")
        }
        XCTAssertEqual(try fetch("a@h")?.type, "agent")
    }

    func testCreatorRoleIsSticky() async throws {
        try await db.dbPool.write { dbx in
            try ParticipantRepository.upsert(dbx, threadId: "t1", identity: "a@h",
                type: "human", role: "creator", joinedAt: "2026-06-08T10:00:00.000Z")
            try ParticipantRepository.upsert(dbx, threadId: "t1", identity: "a@h",
                type: "human", role: "member", joinedAt: "2026-06-08T10:01:00.000Z")
        }
        XCTAssertEqual(try fetch("a@h")?.role, "creator")
    }
}
```

- [ ] **Step 2: Run it; expect FAIL** (`ParticipantRepository` undefined).

- [ ] **Step 3: Write the implementation**

```swift
import GRDB
import Foundation

/// Idempotent persistence for thread participants. The merge is order-independent so the
/// three write points (thread-create seed, delivery upsert, lazy backfill) can run in any
/// order any number of times and converge: earliest joinedAt wins, role is creator-sticky,
/// and type upgrades human→agent only (a confirmed agent never regresses to human).
struct ParticipantRepository: Sendable {
    let dbPool: DatabasePool

    /// Upsert within an existing write transaction (for callers already inside `dbPool.write`).
    static func upsert(
        _ db: Database,
        threadId: String,
        identity: String,
        type: String,
        role: String,
        joinedAt: String
    ) throws {
        if let existing = try ParticipantRecord.fetchOne(
            db, key: ["threadId": threadId, "identity": identity]
        ) {
            let mergedType = (existing.type == "agent" || type == "agent") ? "agent" : "human"
            let mergedRole = (existing.role == "creator" || role == "creator") ? "creator" : "member"
            let mergedJoinedAt = min(existing.joinedAt, joinedAt)   // RFC3339 → lexicographic = chronological
            // No-op if nothing changed (avoids a redundant write).
            if mergedType == existing.type, mergedRole == existing.role, mergedJoinedAt == existing.joinedAt {
                return
            }
            try ParticipantRecord(
                threadId: threadId, identity: identity,
                type: mergedType, joinedAt: mergedJoinedAt, role: mergedRole
            ).update(db)
        } else {
            try ParticipantRecord(
                threadId: threadId, identity: identity,
                type: type, joinedAt: joinedAt, role: role
            ).insert(db)
        }
    }

    /// Async convenience: open a write and upsert.
    func upsert(
        threadId: String, identity: String, type: String, role: String, joinedAt: String
    ) async throws {
        try await dbPool.write { db in
            try ParticipantRepository.upsert(
                db, threadId: threadId, identity: identity, type: type, role: role, joinedAt: joinedAt)
        }
    }
}
```

> Note: `joinedAt` strings are RFC 3339 with milliseconds + `Z` (fixed-width, UTC) across the codebase, so lexicographic `min` equals chronological min.

- [ ] **Step 4: Run the test; expect PASS (6 tests).** Revert any pbxproj rewrite.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Persistence/ParticipantRepository.swift \
  ios/AINMMessenger/AINMMessengerTests/PersistenceTests/ParticipantRepositoryTests.swift
rtk proxy git commit -m "feat(roster): idempotent ParticipantRepository.upsert (earliest/creator-sticky/agent-upgrade) + tests"
```

---

### Task PRP-3: Seed participants at thread creation

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/App/AppState.swift` (`startThread(withAgent:)`, ~line 680)
- Test: extend `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/ParticipantPopulationTests.swift` (create in this task)

- [ ] **Step 1: Write the failing test** — create `ParticipantPopulationTests.swift`:

```swift
import XCTest
import GRDB
@testable import AINMMessenger

@MainActor
final class ParticipantPopulationTests: XCTestCase {

    /// Helper: count participant rows for a thread.
    func participantCount(_ db: AppDatabase, _ threadId: String) throws -> Int {
        try db.dbPool.read { dbx in
            try ParticipantRecord.filter(Column("threadId") == threadId).fetchCount(dbx)
        }
    }
    func participant(_ db: AppDatabase, _ threadId: String, _ identity: String) throws -> ParticipantRecord? {
        try db.dbPool.read { dbx in
            try ParticipantRecord.fetchOne(dbx, key: ["threadId": threadId, "identity": identity])
        }
    }

    func test_startThread_seedsUserAndAgent() async throws {
        let db = try AppDatabase.makeInMemory()
        let appState = AppState()
        appState.appDatabase = db
        appState.connectedIdentity = "me@localhost"

        let threadId = await appState.startThread(withAgent: "assistant@localhost")
        let tid = try XCTUnwrap(threadId)

        XCTAssertEqual(try participantCount(db, tid), 2)
        XCTAssertEqual(try participant(db, tid, "me@localhost")?.type, "human")
        XCTAssertEqual(try participant(db, tid, "me@localhost")?.role, "creator")
        XCTAssertEqual(try participant(db, tid, "assistant@localhost")?.type, "agent")
        XCTAssertEqual(try participant(db, tid, "assistant@localhost")?.role, "member")
    }
}
```

> If `AppState()` / `appState.appDatabase` / `appState.connectedIdentity` are not directly settable in a test (check their access level first), construct `AppState` the same way the existing AppState tests do and set the database/identity via whatever path those tests use. Do NOT change production access levels solely for the test unless the existing tests already rely on that access.

- [ ] **Step 2: Run it; expect FAIL** (only the thread row, 0 participants → count 2 fails).

- [ ] **Step 3: Implement the seed.** In `AppState.startThread(withAgent:)`, replace the write block

```swift
        do {
            try await db.dbPool.write { db in try record.save(db) }
        } catch {
            return nil
        }
```

with one that also seeds both participants in the same transaction:

```swift
        do {
            try await db.dbPool.write { db in
                try record.save(db)
                try ParticipantRepository.upsert(
                    db, threadId: threadId, identity: identity,
                    type: "human", role: "creator", joinedAt: timestamp)
                try ParticipantRepository.upsert(
                    db, threadId: threadId, identity: address,
                    type: "agent", role: "member", joinedAt: timestamp)
            }
        } catch {
            return nil
        }
```

(`identity`, `address`, `timestamp`, `threadId` are all already in scope in this function.)

- [ ] **Step 4: Run the test; expect PASS.** Then build the full app target to confirm no break:
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build
```
Revert any pbxproj rewrite.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/App/AppState.swift \
  ios/AINMMessenger/AINMMessengerTests/PersistenceTests/ParticipantPopulationTests.swift
rtk proxy git commit -m "feat(roster): seed user+agent participants at startThread"
```

---

### Task PRP-4: Upsert sender on live delivery

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/Networking/AINMClient.swift` (store `connectedIdentity` ~line 147; upsert in `handleMessageDelivery` ~line 691)
- Test: extend `ParticipantPopulationTests.swift`

- [ ] **Step 1: Write the failing test** — append to `ParticipantPopulationTests.swift`. This drives the participant write through the same `appDatabase` that `handleMessageDelivery` uses. Because `handleMessageDelivery` is `private`, the test exercises the public delivery entry indirectly is not feasible; instead assert the **isolated participant-write helper** that PRP-4 extracts. Implement delivery to call a testable instance method `recordParticipant(from:in:)` — and test THAT:

```swift
    func test_delivery_recordsSenderParticipant_agentViaSavedAgent() async throws {
        let db = try AppDatabase.makeInMemory()
        // Seed a thread + a saved agent so classification resolves to "agent".
        try await db.dbPool.write { dbx in
            try ThreadRecord(id: "tA", title: nil, createdAt: "2026-06-08T09:00:00.000Z",
                createdBy: "me@localhost", parentThreadId: nil, parentMessageId: nil,
                agentAddress: nil).save(dbx)
            try SavedAgentRecord(address: "researcher@acme.test", displayName: "Researcher",
                cardJson: "{}", addedAt: "2026-06-08T09:00:00.000Z").save(dbx)
        }
        try await AINMClient.recordParticipant(
            in: db, threadId: "tA", from: "researcher@acme.test",
            timestamp: "2026-06-08T10:00:00.000Z", contentSource: nil,
            currentUserIdentity: "me@localhost", threadCreatedBy: "me@localhost", agentAddress: nil)

        let p = try db.dbPool.read { dbx in
            try ParticipantRecord.fetchOne(dbx, key: ["threadId": "tA", "identity": "researcher@acme.test"]) }
        XCTAssertEqual(p?.type, "agent")
        XCTAssertEqual(p?.role, "member")
    }

    func test_delivery_selfSenderIsHumanCreator() async throws {
        let db = try AppDatabase.makeInMemory()
        try await db.dbPool.write { dbx in
            try ThreadRecord(id: "tB", title: nil, createdAt: "2026-06-08T09:00:00.000Z",
                createdBy: "me@localhost", parentThreadId: nil, parentMessageId: nil).save(dbx)
        }
        try await AINMClient.recordParticipant(
            in: db, threadId: "tB", from: "me@localhost",
            timestamp: "2026-06-08T10:00:00.000Z", contentSource: "user",
            currentUserIdentity: "me@localhost", threadCreatedBy: "me@localhost", agentAddress: nil)

        let p = try db.dbPool.read { dbx in
            try ParticipantRecord.fetchOne(dbx, key: ["threadId": "tB", "identity": "me@localhost"]) }
        XCTAssertEqual(p?.type, "human")
        XCTAssertEqual(p?.role, "creator")
    }
```

- [ ] **Step 2: Run it; expect FAIL** (`AINMClient.recordParticipant` undefined).

- [ ] **Step 3a: Store the connected identity.** In `AINMClient`, add a stored property near `lastDisplayName` (~line 42):

```swift
    private var lastDisplayName: String?
    /// The identity string formed at handshake (`displayName@host`); used to mark self on delivery.
    private(set) var connectedIdentity: String?
```

In `connect(...)`, immediately after `let identity = "\(displayName)@\(extractHost(from: serverURL))"` (~line 147), add:

```swift
        self.connectedIdentity = identity
```

- [ ] **Step 3b: Add the testable static helper** to `AINMClient` (classifies via DB-queryable signals + passed context, then upserts in its own write so it can never roll back a message save):

```swift
    /// Record/refresh the participant row for a delivered (or seeded) sender. Best-effort:
    /// classification uses the only client-side signals available. Runs its own write so a
    /// failure here can never regress message persistence.
    static func recordParticipant(
        in appDatabase: AppDatabase,
        threadId: String,
        from address: String,
        timestamp: String,
        contentSource: String?,
        currentUserIdentity: String?,
        threadCreatedBy: String?,
        agentAddress: String?
    ) async throws {
        try await appDatabase.dbPool.write { db in
            let savedAgentAddresses = Set(
                try String.fetchAll(db, sql: "SELECT address FROM savedAgent"))
            let type = ParticipantClassifier.classify(
                address: address,
                isSelf: address == currentUserIdentity,
                agentAddress: agentAddress,
                savedAgentAddresses: savedAgentAddresses,
                contentSource: contentSource)
            let role = (address == threadCreatedBy) ? "creator" : "member"
            try ParticipantRepository.upsert(
                db, threadId: threadId, identity: address,
                type: type, role: role, joinedAt: timestamp)
        }
    }
```

- [ ] **Step 3c: Call it from `handleMessageDelivery`.** After the existing message-write block (the `try await appDatabase.dbPool.write { ... }` that saves the record and sets `hasUnread`), add a best-effort participant write that cannot fail delivery:

```swift
            // Best-effort: record the sender in the thread roster. A failure here must never
            // drop the message, so it runs in its own write and swallows errors.
            let contentSource: String? = envelope.content.compactMap { block -> String? in
                if case .text(let c) = block { return c.source }
                return nil
            }.first
            let threadCreatedBy = try? await appDatabase.dbPool.read { db in
                try ThreadRecord.fetchOne(db, key: envelope.threadId)?.createdBy
            }
            let agentAddress = try? await appDatabase.dbPool.read { db in
                try ThreadRecord.fetchOne(db, key: envelope.threadId)?.agentAddress
            }
            try? await AINMClient.recordParticipant(
                in: appDatabase, threadId: envelope.threadId, from: envelope.from,
                timestamp: envelope.timestamp, contentSource: contentSource,
                currentUserIdentity: connectedIdentity,
                threadCreatedBy: threadCreatedBy ?? nil, agentAddress: agentAddress ?? nil)
```

Place this immediately AFTER the closing brace of the existing `do { try await appDatabase.dbPool.write { ... } }` message write but still inside the method's `do` block (so `envelope` is in scope). If the existing structure is a single `do { ... } catch { }`, add these lines at the end of the `do` body, after the write block.

- [ ] **Step 4: Run the tests; expect PASS (2 new).** Then full build:
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/ParticipantPopulationTests
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build
```
Revert any pbxproj rewrite.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Networking/AINMClient.swift \
  ios/AINMMessenger/AINMMessengerTests/PersistenceTests/ParticipantPopulationTests.swift
rtk proxy git commit -m "feat(roster): record sender participant on delivery (isolated, classified) + connectedIdentity"
```

---

### Task PRP-5: Lazy backfill on thread open

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadViewModel.swift` (`loadInitial`, ~lines 72-102)
- Test: extend `ParticipantPopulationTests.swift`

- [ ] **Step 1: Write the failing test** — append. Seeds a thread with messages but ZERO participant rows (pre-feature state), then asserts backfill + `isMultiParticipant`:

```swift
    func test_backfill_populatesRosterFromMessages_andFlipsMultiParticipant() async throws {
        let db = try AppDatabase.makeInMemory()
        try await db.dbPool.write { dbx in
            try ThreadRecord(id: "tC", title: nil, createdAt: "2026-06-08T09:00:00.000Z",
                createdBy: "me@localhost", parentThreadId: nil, parentMessageId: nil,
                agentAddress: "assistant@localhost").save(dbx)
            // Two distinct non-self senders → multi-participant once backfilled.
            for (i, from) in ["assistant@localhost", "researcher@acme.test"].enumerated() {
                try MessageRecord(
                    id: "m\(i)", threadId: "tC", fromAddress: from,
                    timestamp: "2026-06-08T10:0\(i):00.000Z", type: "message",
                    contentJson: "[]", signature: "s", serverTimestamp: nil).save(dbx)
            }
            try SavedAgentRecord(address: "researcher@acme.test", displayName: "Researcher",
                cardJson: "{}", addedAt: "2026-06-08T09:00:00.000Z").save(dbx)
        }
        let vm = MessageThreadViewModel(threadId: "tC", currentUserIdentity: "me@localhost", database: db)
        vm.savedAgentAddresses = ["assistant@localhost", "researcher@acme.test"]   // injected (see Step 3)
        await vm.loadInitial()

        let count = try db.dbPool.read { dbx in
            try ParticipantRecord.filter(Column("threadId") == "tC").fetchCount(dbx) }
        XCTAssertGreaterThanOrEqual(count, 2)           // assistant + researcher (+ maybe self)
        XCTAssertTrue(vm.isMultiParticipant)            // >1 non-current-user participant

        // Idempotent on re-open: no duplicates, no churn.
        await vm.loadInitial()
        let count2 = try db.dbPool.read { dbx in
            try ParticipantRecord.filter(Column("threadId") == "tC").fetchCount(dbx) }
        XCTAssertEqual(count, count2)
    }
```

- [ ] **Step 2: Run it; expect FAIL** (`vm.savedAgentAddresses` undefined and no backfill).

- [ ] **Step 3a: Add a savedAgent-addresses input to the view model.** In `MessageThreadViewModel`, add a stored property (near the other vars, after `var hasAgentParticipant`):

```swift
    /// Card-resolved agent addresses (from AppState.savedAgents), used to classify
    /// participants during lazy backfill. Set by the view before/at load.
    var savedAgentAddresses: Set<String> = []
```

In `MessageThreadView.onAppear`, where the VM is constructed (`let vm = MessageThreadViewModel(...)`), set it before `await vm.loadInitial()`:

```swift
                vm.streamingViewModel = appState.streamingViewModel
                vm.savedAgentAddresses = Set(appState.savedAgents.map(\.address))
                self.viewModel = vm
```

- [ ] **Step 3b: Add the backfill** in `loadInitial`, immediately after participants are loaded (after the `for p in parts { participants[p.identity] = p }` / `participantCount` / `hasAgentParticipant` block) and BEFORE `rebuildGroups()`:

```swift
        // Lazy backfill: a thread with no participant rows (pre-roster-feature data, or a
        // thread auto-created on delivery) gets its roster derived from the loaded messages
        // + the current user + the thread's agent address, classified with in-memory savedAgents.
        if parts.isEmpty {
            await backfillParticipantsFromMessages()
            // Reload so participants/participantCount reflect the backfill.
            let refreshed = (try? await repository.fetchParticipants(threadId: threadId)) ?? []
            participants.removeAll()
            for p in refreshed { participants[p.identity] = p }
            participantCount = refreshed.count
            hasAgentParticipant = refreshed.contains { $0.type == "agent" }
        }
```

Then add the private helper (anywhere in the class body):

```swift
    /// Derive + upsert participant rows from the loaded messages. Idempotent.
    private func backfillParticipantsFromMessages() async {
        // Thread metadata for role + agent classification.
        let thread = try? await threadRepository.dbPool.read { db in
            try ThreadRecord.fetchOne(db, key: self.threadId)
        }
        let createdBy = (thread ?? nil)?.createdBy
        let agentAddress = (thread ?? nil)?.agentAddress

        // Earliest message timestamp per identity (for joinedAt) + that message's content source.
        struct Seen { var joinedAt: String; var source: String? }
        var seen: [String: Seen] = [:]
        for m in allMessages {
            let src = contentBlocks(for: m).compactMap { block -> String? in
                if case .text(let c) = block { return c.source }; return nil
            }.first
            if let existing = seen[m.fromAddress] {
                if m.timestamp < existing.joinedAt { seen[m.fromAddress] = Seen(joinedAt: m.timestamp, source: src) }
            } else {
                seen[m.fromAddress] = Seen(joinedAt: m.timestamp, source: src)
            }
        }
        // Always include the current user (human/creator-if-creator) and the thread agent.
        let nowIsh = allMessages.first?.timestamp ?? thread??.createdAt ?? currentUserIdentity
        if seen[currentUserIdentity] == nil { seen[currentUserIdentity] = Seen(joinedAt: nowIsh, source: "user") }
        if let agentAddress, seen[agentAddress] == nil { seen[agentAddress] = Seen(joinedAt: nowIsh, source: "agent") }

        for (identity, info) in seen {
            let type = ParticipantClassifier.classify(
                address: identity,
                isSelf: identity == currentUserIdentity,
                agentAddress: agentAddress,
                savedAgentAddresses: savedAgentAddresses,
                contentSource: info.source)
            let role = (identity == createdBy) ? "creator" : "member"
            try? await ParticipantRepository(dbPool: database.dbPool).upsert(
                threadId: threadId, identity: identity, type: type, role: role, joinedAt: info.joinedAt)
        }
    }
```

> Implementation note for the engineer: `thread??.createdAt` reflects the double-optional from `try? ... fetchOne`. If the compiler flags the double-optional ergonomics, bind it cleanly first: `let thread = (try? await ...) ?? nil` so `thread` is a single `ThreadRecord?`, then use `thread?.createdBy` / `thread?.createdAt` / `thread?.agentAddress`. Use whichever form compiles; the behavior (nil-safe) is what matters. `database` is the stored `AppDatabase` on the view model; `threadRepository.dbPool` is the same pool.

- [ ] **Step 4: Run the test; expect PASS.** Then full build + the existing thread-VM/fixture suites to confirm no regression (fixture-seeded participants must not be disturbed — backfill only runs when `parts.isEmpty`):
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/ParticipantPopulationTests
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build
```
Revert any pbxproj rewrite.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadViewModel.swift \
  ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift \
  ios/AINMMessenger/AINMMessengerTests/PersistenceTests/ParticipantPopulationTests.swift
rtk proxy git commit -m "feat(roster): lazy backfill roster from messages on thread open; flips isMultiParticipant live"
```

---

### Task PRP-6: Full suites + DoD audit

**Files:** none (verification).

- [ ] **Step 1: Full unit suite**
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test
```
Expected: entire `AINMMessengerTests` suite green. If a pre-existing test that relies on `ScenarioFixture`-seeded participants fails, confirm it's because backfill ran when it shouldn't — backfill must only run when `parts.isEmpty`, so a fixture-seeded thread (non-empty) must be untouched. Fix the guard, not the test.

- [ ] **Step 2: UITest suite**
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessengerUITests \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test
```
Expected: green.

- [ ] **Step 3: Clean build**
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build
```
Expected: BUILD SUCCEEDED, no new warnings.

- [ ] **Step 4: DoD audit (report each PASS/FAIL with evidence)**
1. `ParticipantClassifier` pure + signal-matrix/precedence tests (PRP-1).
2. `ParticipantRepository.upsert` earliest/creator-sticky/agent-upgrade + no-downgrade tests (PRP-2).
3. `startThread` seeds user + agent in one write (PRP-3).
4. `handleMessageDelivery` upserts sender, isolated from message persistence (PRP-4).
5. `loadInitial` lazy-backfills empty-roster threads, idempotent on re-open (PRP-5).
6. `isMultiParticipant` true for user+2-agent thread, false for 1:1 (PRP-5 test + classifier).
7. Full unit + UITest green; clean build; committed to `main`.

- [ ] **Step 5: Commit (if any verification-only assertion changed)** — otherwise nothing to commit. Revert any pbxproj rewrite.

---

## Self-Review

**Spec coverage (each §8 DoD → task):** DoD 1→PRP-1; DoD 2→PRP-2; DoD 3→PRP-3; DoD 4→PRP-4; DoD 5→PRP-5; DoD 6→PRP-5 (`isMultiParticipant` integration test); DoD 7→PRP-6. Build order (§9: classifier → repository → seed → delivery → backfill → verify) = PRP-1…PRP-6 exactly. ✓

**Type consistency:**
- `ParticipantClassifier.classify(address:isSelf:agentAddress:savedAgentAddresses:contentSource:) -> String` — identical signature in PRP-1 (def), PRP-4 (`recordParticipant`), PRP-5 (backfill). `savedAgentAddresses` is `Set<String>` everywhere. ✓
- `ParticipantRepository.upsert(_ db:threadId:identity:type:role:joinedAt:)` static + async wrapper — same labels in PRP-2 (def), PRP-3 (seed, static in-`db`), PRP-4 (`recordParticipant`, static in its own write), PRP-5 (async wrapper). ✓
- `ParticipantRecord` memberwise init order `(threadId:identity:type:joinedAt:role:)` used consistently. ✓
- `AINMClient.recordParticipant(in:threadId:from:timestamp:contentSource:currentUserIdentity:threadCreatedBy:agentAddress:)` — same in PRP-4 def + tests. ✓
- `MessageThreadViewModel.savedAgentAddresses: Set<String>` set in `MessageThreadView` (PRP-5) and the PRP-5 test. ✓

**Placeholder scan:** none — every code step has complete code + exact commands. The two engineer-notes (AppState test access level in PRP-3; double-optional ergonomics in PRP-5) flag real Swift wrinkles with a concrete resolution rather than leaving them vague.

**Idempotency/regression guard:** backfill is gated on `parts.isEmpty`, so fixture-seeded and already-populated threads are never disturbed — called out in PRP-5 and PRP-6.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-08-participant-roster-population.md`. Per the established pattern, proceeding with **Subagent-Driven execution** (superpowers:subagent-driven-development): a fresh implementer subagent per task (PRP-1…PRP-6), build/test verification between tasks, commits to `main` via `rtk proxy git`. Pushing to origin remains a separate, user-gated step after all tasks complete.
