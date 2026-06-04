import XCTest
import GRDB
@testable import AINMMessenger

final class SavedAgentStoreTests: XCTestCase {
    func testRecordPersistsAndDecodesCard() throws {
        let db = try AppDatabase.makeInMemory()
        let cardJson = "{\"name\":\"Helper\",\"description\":\"d\",\"url\":\"\",\"version\":\"1.0\",\"protocol_version\":\"0.1.0\",\"capabilities\":{\"streaming\":true,\"push_notifications\":false},\"skills\":[]}"
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
        XCTAssertEqual(fetched?.agentCard()?.name, "Helper")
    }
}
