import GRDB
import Foundation

/// GRDB record for a per-account saved agent contact.
/// Maps to the "savedAgent" table created in the v8 migration. The user's chosen
/// agent contacts live in the ACTIVE account's database (multi-account isolation gives
/// each account its own DB file). Distinct from the ETag-keyed `agentCard` HTTP cache:
/// a saved agent is a deliberate contact whose card snapshot lets the list render offline.
struct SavedAgentRecord: Codable, Sendable, FetchableRecord, PersistableRecord, Identifiable {
    static let databaseTableName = "savedAgent"

    let address: String        // Primary key -- canonical name@host
    let displayName: String    // From the card's `name`, for list display
    let cardJson: String       // Serialized AgentCard snapshot (renders offline)
    let addedAt: String        // RFC 3339

    var id: String { address }

    /// Decode the cached AgentCard snapshot from JSON, if it is well-formed.
    func agentCard() -> AgentCard? {
        guard let data = cardJson.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(AgentCard.self, from: data)
    }
}
