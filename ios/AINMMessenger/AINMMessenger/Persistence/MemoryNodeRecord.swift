import GRDB
import Foundation

/// GRDB record for locally cached compound memory nodes.
/// Maps to the `memoryNode` SQLite table for offline access and live UI observation.
struct MemoryNodeRecord: Codable, Sendable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "memoryNode"

    let nodeId: String          // UUID string
    let nodeType: String        // semantic, episodic, procedural
    let category: String        // preference, fact, skill, event, etc.
    let label: String
    let propertiesJson: String? // JSON string
    let confidence: Float
    let provenanceJson: String? // JSON string (source/provenance)
    let validFrom: String       // RFC 3339 timestamp
    let validUntil: String?     // RFC 3339 timestamp
    let supersededBy: String?   // UUID string
    let tagsJson: String?       // JSON string

    /// Create from a protocol MemoryNode.
    init(from node: MemoryNode) {
        self.nodeId = node.nodeId
        self.nodeType = node.type
        self.category = node.category
        self.label = node.label
        self.confidence = node.confidence
        self.validFrom = node.temporal.validFrom
        self.validUntil = node.temporal.validUntil
        self.supersededBy = node.temporal.supersededBy

        if let props = node.properties {
            self.propertiesJson = try? String(data: JSONEncoder().encode(props), encoding: .utf8)
        } else {
            self.propertiesJson = nil
        }
        if let source = node.source {
            self.provenanceJson = try? String(data: JSONEncoder().encode(source), encoding: .utf8)
        } else {
            self.provenanceJson = nil
        }
        if let tags = node.tags {
            self.tagsJson = try? String(data: JSONEncoder().encode(tags), encoding: .utf8)
        } else {
            self.tagsJson = nil
        }
    }

    /// Convert back to a protocol MemoryNode.
    func toMemoryNode() -> MemoryNode? {
        let properties: AnyCodableValue? = propertiesJson.flatMap { json in
            guard let data = json.data(using: .utf8) else { return nil }
            return try? JSONDecoder().decode(AnyCodableValue.self, from: data)
        }
        let source: AnyCodableValue? = provenanceJson.flatMap { json in
            guard let data = json.data(using: .utf8) else { return nil }
            return try? JSONDecoder().decode(AnyCodableValue.self, from: data)
        }
        let tags: AnyCodableValue? = tagsJson.flatMap { json in
            guard let data = json.data(using: .utf8) else { return nil }
            return try? JSONDecoder().decode(AnyCodableValue.self, from: data)
        }

        return MemoryNode(
            nodeId: nodeId,
            type: nodeType,
            category: category,
            label: label,
            properties: properties,
            confidence: confidence,
            source: source,
            temporal: TemporalValidity(
                validFrom: validFrom,
                validUntil: validUntil,
                supersededBy: supersededBy
            ),
            tags: tags
        )
    }
}
