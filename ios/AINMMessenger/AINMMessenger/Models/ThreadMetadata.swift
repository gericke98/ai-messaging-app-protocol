import Foundation

/// Thread metadata structure from threading.schema.json.
/// Every thread carries this data.
struct ThreadMetadata: Codable, Sendable {
    let id: String
    let title: String?
    let createdAt: String
    let createdBy: String
    let parentThreadId: String?
    let parentMessageId: String?

    enum CodingKeys: String, CodingKey {
        case id = "thread_id"
        case title
        case createdAt = "created_at"
        case createdBy = "created_by"
        case parentThreadId = "parent_thread_id"
        case parentMessageId = "parent_message_id"
    }
}
