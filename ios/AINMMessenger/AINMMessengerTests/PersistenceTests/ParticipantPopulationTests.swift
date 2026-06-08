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
}
