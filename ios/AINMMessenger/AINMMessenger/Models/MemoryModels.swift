import Foundation

/// Context frame pushed by the server when agent memory state changes.
/// Contains active context window, conversation summary, archival facts, and participants.
struct ContextFrame: Codable, Sendable {
    let threadId: String
    let frame: Frame

    struct Frame: Codable, Sendable {
        let activeContext: ActiveContext
        let conversationSummary: String?
        let archivalFacts: [ArchivalFact]
        let participants: [FrameParticipant]

        enum CodingKeys: String, CodingKey {
            case activeContext = "active_context"
            case conversationSummary = "conversation_summary"
            case archivalFacts = "archival_facts"
            case participants
        }
    }

    struct ActiveContext: Codable, Sendable {
        let recentMessages: [MessageSummary]
        let windowStart: String
        let messageCount: Int

        enum CodingKeys: String, CodingKey {
            case recentMessages = "recent_messages"
            case windowStart = "window_start"
            case messageCount = "message_count"
        }
    }

    struct MessageSummary: Codable, Sendable, Identifiable {
        let id: String
        let from: String
        let timestamp: String
        let summary: String
    }

    struct ArchivalFact: Codable, Sendable, Identifiable {
        let factId: String
        let key: String
        let value: AnyCodableValue
        let tags: [String]?
        let storedBy: String
        let storedAt: String

        var id: String { factId }

        enum CodingKeys: String, CodingKey {
            case factId = "fact_id"
            case key, value, tags
            case storedBy = "stored_by"
            case storedAt = "stored_at"
        }
    }

    struct FrameParticipant: Codable, Sendable {
        let identity: String
        let type: String
        let presence: String
    }

    enum CodingKeys: String, CodingKey {
        case threadId = "thread_id"
        case frame
    }
}

/// Response from ainm.memory.query
struct MemoryQueryResult: Codable, Sendable {
    let facts: [ContextFrame.ArchivalFact]
}
