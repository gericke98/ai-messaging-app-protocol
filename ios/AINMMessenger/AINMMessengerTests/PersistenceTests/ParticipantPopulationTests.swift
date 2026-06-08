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
}
