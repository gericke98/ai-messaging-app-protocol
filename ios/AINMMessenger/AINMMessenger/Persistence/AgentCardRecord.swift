import GRDB
import Foundation

/// GRDB record for cached Agent Cards with ETag-based conditional refresh.
/// Maps to the "agentCard" table created in v4 migration.
struct AgentCardRecord: Codable, Sendable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "agentCard"

    let url: String          // Primary key -- agent card URL
    let identity: String     // AINM address
    let cardJson: String     // Full JSON
    var etag: String?        // HTTP ETag
    let fetchedAt: String    // RFC 3339

    /// Decode the cached AgentCard from JSON.
    func agentCard() -> AgentCard? {
        guard let data = cardJson.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(AgentCard.self, from: data)
    }
}
