import XCTest
import GRDB
@testable import AINMMessenger

final class MessageToAddressTests: XCTestCase {
    var db: AppDatabase!

    override func setUp() async throws {
        db = try AppDatabase.makeInMemory()
        // Seed the thread row required by the FK on the message table.
        let thread = ThreadRecord(
            id: "t1", title: nil,
            createdAt: "2026-06-08T09:00:00.000Z", createdBy: "alice@localhost",
            parentThreadId: nil, parentMessageId: nil
        )
        try await db.dbPool.write { db in try thread.insert(db) }
    }

    /// init(from envelope:) copies the envelope `to` field.
    func testInitFromEnvelopeCopiesTo() throws {
        let env = MessageEnvelope(
            id: "m1", from: "assistant@localhost", threadId: "t1",
            timestamp: "2026-06-08T09:41:00.000Z", type: .message,
            content: [.text(.init(version: "1.0", source: "agent", text: "hi", mentions: nil, fallback: "hi"))],
            signature: "sig", parentId: nil, to: "researcher@acme.test"
        )
        let record = try MessageRecord(from: env)
        XCTAssertEqual(record.toAddress, "researcher@acme.test")
    }

    /// A record with a toAddress round-trips through the DB.
    func testToAddressRoundTrips() throws {
        let record = MessageRecord(
            id: "m2", threadId: "t1", fromAddress: "assistant@localhost",
            timestamp: "2026-06-08T09:41:00.000Z", type: "message",
            contentJson: "[]", signature: "sig", serverTimestamp: nil,
            toAddress: "researcher@acme.test"
        )
        try db.dbPool.write { db in try record.insert(db) }
        let fetched = try db.dbPool.read { db in try MessageRecord.fetchOne(db, key: "m2") }
        XCTAssertEqual(fetched?.toAddress, "researcher@acme.test")
    }

    /// A record without a toAddress reads back nil (existing-row behavior).
    func testMissingToAddressReadsNil() throws {
        let record = MessageRecord(
            id: "m3", threadId: "t1", fromAddress: "alice@localhost",
            timestamp: "2026-06-08T09:42:00.000Z", type: "message",
            contentJson: "[]", signature: "sig", serverTimestamp: nil
        )
        try db.dbPool.write { db in try record.insert(db) }
        let fetched = try db.dbPool.read { db in try MessageRecord.fetchOne(db, key: "m3") }
        XCTAssertNil(fetched?.toAddress)
    }
}
