import Foundation

/// Discriminated union for AINM content blocks matching content.schema.json.
/// Supports text, code, image, file, button, form, table, approval, action, and unknown (fallback) block types.
/// Per content spec Section 5, unknown types decode gracefully using the fallback field.
enum ContentBlock: Codable, Sendable {
    case text(TextContent)
    case code(CodeContent)
    case image(ImageContent)
    case file(FileContent)
    case button(ButtonContent)
    case form(FormContent)
    case table(TableContent)
    case approval(ApprovalContent)
    case action(ActionContent)
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

    // MARK: - Interactive Block Types

    struct ButtonContent: Codable, Sendable {
        let version: String
        let source: String
        let label: String
        let actionId: String
        let style: String?
        let disabled: Bool?
        let confirmation: String?
        let fallback: String

        enum CodingKeys: String, CodingKey {
            case version, source, label, style, disabled, confirmation, fallback
            case actionId = "action_id"
        }
    }

    struct FormContent: Codable, Sendable {
        let version: String
        let source: String
        let title: String?
        let description: String?
        let fields: [FormField]
        let submitLabel: String?
        let fallback: String

        enum CodingKeys: String, CodingKey {
            case version, source, title, description, fields, fallback
            case submitLabel = "submit_label"
        }
    }

    struct FormField: Codable, Sendable, Identifiable {
        let id: String
        let type: String // text, textarea, number, select, multiselect, checkbox, date, datetime
        let label: String
        let description: String?
        let required: Bool?
        let defaultValue: AnyCodableValue?
        let placeholder: String?
        let validation: String?
        let options: [FormOption]?

        enum CodingKeys: String, CodingKey {
            case id, type, label, description, required, placeholder, validation, options
            case defaultValue = "default"
        }
    }

    struct FormOption: Codable, Sendable, Identifiable {
        var id: String { value }
        let value: String
        let label: String

        enum CodingKeys: String, CodingKey {
            case value, label
        }
    }

    struct TableContent: Codable, Sendable {
        let version: String
        let source: String
        let title: String?
        let columns: [TableColumn]
        let rows: [[AnyCodableValue]]
        let actions: [TableAction]?
        let sortable: Bool?
        let fallback: String
    }

    struct TableColumn: Codable, Sendable {
        let key: String
        let label: String
        let align: String?
        let sortable: Bool?
    }

    struct TableAction: Codable, Sendable {
        let actionId: String
        let label: String
        let style: String?

        enum CodingKeys: String, CodingKey {
            case label, style
            case actionId = "action_id"
        }
    }

    struct ApprovalContent: Codable, Sendable {
        let version: String
        let source: String
        let actionId: String
        let title: String
        let description: String?
        let status: String // pending, approved, rejected, changes_requested
        let options: [ApprovalOption]
        let fallback: String

        enum CodingKeys: String, CodingKey {
            case version, source, title, description, status, options, fallback
            case actionId = "action_id"
        }
    }

    struct ApprovalOption: Codable, Sendable, Identifiable {
        var id: String { value }
        let value: String
        let label: String
        let style: String?

        enum CodingKeys: String, CodingKey {
            case value, label, style
        }
    }

    struct ActionContent: Codable, Sendable {
        let version: String
        let source: String
        let refMessageId: String
        let refBlockIndex: Int
        let actionId: String
        let data: AnyCodableValue
        let fallback: String

        enum CodingKeys: String, CodingKey {
            case version, source, data, fallback
            case refMessageId = "ref_message_id"
            case refBlockIndex = "ref_block_index"
            case actionId = "action_id"
        }
    }

    // MARK: - Computed Properties

    /// Returns the fallback text for any content block case.
    var fallbackText: String {
        switch self {
        case .text(let content): return content.fallback
        case .code(let content): return content.fallback
        case .image(let content): return content.fallback
        case .file(let content): return content.fallback
        case .button(let content): return content.fallback
        case .form(let content): return content.fallback
        case .table(let content): return content.fallback
        case .approval(let content): return content.fallback
        case .action(let content): return content.fallback
        case .unknown(let fallback): return fallback
        }
    }

    // MARK: - Codable

    private enum TypeDiscriminator: String, Codable {
        case text
        case code
        case image
        case file
        case button
        case form
        case table
        case approval
        case action
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
        case "button":
            let content = try ButtonContent(from: decoder)
            self = .button(content)
        case "form":
            let content = try FormContent(from: decoder)
            self = .form(content)
        case "table":
            let content = try TableContent(from: decoder)
            self = .table(content)
        case "approval":
            let content = try ApprovalContent(from: decoder)
            self = .approval(content)
        case "action":
            let content = try ActionContent(from: decoder)
            self = .action(content)
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
        case .button(let content):
            try container.encode("button", forKey: .type)
            try content.encode(to: encoder)
        case .form(let content):
            try container.encode("form", forKey: .type)
            try content.encode(to: encoder)
        case .table(let content):
            try container.encode("table", forKey: .type)
            try content.encode(to: encoder)
        case .approval(let content):
            try container.encode("approval", forKey: .type)
            try content.encode(to: encoder)
        case .action(let content):
            try container.encode("action", forKey: .type)
            try content.encode(to: encoder)
        case .unknown(let fallback):
            try container.encode("unknown", forKey: .type)
            try container.encode(fallback, forKey: .fallback)
        }
    }
}
