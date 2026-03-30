import Foundation

/// Discriminated union for AINM content blocks matching content.schema.json.
/// Supports text, code, image, file, and unknown (fallback) block types.
/// Per content spec Section 5, unknown types decode gracefully using the fallback field.
enum ContentBlock: Codable, Sendable {
    case text(TextContent)
    case code(CodeContent)
    case image(ImageContent)
    case file(FileContent)
    case unknown(fallback: String)

    // MARK: - Inner Types

    struct TextContent: Codable, Sendable {
        let version: String
        let source: String
        let text: String
        let mentions: [Mention]?
        let fallback: String
    }

    struct CodeContent: Codable, Sendable {
        let version: String
        let source: String
        let language: String
        let code: String
        let filename: String?
        let fallback: String
    }

    struct ImageContent: Codable, Sendable {
        let version: String
        let source: String
        let url: String
        let mimeType: String
        let alt: String
        let width: Int?
        let height: Int?
        let sizeBytes: Int?
        let fallback: String

        enum CodingKeys: String, CodingKey {
            case version, source, url, alt, width, height, fallback
            case mimeType = "mime_type"
            case sizeBytes = "size_bytes"
        }
    }

    struct FileContent: Codable, Sendable {
        let version: String
        let source: String
        let url: String
        let mimeType: String
        let filename: String
        let sizeBytes: Int
        let digest: String
        let fallback: String

        enum CodingKeys: String, CodingKey {
            case version, source, url, filename, digest, fallback
            case mimeType = "mime_type"
            case sizeBytes = "size_bytes"
        }
    }

    struct Mention: Codable, Sendable {
        let address: String
        let offset: Int
        let length: Int
    }

    // MARK: - Computed Properties

    /// Returns the fallback text for any content block case.
    var fallbackText: String {
        switch self {
        case .text(let content): return content.fallback
        case .code(let content): return content.fallback
        case .image(let content): return content.fallback
        case .file(let content): return content.fallback
        case .unknown(let fallback): return fallback
        }
    }

    // MARK: - Codable

    private enum TypeDiscriminator: String, Codable {
        case text
        case code
        case image
        case file
    }

    private enum CodingKeys: String, CodingKey {
        case type
        case fallback
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let typeString = try container.decode(String.self, forKey: .type)

        switch typeString {
        case "text":
            let content = try TextContent(from: decoder)
            self = .text(content)
        case "code":
            let content = try CodeContent(from: decoder)
            self = .code(content)
        case "image":
            let content = try ImageContent(from: decoder)
            self = .image(content)
        case "file":
            let content = try FileContent(from: decoder)
            self = .file(content)
        default:
            let fallback = (try? container.decode(String.self, forKey: .fallback))
                ?? "[Unsupported content block]"
            self = .unknown(fallback: fallback)
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)

        switch self {
        case .text(let content):
            try container.encode("text", forKey: .type)
            try content.encode(to: encoder)
        case .code(let content):
            try container.encode("code", forKey: .type)
            try content.encode(to: encoder)
        case .image(let content):
            try container.encode("image", forKey: .type)
            try content.encode(to: encoder)
        case .file(let content):
            try container.encode("file", forKey: .type)
            try content.encode(to: encoder)
        case .unknown(let fallback):
            try container.encode("unknown", forKey: .type)
            try container.encode(fallback, forKey: .fallback)
        }
    }
}
