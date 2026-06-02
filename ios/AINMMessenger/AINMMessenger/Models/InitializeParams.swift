import Foundation

/// Parameters for the ainm.initialize request (client -> server).
/// Matches transport.schema.json InitializeParams.
struct InitializeParams: Codable, Sendable {
    let protocolVersion: String   // "0.1.0"
    let identity: String          // AINM address
    let timestamp: String         // RFC3339; server reconstructs "{identity}|{timestamp}" to verify the signature
    let signature: String         // base64url init signature
    let capabilities: ClientCapabilities

    enum CodingKeys: String, CodingKey {
        case protocolVersion = "protocolVersion"
        case identity
        case timestamp
        case signature
        case capabilities
    }
}

/// Client capabilities declared during ainm.initialize.
/// Only features declared by both client and server are available.
/// Matches transport.schema.json ClientCapabilities.
struct ClientCapabilities: Codable, Sendable {
    let streaming: Bool
    let subThreads: Bool
    let contentTypes: [String]
    let parallelStreaming: Bool
    let toolVisibility: Bool

    enum CodingKeys: String, CodingKey {
        case streaming
        case subThreads = "subThreads"
        case contentTypes = "contentTypes"
        case parallelStreaming = "parallelStreaming"
        case toolVisibility = "toolVisibility"
    }

    /// Default client capabilities. Updated for Phase 12: parallel streaming + interactive types.
    static let `default` = ClientCapabilities(
        streaming: true,
        subThreads: true,
        contentTypes: ["text", "code", "image", "file", "button", "form", "table", "approval", "action"],
        parallelStreaming: true,
        toolVisibility: false
    )
}
