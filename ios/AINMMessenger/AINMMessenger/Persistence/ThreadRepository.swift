import GRDB
import Foundation

/// Data transfer object for conversation list rows.
/// Joins thread data with the latest message for preview display.
struct ConversationRow: Codable, Sendable, FetchableRecord {
    let id: String
    var title: String?
    let createdAt: String
    let createdBy: String
    var hasUnread: Bool
    var lastMessageTimestamp: String?
    var lastMessageContent: String?
    var lastMessageFrom: String?
}

/// Repository for thread-level GRDB queries.
/// Provides conversation list observation and unread state management.
struct ThreadRepository: Sendable {
    let dbPool: DatabasePool

    /// Returns a ValueObservation of conversation rows sorted by most recent message.
    /// The observation emits a new value whenever the thread or message tables change.
    func observeConversations() -> ValueObservation<ValueReducers.Fetch<[ConversationRow]>> {
        ValueObservation.tracking { db in
            try Row.fetchAll(db, sql: """
                SELECT t.*,
                       m.timestamp AS lastMessageTimestamp,
                       m.contentJson AS lastMessageContent,
                       m.fromAddress AS lastMessageFrom
                FROM thread t
                LEFT JOIN message m ON m.id = (
                    SELECT m2.id FROM message m2
                    WHERE m2.threadId = t.id AND m2.type = 'message'
                    ORDER BY m2.timestamp DESC LIMIT 1
                )
                ORDER BY COALESCE(m.timestamp, t.createdAt) DESC
                """)
            .map { row in
                ConversationRow(
                    id: row["id"],
                    title: row["title"],
                    createdAt: row["createdAt"],
                    createdBy: row["createdBy"],
                    hasUnread: row["hasUnread"],
                    lastMessageTimestamp: row["lastMessageTimestamp"],
                    lastMessageContent: row["lastMessageContent"],
                    lastMessageFrom: row["lastMessageFrom"]
                )
            }
        }
    }

    /// Mark a thread as read.
    func markAsRead(threadId: String) async throws {
        try await dbPool.write { db in
            try db.execute(
                sql: "UPDATE thread SET hasUnread = 0 WHERE id = ?",
                arguments: [threadId]
            )
        }
    }

    /// Mark a thread as unread.
    func markAsUnread(threadId: String) async throws {
        try await dbPool.write { db in
            try db.execute(
                sql: "UPDATE thread SET hasUnread = 1 WHERE id = ?",
                arguments: [threadId]
            )
        }
    }
}
