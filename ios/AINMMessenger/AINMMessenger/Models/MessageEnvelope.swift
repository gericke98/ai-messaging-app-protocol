import Foundation

/// Signed message envelope matching security.schema.json and common.schema.json MessageEnvelope.
/// The fundamental unit of communication in AINM. Every message uses this envelope
/// structure and is cryptographically signed per security.md Section 4.
struct MessageEnvelope: Codable, Sendable {
    let id: String           // UUIDv7 string
    let from: String         // AINM address e.g. "alice@server.com"
    let threadId: String     // UUIDv7 string
    let timestamp: String    // RFC 3339 with milliseconds + Z
    let type: MessageType    // "message", "thread.create", etc.
    let content: [ContentBlock]
    var signature: String?   // base64url Ed25519 sig, nil before signing
    let parentId: String?    // Optional parent message ID for sub-threading

    enum MessageType: String, Codable, Sendable {
        case message
        case threadCreate = "thread.create"
        case threadJoin = "thread.join"
        case threadLeave = "thread.leave"
        case presenceUpdate = "presence.update"
        case action
        case toolCallStarted = "tool.call.started"
        case toolCallProgress = "tool.call.progress"
        case toolCallCompleted = "tool.call.completed"
    }

    enum CodingKeys: String, CodingKey {
        case id
        case from
        case threadId = "thread_id"
        case timestamp
        case type
        case content
        case signature
        case parentId = "parent_id"
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(id, forKey: .id)
        try container.encode(from, forKey: .from)
        try container.encode(threadId, forKey: .threadId)
        try container.encode(timestamp, forKey: .timestamp)
        try container.encode(type, forKey: .type)
        try container.encode(content, forKey: .content)
        // Only encode signature when present (matches Rust pattern)
        if let signature {
            try container.encode(signature, forKey: .signature)
        }
        if let parentId {
            try container.encode(parentId, forKey: .parentId)
        }
    }
}
