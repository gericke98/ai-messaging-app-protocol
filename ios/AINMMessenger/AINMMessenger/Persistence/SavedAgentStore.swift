import GRDB
import Foundation

/// CRUD over the active account's saved-agent contacts. A thin actor around the
/// account's `DatabasePool` (mirrors the access style of `AgentCardService`).
actor SavedAgentStore {
    private let dbPool: DatabasePool

    init(dbPool: DatabasePool) {
        self.dbPool = dbPool
    }

    /// Insert or update (upsert on the `address` primary key) a saved agent.
    func add(_ record: SavedAgentRecord) async throws {
        try await dbPool.write { db in try record.save(db) }
    }

    /// Remove a saved agent by address. No-op if absent.
    func remove(address: String) async throws {
        _ = try await dbPool.write { db in
            try SavedAgentRecord.deleteOne(db, key: address)
        }
    }

    /// All saved agents, oldest first (stable list order).
    func all() async throws -> [SavedAgentRecord] {
        try await dbPool.read { db in
            try SavedAgentRecord.order(Column("addedAt").asc).fetchAll(db)
        }
    }

    /// Whether an agent with this address is already saved.
    func contains(address: String) async throws -> Bool {
        try await dbPool.read { db in
            try SavedAgentRecord.fetchOne(db, key: address) != nil
        }
    }
}
