import GRDB
import Foundation

/// GRDB record type for persisted thread participants.
/// Maps to the "participant" table created in AppDatabase migrations.
/// Uses a composite primary key of (threadId, identity).
struct ParticipantRecord: Codable, Sendable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "participant"
    static let threadForeignKey = ForeignKey(["threadId"], to: ["id"])

    let threadId: String
    let identity: String
    let type: String         // "human" or "agent"
    let joinedAt: String
    let role: String         // "creator" or "member"

    /// Create from protocol Participant model with associated thread ID.
    init(from participant: Participant, threadId: String) {
        self.threadId = threadId
        self.identity = participant.identity
        self.type = participant.type.rawValue
        self.joinedAt = participant.joinedAt
        self.role = participant.role.rawValue
    }

    /// Memberwise initializer for direct construction.
    init(
        threadId: String,
        identity: String,
        type: String,
        joinedAt: String,
        role: String
    ) {
        self.threadId = threadId
        self.identity = identity
        self.type = type
        self.joinedAt = joinedAt
        self.role = role
    }
}
