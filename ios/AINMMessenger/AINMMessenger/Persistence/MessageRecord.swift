import GRDB
import Foundation

/// GRDB record type for persisted messages.
/// Maps to the "message" table created in AppDatabase migrations.
/// Stores content blocks as a JSON string to avoid a separate table in Phase 10.
struct MessageRecord: Codable, Sendable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "message"
    static let threadForeignKey = ForeignKey(["threadId"], to: ["id"])

    let id: String
    let threadId: String
    let fromAddress: String
    let timestamp: String
    let type: String           // "message", "thread.create", etc.
    let contentJson: String    // JSON-encoded [ContentBlock]
    let signature: String      // base64url Ed25519 signature
    var serverTimestamp: String?

    /// Create from protocol MessageEnvelope.
    /// Encodes the content array to JSON for storage.
    init(from envelope: MessageEnvelope) throws {
        self.id = envelope.id
        self.threadId = envelope.threadId
        self.fromAddress = envelope.from
        self.timestamp = envelope.timestamp
        self.type = envelope.type.rawValue
        let jsonData = try JSONEncoder().encode(envelope.content)
        self.contentJson = String(data: jsonData, encoding: .utf8)!
        self.signature = envelope.signature ?? ""
        self.serverTimestamp = nil
    }

    /// Memberwise initializer for direct construction.
    init(
        id: String,
        threadId: String,
        fromAddress: String,
        timestamp: String,
        type: String,
        contentJson: String,
        signature: String,
        serverTimestamp: String?
    ) {
        self.id = id
        self.threadId = threadId
        self.fromAddress = fromAddress
        self.timestamp = timestamp
        self.type = type
        self.contentJson = contentJson
        self.signature = signature
        self.serverTimestamp = serverTimestamp
    }
}

// MARK: - Query Helpers

extension MessageRecord {
    /// Fetch messages for a thread, ordered by timestamp ascending.
    static func messagesForThread(_ threadId: String) -> QueryInterfaceRequest<MessageRecord> {
        filter(Column("threadId") == threadId)
            .order(Column("timestamp").asc)
    }
}
