import SwiftUI

/// Agent Discovery screen per UI-SPEC Component 4.
/// Displays cached Agent Cards with search, pull-to-refresh, and manual address entry.
struct AgentDiscoveryView: View {
    let appState: AppState

    @State private var agents: [(identity: String, card: AgentCard)] = []
    @State private var searchText = ""
    @State private var isLoading = true
    @State private var addAgentAddress = ""
    @State private var addAgentError: String?
    @State private var isAddingAgent = false

    private var filteredAgents: [(identity: String, card: AgentCard)] {
        if searchText.isEmpty { return agents }
        return agents.filter { item in
            item.card.name.localizedCaseInsensitiveContains(searchText) ||
            item.card.description.localizedCaseInsensitiveContains(searchText) ||
            (item.card.skills.flatMap { $0.tags ?? [] }.contains { $0.localizedCaseInsensitiveContains(searchText) })
        }
    }

    var body: some View {
        Group {
            if isLoading {
                ProgressView()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if agents.isEmpty {
                emptyState
            } else {
                agentList
            }
        }
        .navigationTitle("Discover Agents")
        .searchable(text: $searchText, prompt: "Search agents...")
        .refreshable { await loadAgents() }
        .task { await loadAgents() }
    }

    // MARK: - Empty State

    private var emptyState: some View {
        VStack(spacing: 16) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 48))
                .foregroundStyle(Color(.tertiaryLabel))
            Text("No agents discovered")
                .font(.title2.bold())
            Text("Connect to an agent by entering their AINM address")
                .font(.subheadline)
                .foregroundStyle(Color(.secondaryLabel))
                .multilineTextAlignment(.center)
            addAgentField
        }
        .padding(.horizontal, 32)
    }

    // MARK: - Add Agent Field

    private var addAgentField: some View {
        VStack(spacing: 8) {
            TextField("agent@server.example", text: $addAgentAddress)
                .textFieldStyle(.roundedBorder)
                .textInputAutocapitalization(.never)
                .keyboardType(.emailAddress)
            if let error = addAgentError {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(Color.red)
            }
            Button("Add Agent") {
                Task { await addAgent() }
            }
            .buttonStyle(.borderedProminent)
            .disabled(addAgentAddress.isEmpty || isAddingAgent)
        }
    }

    // MARK: - Agent List

    private var agentList: some View {
        List {
            ForEach(filteredAgents, id: \.identity) { item in
                NavigationLink {
                    AgentCardDetailView(
                        identity: item.identity,
                        card: item.card,
                        appState: appState
                    )
                } label: {
                    agentRow(item.card)
                }
            }

            Section {
                addAgentField
            }
        }
        .listStyle(.plain)
    }

    // MARK: - Agent Row

    private func agentRow(_ card: AgentCard) -> some View {
        HStack(spacing: 12) {
            AvatarView(participantType: "agent", size: 44)

            VStack(alignment: .leading, spacing: 4) {
                Text(card.name)
                    .font(.headline)
                    .lineLimit(1)

                Text(card.description)
                    .font(.body)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)

                let tags = card.skills.flatMap { $0.tags ?? [] }
                if !tags.isEmpty {
                    HStack(spacing: 6) {
                        ForEach(Array(tags.prefix(3)), id: \.self) { tag in
                            Text(tag)
                                .font(.caption.bold())
                                .padding(.horizontal, 8)
                                .padding(.vertical, 4)
                                .background(Color(.tertiarySystemFill))
                                .clipShape(Capsule())
                        }
                    }
                }
            }
        }
        .padding(.vertical, 4)
    }

    // MARK: - Actions

    private func loadAgents() async {
        guard let db = appState.appDatabase else {
            isLoading = false
            return
        }
        let service = AgentCardService(appDatabase: db)
        do {
            agents = try await service.loadCachedCards()
        } catch {
            // Silently handle -- empty state shown
        }
        isLoading = false
    }

    private func addAgent() async {
        guard let db = appState.appDatabase else { return }
        isAddingAgent = true
        addAgentError = nil
        let service = AgentCardService(appDatabase: db)
        do {
            let card = try await service.fetchCardByAddress(addAgentAddress)
            let identity = addAgentAddress
            agents.append((identity: identity, card: card))
            addAgentAddress = ""
        } catch {
            addAgentError = "Could not reach agent at this address"
        }
        isAddingAgent = false
    }
}
