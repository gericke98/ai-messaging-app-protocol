import Foundation

/// Agent Card model per AINM protocol agent-card.md specification.
/// Compatible with A2A Agent Cards with AINM-specific extensions.
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
        enum CodingKeys: String, CodingKey {
            case streaming
            case pushNotifications = "push_notifications"
        }
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

        enum CodingKeys: String, CodingKey {
            case protocolVersion = "protocol_version"
            case contentTypes = "content_types"
            case streaming, threading, memory, tools
        }
    }

    struct StreamingCapability: Codable, Sendable {
        let supported: Bool
        let parallelBlocks: Bool
        let preferredChunkSize: String
        enum CodingKeys: String, CodingKey {
            case supported
            case parallelBlocks = "parallel_blocks"
            case preferredChunkSize = "preferred_chunk_size"
        }
    }

    struct ThreadingCapability: Codable, Sendable {
        let subThreads: Bool
        let maxConcurrentThreads: Int?
        enum CodingKeys: String, CodingKey {
            case subThreads = "sub_threads"
            case maxConcurrentThreads = "max_concurrent_threads"
        }
    }

    struct MemoryCapability: Codable, Sendable {
        let archivalSupported: Bool
        let contextFramePreferred: Bool
        enum CodingKeys: String, CodingKey {
            case archivalSupported = "archival_supported"
            case contextFramePreferred = "context_frame_preferred"
        }
    }

    struct Tool: Codable, Sendable, Identifiable {
        var id: String { name }
        let name: String
        let description: String
        let inputSchema: AnyCodableValue
        enum CodingKeys: String, CodingKey {
            case name, description
            case inputSchema = "input_schema"
        }
    }

    enum CodingKeys: String, CodingKey {
        case name, description, url, version, capabilities, skills, ainm
        case protocolVersion = "protocol_version"
    }
}
