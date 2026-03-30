import Foundation
import GRDB

/// Fetches and caches Agent Cards via HTTP with ETag-based conditional requests.
actor AgentCardService {
    private let appDatabase: AppDatabase
    private let session = URLSession.shared

    init(appDatabase: AppDatabase) {
        self.appDatabase = appDatabase
    }

    /// Fetch Agent Card from URL with ETag-based conditional request.
    /// Returns cached version if server returns 304 Not Modified.
    func fetchCard(agentCardURL: String, identity: String) async throws -> AgentCard {
        guard let url = URL(string: agentCardURL) else {
            throw AgentCardError.invalidURL
        }

        // Check cache for ETag
        let cachedRecord = try await appDatabase.dbPool.read { db in
            try AgentCardRecord.fetchOne(db, key: agentCardURL)
        }

        var request = URLRequest(url: url)
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        if let etag = cachedRecord?.etag {
            request.setValue(etag, forHTTPHeaderField: "If-None-Match")
        }

        let (data, response) = try await session.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw AgentCardError.invalidResponse
        }

        if httpResponse.statusCode == 304, let cached = cachedRecord?.agentCard() {
            return cached
        }

        guard httpResponse.statusCode == 200 else {
            throw AgentCardError.httpError(httpResponse.statusCode)
        }

        let card = try JSONDecoder().decode(AgentCard.self, from: data)
        let etag = httpResponse.value(forHTTPHeaderField: "ETag")
        let record = AgentCardRecord(
            url: agentCardURL,
            identity: identity,
            cardJson: String(data: data, encoding: .utf8) ?? "",
            etag: etag,
            fetchedAt: ISO8601DateFormatter.ainm.string(from: Date())
        )
        try await appDatabase.dbPool.write { db in
            try record.save(db)
        }
        return card
    }

    /// Load all cached agent cards from GRDB.
    func loadCachedCards() async throws -> [(identity: String, card: AgentCard)] {
        let records = try await appDatabase.dbPool.read { db in
            try AgentCardRecord.fetchAll(db)
        }
        return records.compactMap { record in
            guard let card = record.agentCard() else { return nil }
            return (identity: record.identity, card: card)
        }
    }

    /// Fetch Agent Card by AINM address using identity resolution.
    /// For v1: constructs the well-known URL from the address domain.
    func fetchCardByAddress(_ address: String) async throws -> AgentCard {
        // Extract domain from address (e.g., "agent@server.com" -> "server.com")
        let parts = address.components(separatedBy: "@")
        guard parts.count == 2, let domain = parts.last, !domain.isEmpty else {
            throw AgentCardError.invalidAddress
        }
        let localPart = parts[0]
        // Well-known URL pattern: https://{domain}/.well-known/ainm/agents/{local}/agent-card.json
        let cardURL = "https://\(domain)/.well-known/ainm/agents/\(localPart)/agent-card.json"
        return try await fetchCard(agentCardURL: cardURL, identity: address)
    }

    enum AgentCardError: Error, LocalizedError {
        case invalidURL
        case invalidResponse
        case invalidAddress
        case httpError(Int)

        var errorDescription: String? {
            switch self {
            case .invalidURL: return "Invalid agent card URL"
            case .invalidResponse: return "Invalid server response"
            case .invalidAddress: return "Invalid AINM address format"
            case .httpError(let code): return "Server returned HTTP \(code)"
            }
        }
    }
}
