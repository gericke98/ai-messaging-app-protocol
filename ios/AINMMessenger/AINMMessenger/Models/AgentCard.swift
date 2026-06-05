import Foundation

/// Agent Card model per AINM protocol agent-card.md specification.
/// Compatible with A2A Agent Cards with AINM-specific extensions.
///
/// Keys are canonical camelCase (matching `schemas/agent-card.schema.json` and the
/// server's published cards). Swift's synthesized Codable maps property names directly —
/// no `CodingKeys` overrides. Unmodeled JSON keys (e.g. `ainm.address`, `ainm.type`) are
/// ignored on decode.
struct AgentCard: Codable, Sendable {
    let name: String
    let description: String
    let url: String
    let version: String
    let protocolVersion: String
    let capabilities: A2ACapabilities
    let skills: [Skill]
    let ainm: AINMExtension?

    struct A2ACapabilities: Codable, Sendable {
        let streaming: Bool
        let pushNotifications: Bool
    }

    struct Skill: Codable, Sendable, Identifiable {
        let id: String
        let name: String
        let description: String
        let tags: [String]?
    }

    struct AINMExtension: Codable, Sendable {
        let protocolVersion: String
        let contentTypes: [String]
        let streaming: StreamingCapability
        let threading: ThreadingCapability
        let memory: MemoryCapability
        let tools: [Tool]
    }

    struct StreamingCapability: Codable, Sendable {
        let supported: Bool
        let parallelBlocks: Bool
        let preferredChunkSize: String
    }

    struct ThreadingCapability: Codable, Sendable {
        let subThreads: Bool
        let maxConcurrentThreads: Int?
    }

    struct MemoryCapability: Codable, Sendable {
        let archivalSupported: Bool
        let contextFramePreferred: Bool
    }

    struct Tool: Codable, Sendable, Identifiable {
        var id: String { name }
        let name: String
        let description: String
        let inputSchema: AnyCodableValue
    }
}
