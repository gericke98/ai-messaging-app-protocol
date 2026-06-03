import XCTest
import GRDB
@testable import AINMMessenger

final class ThreadAgentAddressTests: XCTestCase {
    var db: AppDatabase!

    override func setUp() async throws {
        db = try AppDatabase.makeInMemory()
    }

    func testAgentAddressPersists() throws {
        let thread = ThreadRecord(
            id: "t-agent",
            title: "Agent Thread",
            createdAt: "2026-06-02T00:00:00.000Z",
            createdBy: "alice@localhost",
            parentThreadId: nil,
            parentMessageId: nil,
            agentAddress: "assistant@localhost"
        )
        try db.dbPool.write { db in
            try thread.insert(db)
        }
        let fetched = try db.dbPool.read { db in
            try ThreadRecord.fetchOne(db, key: "t-agent")
        }
        XCTAssertNotNil(fetched)
        XCTAssertEqual(fetched?.agentAddress, "assistant@localhost")
    }

    func testAgentAddressDefaultsToNil() throws {
        let thread = ThreadRecord(
            id: "t-human",
            title: "Human Thread",
            createdAt: "2026-06-02T00:00:00.000Z",
            createdBy: "alice@localhost",
            parentThreadId: nil,
            parentMessageId: nil
        )
        try db.dbPool.write { db in
            try thread.insert(db)
        }
        let fetched = try db.dbPool.read { db in
            try ThreadRecord.fetchOne(db, key: "t-human")
        }
        XCTAssertNotNil(fetched)
        XCTAssertNil(fetched?.agentAddress)
    }
}
