import Foundation
import Observation
import GRDB

/// ViewModel for the compound memory graph screen.
/// Observes the local GRDB cache for real-time node/edge updates.
/// Groups nodes by type (semantic/episodic/procedural) and computes stats.
@MainActor
@Observable
final class MemoryGraphViewModel {
    var nodes: [MemoryNode] = []
    var edges: [MemoryEdge] = []
    var isLoading: Bool = false
    var searchText: String = ""
    var selectedTypeFilter: String?
    var selectedCategoryFilter: String?

    private var observationTask: Task<Void, Never>?
    private let repository: MemoryRepository

    init(repository: MemoryRepository) {
        self.repository = repository
    }

    // MARK: - Observation

    func startObserving() {
        observationTask = Task {
            isLoading = true
            let observation = repository.observeNodes(
                typeFilter: selectedTypeFilter,
                categoryFilter: selectedCategoryFilter
            )
            do {
                for try await records in observation.values(in: repository.dbPool) {
                    let allNodes = records.compactMap { $0.toMemoryNode() }
                    self.nodes = allNodes
                    self.isLoading = false
                    // Fetch edges for the loaded nodes
                    await self.loadEdges(for: allNodes)
                }
            } catch {
                self.isLoading = false
            }
        }
    }

    func stopObserving() {
        observationTask?.cancel()
        observationTask = nil
    }

    /// Restart observation when filters change.
    func applyFilters() {
        stopObserving()
        startObserving()
    }

    // MARK: - Grouping

    /// Nodes grouped by type for sectioned display.
    var nodesByType: [(type: String, nodes: [MemoryNode])] {
        let grouped = Dictionary(grouping: filteredNodes) { $0.type }
        let order = ["semantic", "episodic", "procedural"]
        return order.compactMap { type in
            guard let nodes = grouped[type], !nodes.isEmpty else { return nil }
            return (type: type, nodes: nodes)
        } + grouped.filter { !order.contains($0.key) }.map { (type: $0.key, nodes: $0.value) }
    }

    /// All unique categories present in the current nodes.
    var availableCategories: [String] {
        Array(Set(nodes.map(\.category))).sorted()
    }

    /// All unique types present in the current nodes.
    var availableTypes: [String] {
        Array(Set(nodes.map(\.type))).sorted()
    }

    // MARK: - Stats

    var nodeCount: Int { nodes.count }
    var edgeCount: Int { edges.count }

    var statsByType: [(type: String, count: Int)] {
        let grouped = Dictionary(grouping: nodes) { $0.type }
        return grouped.map { (type: $0.key, count: $0.value.count) }
            .sorted { $0.count > $1.count }
    }

    // MARK: - Search

    /// Nodes filtered by search text (label match).
    var filteredNodes: [MemoryNode] {
        guard !searchText.isEmpty else { return nodes }
        let query = searchText.lowercased()
        return nodes.filter { $0.label.lowercased().contains(query) }
    }

    /// Edges connecting the currently visible nodes.
    var filteredEdges: [MemoryEdge] {
        let visibleIds = Set(filteredNodes.map(\.nodeId))
        return edges.filter { visibleIds.contains($0.sourceNode) || visibleIds.contains($0.targetNode) }
    }

    // MARK: - Node Detail

    /// Get edges connected to a specific node.
    func edges(for nodeId: String) -> [MemoryEdge] {
        edges.filter { $0.sourceNode == nodeId || $0.targetNode == nodeId }
    }

    /// Get the connected node on the other end of an edge, relative to a given node.
    func connectedNode(edge: MemoryEdge, from nodeId: String) -> MemoryNode? {
        let targetId = edge.sourceNode == nodeId ? edge.targetNode : edge.sourceNode
        return nodes.first { $0.nodeId == targetId }
    }

    // MARK: - Private

    private func loadEdges(for nodes: [MemoryNode]) async {
        let nodeIds = nodes.map(\.nodeId)
        guard !nodeIds.isEmpty else {
            edges = []
            return
        }
        do {
            let records = try await repository.fetchEdges(forNodeIds: nodeIds)
            edges = records.compactMap { $0.toMemoryEdge() }
        } catch {
            // Non-fatal
        }
    }
}
