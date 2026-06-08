import XCTest
import GRDB
@testable import AINMMessenger

@MainActor
final class ParticipantPopulationTests: XCTestCase {

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

    func test_delivery_recordsSenderParticipant_agentViaSavedAgent() async throws {
        let db = try AppDatabase.makeInMemory()
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

        let p = try await db.dbPool.read { dbx in
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

        let p = try await db.dbPool.read { dbx in
            try ParticipantRecord.fetchOne(dbx, key: ["threadId": "tB", "identity": "me@localhost"]) }
        XCTAssertEqual(p?.type, "human")
        XCTAssertEqual(p?.role, "creator")
    }

    func test_backfill_populatesRosterFromMessages_andFlipsMultiParticipant() async throws {
        let db = try AppDatabase.makeInMemory()
        try await db.dbPool.write { dbx in
            try ThreadRecord(id: "tC", title: nil, createdAt: "2026-06-08T09:00:00.000Z",
                createdBy: "me@localhost", parentThreadId: nil, parentMessageId: nil,
                agentAddress: "assistant@localhost").save(dbx)
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
        vm.savedAgentAddresses = ["assistant@localhost", "researcher@acme.test"]
        await vm.loadInitial()

        let count = try await db.dbPool.read { dbx in
            try ParticipantRecord.filter(Column("threadId") == "tC").fetchCount(dbx) }
        XCTAssertGreaterThanOrEqual(count, 2)
        XCTAssertTrue(vm.isMultiParticipant)

        await vm.loadInitial()
        let count2 = try await db.dbPool.read { dbx in
            try ParticipantRecord.filter(Column("threadId") == "tC").fetchCount(dbx) }
        XCTAssertEqual(count, count2)   // idempotent: no dupes on re-open
    }
}
