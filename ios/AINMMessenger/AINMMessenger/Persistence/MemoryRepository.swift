import GRDB
import Foundation

/// Repository for compound memory graph persistence.
/// Provides ValueObservation for live UI updates and bulk upsert for server sync.
struct MemoryRepository: Sendable {
    let dbPool: DatabasePool

    // MARK: - Observation

    /// Observe all current (non-superseded) memory nodes, optionally filtered by type/category.
    func observeNodes(
        typeFilter: String? = nil,
        categoryFilter: String? = nil
    ) -> ValueObservation<ValueReducers.Fetch<[MemoryNodeRecord]>> {
        ValueObservation.tracking { db in
            var sql = "SELECT * FROM memoryNode WHERE validUntil IS NULL"
            var arguments: [String] = []

            if let typeFilter {
                sql += " AND nodeType = ?"
                arguments.append(typeFilter)
            }
            if let categoryFilter {
                sql += " AND category = ?"
                arguments.append(categoryFilter)
            }
            sql += " ORDER BY confidence DESC, validFrom DESC"

            return try MemoryNodeRecord.fetchAll(
                db,
                sql: sql,
                arguments: StatementArguments(arguments)
            )
        }
    }

    /// Observe edges connecting a set of node IDs.
    func observeEdges(forNodeIds nodeIds: [String]) -> ValueObservation<ValueReducers.Fetch<[MemoryEdgeRecord]>> {
        ValueObservation.tracking { db in
            guard !nodeIds.isEmpty else { return [] }
            let placeholders = nodeIds.map { _ in "?" }.joined(separator: ", ")
            let sql = """
                SELECT * FROM memoryEdge
                WHERE validUntil IS NULL
                  AND (sourceNode IN (\(placeholders)) OR targetNode IN (\(placeholders)))
                ORDER BY confidence DESC
                """
            let args = nodeIds + nodeIds
            return try MemoryEdgeRecord.fetchAll(
                db,
                sql: sql,
                arguments: StatementArguments(args)
            )
        }
    }

    // MARK: - Fetch

    /// Fetch a single node by ID.
    func fetchNode(nodeId: String) async throws -> MemoryNodeRecord? {
        try await dbPool.read { db in
            try MemoryNodeRecord.fetchOne(db, key: ["nodeId": nodeId])
        }
    }

    /// Fetch all current nodes (non-superseded).
    func fetchCurrentNodes(limit: Int = 200) async throws -> [MemoryNodeRecord] {
        try await dbPool.read { db in
            try MemoryNodeRecord.fetchAll(
                db,
                sql: "SELECT * FROM memoryNode WHERE validUntil IS NULL ORDER BY confidence DESC LIMIT ?",
                arguments: [limit]
            )
        }
    }

    /// Fetch edges for a set of node IDs.
    func fetchEdges(forNodeIds nodeIds: [String]) async throws -> [MemoryEdgeRecord] {
        guard !nodeIds.isEmpty else { return [] }
        return try await dbPool.read { db in
            let placeholders = nodeIds.map { _ in "?" }.joined(separator: ", ")
            let sql = """
                SELECT * FROM memoryEdge
                WHERE validUntil IS NULL
                  AND (sourceNode IN (\(placeholders)) OR targetNode IN (\(placeholders)))
                """
            let args = nodeIds + nodeIds
            return try MemoryEdgeRecord.fetchAll(
                db,
                sql: sql,
                arguments: StatementArguments(args)
            )
        }
    }

    // MARK: - Sync

    /// Upsert nodes from server response. Replaces existing records by nodeId.
    func upsertNodes(_ nodes: [MemoryNode]) async throws {
        let records = nodes.map { MemoryNodeRecord(from: $0) }
        try await dbPool.write { db in
            for record in records {
                try record.save(db)
            }
        }
    }

    /// Upsert edges from server response. Replaces existing records by edgeId.
    func upsertEdges(_ edges: [MemoryEdge]) async throws {
        let records = edges.map { MemoryEdgeRecord(from: $0) }
        try await dbPool.write { db in
            for record in records {
                try record.save(db)
            }
        }
    }

    /// Delete a node and its connected edges from the local cache.
    func deleteNode(nodeId: String) async throws {
        try await dbPool.write { db in
            try db.execute(
                sql: "DELETE FROM memoryEdge WHERE sourceNode = ? OR targetNode = ?",
                arguments: [nodeId, nodeId]
            )
            try db.execute(
                sql: "DELETE FROM memoryNode WHERE nodeId = ?",
                arguments: [nodeId]
            )
        }
    }

    /// Replace entire local cache with a fresh graph snapshot.
    func replaceAll(nodes: [MemoryNode], edges: [MemoryEdge]) async throws {
        let nodeRecords = nodes.map { MemoryNodeRecord(from: $0) }
        let edgeRecords = edges.map { MemoryEdgeRecord(from: $0) }
        try await dbPool.write { db in
            try db.execute(sql: "DELETE FROM memoryEdge")
            try db.execute(sql: "DELETE FROM memoryNode")
            for record in nodeRecords {
                try record.save(db)
            }
            for record in edgeRecords {
                try record.save(db)
            }
        }
    }

    // MARK: - Stats

    /// Get node and edge counts for the local cache.
    func stats() async throws -> (nodes: Int, edges: Int) {
        try await dbPool.read { db in
            let nodeCount = try Int.fetchOne(
                db, sql: "SELECT COUNT(*) FROM memoryNode WHERE validUntil IS NULL"
            ) ?? 0
            let edgeCount = try Int.fetchOne(
                db, sql: "SELECT COUNT(*) FROM memoryEdge WHERE validUntil IS NULL"
            ) ?? 0
            return (nodeCount, edgeCount)
        }
    }
}
