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
        let count = try await db.dbPool.read { dbx in
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
