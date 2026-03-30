import Foundation

/// Response to ainm.initialize (server -> client).
/// Matches transport.schema.json InitializeResult.
struct InitializeResult: Codable, Sendable {
    let protocolVersion: String
    let serverInfo: ServerInfo
    let capabilities: ServerCapabilities
    let nonce: String

    enum CodingKeys: String, CodingKey {
        case protocolVersion = "protocolVersion"
        case serverInfo = "serverInfo"
        case capabilities
        case nonce
    }
}

/// Server identification returned during initialization.
/// Matches transport.schema.json ServerInfo.
struct ServerInfo: Codable, Sendable {
    let name: String
    let version: String
}

/// Server capabilities returned during ainm.initialize.
/// Matches transport.schema.json ServerCapabilities.
struct ServerCapabilities: Codable, Sendable {
    let streaming: Bool
    let subThreads: Bool
    let storeAndForward: Bool
    let maxContentBlocks: Int
    let maxMessageSize: Int

    enum CodingKeys: String, CodingKey {
        case streaming
        case subThreads = "subThreads"
        case storeAndForward = "storeAndForward"
        case maxContentBlocks = "maxContentBlocks"
        case maxMessageSize = "maxMessageSize"
    }
}
