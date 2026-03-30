import XCTest
import GRDB
@testable import AINMMessenger

final class AppDatabaseTests: XCTestCase {
    var db: AppDatabase!

    override func setUp() async throws {
        db = try AppDatabase.makeInMemory()
    }

    // MARK: - Migration Tests

    func testMigrationsCreateTables() throws {
        try db.dbPool.read { db in
            XCTAssertTrue(try db.tableExists("thread"))
            XCTAssertTrue(try db.tableExists("message"))
            XCTAssertTrue(try db.tableExists("participant"))
        }
    }

    // MARK: - ThreadRecord CRUD

    func testInsertAndFetchThread() throws {
        let thread = ThreadRecord(
            id: "t-001",
            title: "Test Thread",
            createdAt: "2026-03-29T12:00:00.000Z",
            createdBy: "alice@example.com",
            parentThreadId: nil,
            parentMessageId: nil
        )
        try db.dbPool.write { db in
            try thread.insert(db)
        }
        let fetched = try db.dbPool.read { db in
            try ThreadRecord.fetchOne(db, key: "t-001")
        }
        XCTAssertNotNil(fetched)
        XCTAssertEqual(fetched?.id, "t-001")
        XCTAssertEqual(fetched?.title, "Test Thread")
        XCTAssertEqual(fetched?.createdAt, "2026-03-29T12:00:00.000Z")
        XCTAssertEqual(fetched?.createdBy, "alice@example.com")
        XCTAssertNil(fetched?.parentThreadId)
    }

    // MARK: - MessageRecord CRUD

    func testInsertAndFetchMessage() throws {
        // Insert thread first (FK constraint)
        let thread = ThreadRecord(
            id: "t-001",
            title: nil,
            createdAt: "2026-03-29T12:00:00.000Z",
            createdBy: "alice@example.com",
            parentThreadId: nil,
            parentMessageId: nil
        )
        let message = MessageRecord(
            id: "m-001",
            threadId: "t-001",
            fromAddress: "alice@example.com",
            timestamp: "2026-03-29T12:00:01.000Z",
            type: "message",
            contentJson: "[{\"type\":\"text\",\"text\":\"Hello\"}]",
            signature: "abc123",
            serverTimestamp: nil
        )
        try db.dbPool.write { db in
            try thread.insert(db)
            try message.insert(db)
        }
        let fetched = try db.dbPool.read { db in
            try MessageRecord.fetchAll(db, MessageRecord.messagesForThread("t-001"))
        }
        XCTAssertEqual(fetched.count, 1)
        XCTAssertEqual(fetched.first?.id, "m-001")
        XCTAssertEqual(fetched.first?.fromAddress, "alice@example.com")
    }

    // MARK: - ParticipantRecord CRUD

    func testInsertAndFetchParticipant() throws {
        let thread = ThreadRecord(
            id: "t-001",
            title: nil,
            createdAt: "2026-03-29T12:00:00.000Z",
            createdBy: "alice@example.com",
            parentThreadId: nil,
            parentMessageId: nil
        )
        let participant = ParticipantRecord(
            threadId: "t-001",
            identity: "alice@example.com",
            type: "human",
            joinedAt: "2026-03-29T12:00:00.000Z",
            role: "creator"
        )
        try db.dbPool.write { db in
            try thread.insert(db)
            try participant.insert(db)
        }
        let fetched = try db.dbPool.read { db in
            try ParticipantRecord.fetchOne(db, key: ["threadId": "t-001", "identity": "alice@example.com"])
        }
        XCTAssertNotNil(fetched)
        XCTAssertEqual(fetched?.identity, "alice@example.com")
        XCTAssertEqual(fetched?.type, "human")
        XCTAssertEqual(fetched?.role, "creator")
    }

    // MARK: - Foreign Key Constraint

    func testMessageForeignKeyConstraint() throws {
        let message = MessageRecord(
            id: "m-orphan",
            threadId: "nonexistent-thread",
            fromAddress: "alice@example.com",
            timestamp: "2026-03-29T12:00:01.000Z",
            type: "message",
            contentJson: "[]",
            signature: "abc123",
            serverTimestamp: nil
        )
        XCTAssertThrowsError(try db.dbPool.write { db in
            try message.insert(db)
        })
    }

    // MARK: - Cascade Delete

    func testCascadeDeleteThread() throws {
        let thread = ThreadRecord(
            id: "t-001",
            title: nil,
            createdAt: "2026-03-29T12:00:00.000Z",
            createdBy: "alice@example.com",
            parentThreadId: nil,
            parentMessageId: nil
        )
        let message = MessageRecord(
            id: "m-001",
            threadId: "t-001",
            fromAddress: "alice@example.com",
            timestamp: "2026-03-29T12:00:01.000Z",
            type: "message",
            contentJson: "[]",
            signature: "abc123",
            serverTimestamp: nil
        )
        let participant = ParticipantRecord(
            threadId: "t-001",
            identity: "alice@example.com",
            type: "human",
            joinedAt: "2026-03-29T12:00:00.000Z",
            role: "creator"
        )
        try db.dbPool.write { db in
            try thread.insert(db)
            try message.insert(db)
            try participant.insert(db)
        }
        // Delete the thread
        _ = try db.dbPool.write { db in
            try ThreadRecord.deleteOne(db, key: "t-001")
        }
        // Verify cascade
        let messages = try db.dbPool.read { db in
            try MessageRecord.fetchAll(db)
        }
        let participants = try db.dbPool.read { db in
            try ParticipantRecord.fetchAll(db)
        }
        XCTAssertTrue(messages.isEmpty)
        XCTAssertTrue(participants.isEmpty)
    }

    // MARK: - Ordering

    func testMessagesOrderedByTimestamp() throws {
        let thread = ThreadRecord(
            id: "t-001",
            title: nil,
            createdAt: "2026-03-29T12:00:00.000Z",
            createdBy: "alice@example.com",
            parentThreadId: nil,
            parentMessageId: nil
        )
        try db.dbPool.write { db in
            try thread.insert(db)
            // Insert out of order
            try MessageRecord(
                id: "m-003",
                threadId: "t-001",
                fromAddress: "alice@example.com",
                timestamp: "2026-03-29T12:00:03.000Z",
                type: "message",
                contentJson: "[]",
                signature: "sig3",
                serverTimestamp: nil
            ).insert(db)
            try MessageRecord(
                id: "m-001",
                threadId: "t-001",
                fromAddress: "bob@example.com",
                timestamp: "2026-03-29T12:00:01.000Z",
                type: "message",
                contentJson: "[]",
                signature: "sig1",
                serverTimestamp: nil
            ).insert(db)
            try MessageRecord(
                id: "m-002",
                threadId: "t-001",
                fromAddress: "alice@example.com",
                timestamp: "2026-03-29T12:00:02.000Z",
                type: "message",
                contentJson: "[]",
                signature: "sig2",
                serverTimestamp: nil
            ).insert(db)
        }
        let ordered = try db.dbPool.read { db in
            try MessageRecord.fetchAll(db, MessageRecord.messagesForThread("t-001"))
        }
        XCTAssertEqual(ordered.count, 3)
        XCTAssertEqual(ordered[0].id, "m-001")
        XCTAssertEqual(ordered[1].id, "m-002")
        XCTAssertEqual(ordered[2].id, "m-003")
    }

    // MARK: - MessageRecord from Envelope

    func testMessageRecordFromEnvelope() throws {
        let envelope = MessageEnvelope(
            id: "m-env-001",
            from: "alice@example.com",
            threadId: "t-001",
            timestamp: "2026-03-29T12:00:00.000Z",
            type: .message,
            content: [
                ContentBlock(type: "text", version: "1.0", source: "user", text: "Hello world", fallback: "Hello world")
            ],
            signature: "base64url-sig",
            parentId: nil
        )
        let record = try MessageRecord(from: envelope)
        XCTAssertEqual(record.id, "m-env-001")
        XCTAssertEqual(record.fromAddress, "alice@example.com")
        XCTAssertEqual(record.threadId, "t-001")
        XCTAssertEqual(record.type, "message")
        XCTAssertEqual(record.signature, "base64url-sig")
        XCTAssertTrue(record.contentJson.contains("Hello world"))
    }
}
