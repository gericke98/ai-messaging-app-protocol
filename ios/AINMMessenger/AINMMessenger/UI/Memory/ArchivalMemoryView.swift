import SwiftUI

/// Full-screen archival memory browser with search and delete (UI-SPEC Component 7).
/// Displays facts grouped by agent, supports search via ainm.memory.query
/// and swipe-to-delete via ainm.memory.evict.
struct ArchivalMemoryView: View {
    let appState: AppState

    @State private var facts: [ContextFrame.ArchivalFact] = []
    @State private var searchText = ""
    @State private var isSearching = false
    @State private var factToDelete: ContextFrame.ArchivalFact?
    @State private var showDeleteConfirmation = false

    /// Group facts by storedBy agent.
    private var groupedFacts: [(agent: String, facts: [ContextFrame.ArchivalFact])] {
        let grouped = Dictionary(grouping: facts) { $0.storedBy }
        return grouped.sorted { $0.key < $1.key }.map { (agent: $0.key, facts: $0.value) }
    }

    var body: some View {
        Group {
            if facts.isEmpty && !isSearching {
                emptyState
            } else {
                factsList
            }
        }
        .navigationTitle("Archival Memory")
        .searchable(text: $searchText, prompt: "Search facts...")
        .onSubmit(of: .search) {
            Task { await searchFacts() }
        }
        .onChange(of: searchText) { _, newValue in
            if newValue.isEmpty {
                loadCachedFacts()
            }
        }
        .alert("Delete Fact", isPresented: $showDeleteConfirmation, presenting: factToDelete) { fact in
            Button("Delete", role: .destructive) {
                Task { await deleteFact(fact) }
            }
            Button("Cancel", role: .cancel) {}
        } message: { _ in
            Text("This will permanently remove this fact from the agent's memory.")
        }
        .task {
            loadCachedFacts()
        }
    }

    // MARK: - Facts List

    @ViewBuilder
    private var factsList: some View {
        List {
            ForEach(groupedFacts, id: \.agent) { group in
                Section {
                    ForEach(group.facts) { fact in
                        factRow(fact)
                            .swipeActions(edge: .trailing) {
                                Button("Delete", role: .destructive) {
                                    factToDelete = fact
                                    showDeleteConfirmation = true
                                }
                            }
                    }
                } header: {
                    Text(group.agent.components(separatedBy: "@").first ?? group.agent)
                        .font(.subheadline.bold())
                        .foregroundStyle(Color(.secondaryLabel))
                }
            }
        }
        .listStyle(.plain)
        .overlay {
            if isSearching {
                ProgressView("Searching...")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .background(Color(.systemBackground).opacity(0.7))
            }
        }
    }

    // MARK: - Fact Row

    @ViewBuilder
    private func factRow(_ fact: ContextFrame.ArchivalFact) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(fact.key)
                .font(.subheadline.bold())
                .foregroundStyle(Color(.label))
            Text(displayValue(fact.value))
                .font(.body)
                .foregroundStyle(Color(.label))
                .lineLimit(3)
            if let tags = fact.tags, !tags.isEmpty {
                HStack(spacing: 4) {
                    ForEach(tags, id: \.self) { tag in
                        Text(tag)
                            .font(.caption)
                            .padding(.vertical, 2)
                            .padding(.horizontal, 6)
                            .background(Color(.tertiarySystemFill))
                            .clipShape(Capsule())
                    }
                }
            }
            Text("Stored by \(fact.storedBy.components(separatedBy: "@").first ?? fact.storedBy) on \(formatDate(fact.storedAt))")
                .font(.caption)
                .foregroundStyle(Color(.secondaryLabel))
        }
        .padding(.vertical, 12)
    }

    // MARK: - Empty State

    @ViewBuilder
    private var emptyState: some View {
        VStack(spacing: 12) {
            Image(systemName: "brain")
                .font(.system(size: 48))
                .foregroundStyle(Color(.tertiaryLabel))
            Text("No archival facts")
                .font(.title2.bold())
            Text("This agent hasn't stored any facts yet")
                .font(.subheadline)
                .foregroundStyle(Color(.secondaryLabel))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Actions

    private func loadCachedFacts() {
        // Load from the latest context frame's archival facts
        let allFacts = appState.contextFrames.values.flatMap { $0.frame.archivalFacts }
        facts = allFacts
    }

    private func searchFacts() async {
        guard !searchText.isEmpty else { return }
        isSearching = true
        defer { isSearching = false }
        do {
            let results = try await appState.queryMemory(query: searchText)
            facts = results
        } catch {
            // On error, keep current facts
        }
    }

    private func deleteFact(_ fact: ContextFrame.ArchivalFact) async {
        do {
            try await appState.evictMemoryFact(factId: fact.factId)
            withAnimation {
                facts.removeAll { $0.factId == fact.factId }
            }
        } catch {
            // On error, fact remains in list
        }
    }

    // MARK: - Helpers

    private func displayValue(_ value: AnyCodableValue) -> String {
        switch value {
        case .string(let s): return s
        case .int(let i): return String(i)
        case .double(let d): return String(d)
        case .bool(let b): return String(b)
        case .null: return "null"
        case .array, .object:
            if let data = try? JSONEncoder().encode(value),
               let str = String(data: data, encoding: .utf8) {
                return str
            }
            return "[complex value]"
        }
    }

    private func formatDate(_ isoString: String) -> String {
        guard let date = ISO8601DateFormatter.ainm.date(from: isoString) else {
            return isoString
        }
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        return formatter.string(from: date)
    }
}
