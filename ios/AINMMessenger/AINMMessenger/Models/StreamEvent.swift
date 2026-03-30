import Foundation

// MARK: - Stream Event Models
// Codable models matching spec/streaming.md JSON-RPC notification params.
// Used by StreamingManager and AINMClient to process streaming lifecycle events.

/// Parameters for `ainm.stream.start` notification.
/// Declares the block layout for an incoming streaming response.
struct StreamStartParams: Codable, Sendable {
    let streamId: String
    let messageId: String
    let from: String
    let threadId: String
    let timestamp: String
    let blocks: [BlockLayout]

    enum CodingKeys: String, CodingKey {
        case streamId = "stream_id"
        case messageId = "message_id"
        case from
        case threadId = "thread_id"
        case timestamp
        case blocks
    }

    /// Declares type/version/source for each block position in the stream.
    struct BlockLayout: Codable, Sendable {
        let type: String
        let version: String
        let source: String
    }
}

/// Parameters for `ainm.stream.delta` notification.
/// Carries an incremental text delta for a specific block index.
struct StreamDeltaParams: Codable, Sendable {
    let streamId: String
    let blockIndex: Int
    let delta: String
    let language: String?

    enum CodingKeys: String, CodingKey {
        case streamId = "stream_id"
        case blockIndex = "block_index"
        case delta
        case language
    }
}

/// Parameters for `ainm.stream.end` notification.
/// Signals that streaming for this stream has finished.
struct StreamEndParams: Codable, Sendable {
    let streamId: String
    let status: String  // "completed" or "cancelled"

    enum CodingKeys: String, CodingKey {
        case streamId = "stream_id"
        case status
    }
}

/// Parameters for `ainm.stream.error` notification.
/// Signals a server-side streaming error.
struct StreamErrorParams: Codable, Sendable {
    let streamId: String
    let code: Int
    let message: String

    enum CodingKeys: String, CodingKey {
        case streamId = "stream_id"
        case code
        case message
    }
}

/// Parameters for `ainm.presence.notify` notification.
/// Indicates participant typing/processing status in a thread.
struct PresenceNotifyParams: Codable, Sendable {
    let identity: String
    let status: String
    let threadId: String?
    let detail: String?

    enum CodingKeys: String, CodingKey {
        case identity
        case status
        case threadId = "thread_id"
        case detail
    }
}
