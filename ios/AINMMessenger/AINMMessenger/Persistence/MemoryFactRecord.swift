import GRDB
import Foundation

/// GRDB record for locally cached archival memory facts.
/// Persisted to SQLite so facts are available offline and can be searched locally.
struct MemoryFactRecord: Codable, Sendable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "memoryFact"

    let factId: String
    let key: String
    let value: String       // JSON string
    let tags: String?       // JSON array string
    let storedBy: String
    let storedAt: String

    /// Create from ArchivalFact.
    init(from fact: ContextFrame.ArchivalFact) {
        self.factId = fact.factId
        self.key = fact.key
        self.value = (try? String(data: JSONEncoder().encode(fact.value), encoding: .utf8)) ?? ""
        if let tags = fact.tags {
            self.tags = (try? String(data: JSONEncoder().encode(tags), encoding: .utf8))
        } else {
            self.tags = nil
        }
        self.storedBy = fact.storedBy
        self.storedAt = fact.storedAt
    }

    /// Memberwise.
    init(factId: String, key: String, value: String, tags: String?, storedBy: String, storedAt: String) {
        self.factId = factId
        self.key = key
        self.value = value
        self.tags = tags
        self.storedBy = storedBy
        self.storedAt = storedAt
    }

    /// Decode tags from JSON string.
    func decodedTags() -> [String]? {
        guard let tags, let data = tags.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode([String].self, from: data)
    }
}
