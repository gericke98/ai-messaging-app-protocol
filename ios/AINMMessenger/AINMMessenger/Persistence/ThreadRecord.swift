import GRDB
import Foundation

/// GRDB record type for persisted threads.
/// Maps to the "thread" table created in AppDatabase migrations.
struct ThreadRecord: Codable, Sendable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "thread"

    let id: String
    var title: String?
    let createdAt: String
    let createdBy: String
    var parentThreadId: String?
    var parentMessageId: String?
    var hasUnread: Bool = false
    var agentAddress: String? = nil   // target agent for this thread (e.g. assistant@host); nil = human thread

    /// Create from protocol ThreadMetadata.
    init(from metadata: ThreadMetadata) {
        self.id = metadata.id
        self.title = metadata.title
        self.createdAt = metadata.createdAt
        self.createdBy = metadata.createdBy
        self.parentThreadId = metadata.parentThreadId
        self.parentMessageId = metadata.parentMessageId
        self.hasUnread = false
        self.agentAddress = nil
    }

    /// Memberwise initializer for direct construction.
    init(
        id: String,
        title: String?,
        createdAt: String,
        createdBy: String,
        parentThreadId: String?,
        parentMessageId: String?,
        hasUnread: Bool = false,
        agentAddress: String? = nil
    ) {
        self.id = id
        self.title = title
        self.createdAt = createdAt
        self.createdBy = createdBy
        self.parentThreadId = parentThreadId
        self.parentMessageId = parentMessageId
        self.hasUnread = hasUnread
        self.agentAddress = agentAddress
    }
}

// MARK: - Associations

extension ThreadRecord {
    static let messages = hasMany(MessageRecord.self, using: MessageRecord.threadForeignKey)
    static let participants = hasMany(ParticipantRecord.self, using: ParticipantRecord.threadForeignKey)
}
