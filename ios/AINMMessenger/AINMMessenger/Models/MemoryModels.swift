import Foundation

/// Context frame pushed by the server when agent memory state changes.
/// Contains active context window, conversation summary, archival facts, and participants.
struct ContextFrame: Codable, Sendable {
    let threadId: String
    let frame: Frame

    struct Frame: Codable, Sendable {
        let activeContext: ActiveContext
        let conversationSummary: String?
        let archivalFacts: [ArchivalFact]
        let participants: [FrameParticipant]

        enum CodingKeys: String, CodingKey {
            case activeContext = "active_context"
            case conversationSummary = "conversation_summary"
            case archivalFacts = "archival_facts"
            case participants
        }
    }

    struct ActiveContext: Codable, Sendable {
        let recentMessages: [MessageSummary]
        let windowStart: String
        let messageCount: Int

        enum CodingKeys: String, CodingKey {
            case recentMessages = "recent_messages"
            case windowStart = "window_start"
            case messageCount = "message_count"
        }
    }

    struct MessageSummary: Codable, Sendable, Identifiable {
        let id: String
        let from: String
        let timestamp: String
        let summary: String
    }

    struct ArchivalFact: Codable, Sendable, Identifiable {
        let factId: String
        let key: String
        let value: AnyCodableValue
        let tags: [String]?
        let storedBy: String
        let storedAt: String

        var id: String { factId }

        enum CodingKeys: String, CodingKey {
            case factId = "fact_id"
            case key, value, tags
            case storedBy = "stored_by"
            case storedAt = "stored_at"
        }
    }

    struct FrameParticipant: Codable, Sendable {
        let identity: String
        let type: String
        let presence: String
    }

    enum CodingKeys: String, CodingKey {
        case threadId = "thread_id"
        case frame
    }
}

/// Response from ainm.memory.query
struct MemoryQueryResult: Codable, Sendable {
    let facts: [ContextFrame.ArchivalFact]
}

// MARK: - Compound Memory Types

/// A node in the user's compound memory graph.
struct MemoryNode: Codable, Sendable, Identifiable {
    let nodeId: String
    let type: String          // semantic, episodic, procedural
    let category: String      // preference, fact, skill, event, decision, etc.
    let label: String
    let properties: AnyCodableValue?
    let confidence: Float
    let source: AnyCodableValue?  // provenance object (flexible shape)
    let temporal: TemporalValidity
    let tags: AnyCodableValue?

    var id: String { nodeId }

    enum CodingKeys: String, CodingKey {
        case nodeId = "node_id"
        case type, category, label, properties, confidence, source, temporal, tags
    }
}

/// An edge connecting two nodes in the memory graph.
struct MemoryEdge: Codable, Sendable, Identifiable {
    let edgeId: String
    let sourceNode: String
    let targetNode: String
    let relationship: String
    let weight: Float?
    let confidence: Float
    let source: AnyCodableValue?  // provenance
    let temporal: TemporalValidity
    let properties: AnyCodableValue?

    var id: String { edgeId }

    enum CodingKeys: String, CodingKey {
        case edgeId = "edge_id"
        case sourceNode = "source_node"
        case targetNode = "target_node"
        case relationship, weight, confidence, source, temporal, properties
    }
}

/// Temporal validity window for nodes and edges.
struct TemporalValidity: Codable, Sendable {
    let validFrom: String
    let validUntil: String?
    let supersededBy: String?

    enum CodingKeys: String, CodingKey {
        case validFrom = "valid_from"
        case validUntil = "valid_until"
        case supersededBy = "superseded_by"
    }
}

/// A subgraph snapshot containing nodes and edges.
struct GraphSnapshot: Codable, Sendable {
    let nodes: [MemoryNode]
    let edges: [MemoryEdge]
}

/// Response from ainm.memory.graph.query
struct GraphQueryResult: Codable, Sendable {
    let nodes: [MemoryNode]
    let edges: [MemoryEdge]
    let totalCount: Int
    let returnedCount: Int

    enum CodingKeys: String, CodingKey {
        case nodes, edges
        case totalCount = "total_count"
        case returnedCount = "returned_count"
    }
}

/// Response from ainm.memory.graph.traverse
struct GraphTraverseResult: Codable, Sendable {
    let subgraph: GraphSnapshot
    let traversalDepthReached: Int

    enum CodingKeys: String, CodingKey {
        case subgraph
        case traversalDepthReached = "traversal_depth_reached"
    }
}

/// Response from ainm.memory.graph.contribute
struct GraphContributeResult: Codable, Sendable {
    let contributed: Bool
    let nodeId: String
    let edgeIds: [String]

    enum CodingKeys: String, CodingKey {
        case contributed
        case nodeId = "node_id"
        case edgeIds = "edge_ids"
    }
}

/// Agent briefing delivered when an agent joins a thread.
struct AgentBriefing: Codable, Sendable {
    let briefingId: String
    let threadId: String
    let userIdentity: String
    let generatedAt: String
    let scope: String
    let consentLevel: String
    let sections: BriefingSections
    let graphSnapshot: GraphSnapshot?
    let tokenBudgetUsed: Int?

    enum CodingKeys: String, CodingKey {
        case briefingId = "briefing_id"
        case threadId = "thread_id"
        case userIdentity = "user_identity"
        case generatedAt = "generated_at"
        case scope
        case consentLevel = "consent_level"
        case sections
        case graphSnapshot = "graph_snapshot"
        case tokenBudgetUsed = "token_budget_used"
    }
}

/// Sections within an agent briefing, each optional.
struct BriefingSections: Codable, Sendable {
    let identity: [String: AnyCodableValue]?
    let preferences: [BriefingNodeSummary]?
    let expertise: [BriefingNodeSummary]?
    let activeGoals: [BriefingNodeSummary]?
    let interactionPatterns: [BriefingNodeSummary]?
    let relevantHistory: [BriefingNodeSummary]?

    enum CodingKeys: String, CodingKey {
        case identity, preferences, expertise
        case activeGoals = "active_goals"
        case interactionPatterns = "interaction_patterns"
        case relevantHistory = "relevant_history"
    }
}

/// Summary of a node within a briefing section.
struct BriefingNodeSummary: Codable, Sendable, Identifiable {
    let nodeId: String
    let label: String
    let confidence: Float
    let validFrom: String

    var id: String { nodeId }

    enum CodingKeys: String, CodingKey {
        case nodeId = "node_id"
        case label, confidence
        case validFrom = "valid_from"
    }
}

/// Response from ainm.memory.lint
struct LintResult: Codable, Sendable {
    let lintId: String
    let timestamp: String
    let findings: [LintFinding]
    let summary: LintSummary

    enum CodingKeys: String, CodingKey {
        case lintId = "lint_id"
        case timestamp, findings, summary
    }
}

/// A single lint finding (contradiction, orphan, stale node).
struct LintFinding: Codable, Sendable, Identifiable {
    let type: String
    let severity: String
    let description: String
    let nodeIds: [String]

    var id: String { "\(type)-\(nodeIds.first ?? UUID().uuidString)" }

    enum CodingKeys: String, CodingKey {
        case type, severity, description
        case nodeIds = "node_ids"
    }
}

/// Summary statistics from a lint pass.
struct LintSummary: Codable, Sendable {
    let totalNodes: Int
    let totalEdges: Int
    let contradictions: Int
    let orphanNodes: Int
    let staleNodes: Int

    enum CodingKeys: String, CodingKey {
        case totalNodes = "total_nodes"
        case totalEdges = "total_edges"
        case contradictions
        case orphanNodes = "orphan_nodes"
        case staleNodes = "stale_nodes"
    }
}

/// Response from ainm.memory.export (ainm-memory-v1 format).
struct MemoryExport: Codable, Sendable {
    let format: String
    let version: String
    let exportedAt: String
    let userIdentity: String
    let graph: GraphSnapshot
    let metadata: ExportMetadata

    enum CodingKeys: String, CodingKey {
        case format, version
        case exportedAt = "exported_at"
        case userIdentity = "user_identity"
        case graph, metadata
    }
}

/// Metadata about an export.
struct ExportMetadata: Codable, Sendable {
    let nodeCount: Int
    let edgeCount: Int
    let earliestNode: String?
    let latestNode: String?

    enum CodingKeys: String, CodingKey {
        case nodeCount = "node_count"
        case edgeCount = "edge_count"
        case earliestNode = "earliest_node"
        case latestNode = "latest_node"
    }
}

/// Response from ainm.memory.grant
struct MemoryGrantResult: Codable, Sendable {
    let granted: Bool
    let agentIdentity: String
    let effectiveScopes: [String]
    let effectiveConsentLevel: String

    enum CodingKeys: String, CodingKey {
        case granted
        case agentIdentity = "agent_identity"
        case effectiveScopes = "effective_scopes"
        case effectiveConsentLevel = "effective_consent_level"
    }
}

/// Response from ainm.memory.revoke
struct MemoryRevokeResult: Codable, Sendable {
    let revoked: Bool
    let agentIdentity: String

    enum CodingKeys: String, CodingKey {
        case revoked
        case agentIdentity = "agent_identity"
    }
}

/// Notification from ainm.memory.compile (server push).
struct CompilationNotification: Codable, Sendable {
    let threadId: String
    let nodesCreated: Int
    let edgesCreated: Int
    let nodesUpdated: Int

    enum CodingKeys: String, CodingKey {
        case threadId = "thread_id"
        case nodesCreated = "nodes_created"
        case edgesCreated = "edges_created"
        case nodesUpdated = "nodes_updated"
    }
}
