import GRDB
import Foundation

/// GRDB record for persisting tool call state across app sessions.
/// Maps to the "toolCall" table created in the v3 migration.
struct ToolCallRecord: Codable, Sendable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "toolCall"
    static let threadForeignKey = ForeignKey(["threadId"], to: ["id"])

    let callId: String
    let threadId: String
    let name: String
    var status: String       // "running", "completed", "error", "timedOut"
    var input: String?       // JSON string of tool input parameters
    var resultSummary: String?
    var durationMs: Int?
    var errorMessage: String?
    let startedAt: String    // RFC 3339 timestamp
    var completedAt: String? // RFC 3339 timestamp
}

// MARK: - Query Helpers

extension ToolCallRecord {
    /// Fetch tool calls for a thread, ordered by startedAt ascending.
    static func toolCallsForThread(_ threadId: String) -> QueryInterfaceRequest<ToolCallRecord> {
        filter(Column("threadId") == threadId)
            .order(Column("startedAt").asc)
    }
}
