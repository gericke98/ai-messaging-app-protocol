import GRDB
import Foundation

/// Repository for message-level GRDB queries.
/// Provides paginated message fetching and new-message observation for a specific thread.
struct MessageRepository: Sendable {
    let dbPool: DatabasePool
    private let pageSize = 50

    /// Fetch a page of messages for a thread, ordered by timestamp descending.
    /// Returns messages in chronological order (oldest first).
    func fetchMessages(threadId: String, olderThan: String? = nil) async throws -> [MessageRecord] {
        try await dbPool.read { db in
            var sql = "SELECT * FROM message WHERE threadId = ? AND type = 'message'"
            var arguments: [any DatabaseValueConvertible] = [threadId]
            if let olderThan {
                sql += " AND timestamp < ?"
                arguments.append(olderThan)
            }
            sql += " ORDER BY timestamp DESC LIMIT ?"
            arguments.append(pageSize)
            return try MessageRecord.fetchAll(
                db,
                sql: sql,
                arguments: StatementArguments(arguments)
            )
        }.reversed()
    }

    /// Observe new messages arriving for a specific thread (messages newer than a given timestamp).
    func observeNewMessages(threadId: String, newerThan: String) -> ValueObservation<ValueReducers.Fetch<[MessageRecord]>> {
        ValueObservation.tracking { db in
            try MessageRecord
                .filter(Column("threadId") == threadId)
                .filter(Column("type") == "message")
                .filter(Column("timestamp") > newerThan)
                .order(Column("timestamp").asc)
                .fetchAll(db)
        }
    }

    /// Fetch participants for a thread.
    func fetchParticipants(threadId: String) async throws -> [ParticipantRecord] {
        try await dbPool.read { db in
            try ParticipantRecord
                .filter(Column("threadId") == threadId)
                .fetchAll(db)
        }
    }
}
