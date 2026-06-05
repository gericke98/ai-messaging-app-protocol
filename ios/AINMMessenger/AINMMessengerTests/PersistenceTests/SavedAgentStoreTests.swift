import XCTest
import GRDB
@testable import AINMMessenger

final class SavedAgentStoreTests: XCTestCase {
    func testRecordPersistsAndDecodesCard() throws {
        let db = try AppDatabase.makeInMemory()
        let cardJson = "{\"name\":\"Helper\",\"description\":\"d\",\"url\":\"\",\"version\":\"1.0\",\"protocolVersion\":\"0.1.0\",\"capabilities\":{\"streaming\":true,\"pushNotifications\":false},\"skills\":[]}"
        let record = SavedAgentRecord(
            address: "helper@localhost",
            displayName: "Helper",
            cardJson: cardJson,
            addedAt: "2026-06-04T00:00:00.000Z"
        )
        try db.dbPool.write { db in try record.save(db) }

        let fetched = try db.dbPool.read { db in
            try SavedAgentRecord.fetchOne(db, key: "helper@localhost")
        }
        XCTAssertEqual(fetched?.displayName, "Helper")
        let decoded = try XCTUnwrap(fetched?.agentCard())
        XCTAssertEqual(decoded.name, "Helper")
        XCTAssertEqual(decoded.protocolVersion, "0.1.0")
        XCTAssertFalse(decoded.capabilities.pushNotifications)
    }

    func testAddRemoveAllContains() async throws {
        let db = try AppDatabase.makeInMemory()
        let store = SavedAgentStore(dbPool: db.dbPool)
        let rec = SavedAgentRecord(address: "a@localhost", displayName: "A",
                                   cardJson: "{}", addedAt: "2026-06-04T00:00:00.000Z")
        try await store.add(rec)
        let isThere = try await store.contains(address: "a@localhost")
        XCTAssertTrue(isThere)
        let countAfterAdd = try await store.all().count
        XCTAssertEqual(countAfterAdd, 1)
        try await store.remove(address: "a@localhost")
        let isGone = try await store.contains(address: "a@localhost")
        XCTAssertFalse(isGone)
        let countAfterRemove = try await store.all().count
        XCTAssertEqual(countAfterRemove, 0)
    }

    func testAddIsUpsertNoDuplicate() async throws {
        let db = try AppDatabase.makeInMemory()
        let store = SavedAgentStore(dbPool: db.dbPool)
        try await store.add(SavedAgentRecord(address: "a@localhost", displayName: "A",
                                             cardJson: "{}", addedAt: "2026-06-04T00:00:00.000Z"))
        try await store.add(SavedAgentRecord(address: "a@localhost", displayName: "A2",
                                             cardJson: "{}", addedAt: "2026-06-04T00:00:01.000Z"))
        let all = try await store.all()
        XCTAssertEqual(all.count, 1)
        XCTAssertEqual(all.first?.displayName, "A2")
    }

    func testPerAccountIsolation() async throws {
        // Two distinct DBs model two accounts; a row in A is absent in B.
        let dbA = try AppDatabase.makeInMemory()
        let dbB = try AppDatabase.makeInMemory()
        let storeA = SavedAgentStore(dbPool: dbA.dbPool)
        let storeB = SavedAgentStore(dbPool: dbB.dbPool)
        try await storeA.add(SavedAgentRecord(address: "a@localhost", displayName: "A",
                                              cardJson: "{}", addedAt: "2026-06-04T00:00:00.000Z"))
        let inA = try await storeA.contains(address: "a@localhost")
        let inB = try await storeB.contains(address: "a@localhost")
        XCTAssertTrue(inA)
        XCTAssertFalse(inB)
    }

    @MainActor
    func testStartThreadStampsAgentAddress() async throws {
        let appState = AppState()
        try appState.setup()                       // makeDefault DB + client
        appState.connectedIdentity = "alice@localhost"
        let threadId = await appState.startThread(withAgent: "helper@localhost")
        let id = try XCTUnwrap(threadId)
        let db = try XCTUnwrap(appState.appDatabase)
        let fetched = try await db.dbPool.read { db in try ThreadRecord.fetchOne(db, key: id) }
        XCTAssertEqual(fetched?.agentAddress, "helper@localhost")
    }
}
