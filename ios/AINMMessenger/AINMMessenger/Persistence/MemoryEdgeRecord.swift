import GRDB
import Foundation

/// GRDB record for locally cached compound memory edges.
/// Maps to the `memoryEdge` SQLite table for offline graph traversal.
struct MemoryEdgeRecord: Codable, Sendable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "memoryEdge"

    let edgeId: String          // UUID string
    let sourceNode: String      // UUID string → memoryNode.nodeId
    let targetNode: String      // UUID string → memoryNode.nodeId
    let relationship: String    // related_to, contradicts, supports, etc.
    let weight: Float?
    let confidence: Float
    let provenanceJson: String? // JSON string
    let validFrom: String       // RFC 3339 timestamp
    let validUntil: String?     // RFC 3339 timestamp
    let propertiesJson: String? // JSON string

    /// Create from a protocol MemoryEdge.
    init(from edge: MemoryEdge) {
        self.edgeId = edge.edgeId
        self.sourceNode = edge.sourceNode
        self.targetNode = edge.targetNode
        self.relationship = edge.relationship
        self.weight = edge.weight
        self.confidence = edge.confidence
        self.validFrom = edge.temporal.validFrom
        self.validUntil = edge.temporal.validUntil

        if let source = edge.source {
            self.provenanceJson = try? String(data: JSONEncoder().encode(source), encoding: .utf8)
        } else {
            self.provenanceJson = nil
        }
        if let props = edge.properties {
            self.propertiesJson = try? String(data: JSONEncoder().encode(props), encoding: .utf8)
        } else {
            self.propertiesJson = nil
        }
    }

    /// Convert back to a protocol MemoryEdge.
    func toMemoryEdge() -> MemoryEdge? {
        let source: AnyCodableValue? = provenanceJson.flatMap { json in
            guard let data = json.data(using: .utf8) else { return nil }
            return try? JSONDecoder().decode(AnyCodableValue.self, from: data)
        }
        let properties: AnyCodableValue? = propertiesJson.flatMap { json in
            guard let data = json.data(using: .utf8) else { return nil }
            return try? JSONDecoder().decode(AnyCodableValue.self, from: data)
        }

        return MemoryEdge(
            edgeId: edgeId,
            sourceNode: sourceNode,
            targetNode: targetNode,
            relationship: relationship,
            weight: weight,
            confidence: confidence,
            source: source,
            temporal: TemporalValidity(
                validFrom: validFrom,
                validUntil: validUntil,
                supersededBy: nil
            ),
            properties: properties
        )
    }
}
