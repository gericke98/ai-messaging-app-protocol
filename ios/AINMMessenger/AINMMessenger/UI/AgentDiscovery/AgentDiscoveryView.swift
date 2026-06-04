import SwiftUI

/// Saved agents screen. Lists the active account's saved agent contacts, supports adding a
/// new agent by address or card URL (with a verified preview), starting a thread with one,
/// and removing one. Reads `appState.savedAgents` (per-account `SavedAgentStore`).
struct AgentDiscoveryView: View {
    @Bindable var appState: AppState

    @State private var searchText = ""
    @State private var addAgentInput = ""
    @State private var addAgentError: String?
    @State private var isResolving = false
    @State private var preview: AppState.ResolvedAgent?

    private var saved: [SavedAgentRecord] { appState.savedAgents }

    private var filtered: [SavedAgentRecord] {
        guard !searchText.isEmpty else { return saved }
        return saved.filter { rec in
            rec.displayName.localizedCaseInsensitiveContains(searchText) ||
            rec.address.localizedCaseInsensitiveContains(searchText)
        }
    }

    var body: some View {
        Group {
            if saved.isEmpty {
                emptyState
            } else {
                agentList
            }
        }
        .navigationTitle("Agents")
        .searchable(text: $searchText, prompt: "Search agents...")
        .accessibilityIdentifier(UITestID.AgentDiscovery.search)
        .task { await appState.loadSavedAgents() }
        .refreshable { await appState.loadSavedAgents() }
    }

    // MARK: - Empty state

    private var emptyState: some View {
        VStack(spacing: 16) {
            Image(systemName: "person.2.badge.gearshape")
                .font(.system(size: 48))
                .foregroundStyle(Color(.tertiaryLabel))
            Text("No agents yet")
                .font(.title2.bold())
            Text("Add an agent on your server by address or card URL")
                .font(.subheadline)
                .foregroundStyle(Color(.secondaryLabel))
                .multilineTextAlignment(.center)
            addAgentField
        }
        .padding(.horizontal, 32)
    }

    // MARK: - Add field + preview

    private var addAgentField: some View {
        VStack(spacing: 8) {
            TextField("agent@server.example  ·  or card URL", text: $addAgentInput)
                .textFieldStyle(.roundedBorder)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .keyboardType(.emailAddress)
                .accessibilityIdentifier(UITestID.AgentDiscovery.addByAddressField)

            if let addAgentError {
                Text(addAgentError).font(.caption).foregroundStyle(Color.red)
            }

            if let preview {
                VStack(alignment: .leading, spacing: 4) {
                    Text(preview.card.name).font(.headline)
                    Text(preview.address).font(.caption).foregroundStyle(.secondary)
                    Text(preview.card.description).font(.subheadline).foregroundStyle(.secondary)
                        .lineLimit(2)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(8)
                .background(Color(.tertiarySystemFill))
                .clipShape(RoundedRectangle(cornerRadius: 8))

                Button("Add \(preview.card.name)") {
                    Task { await confirmAdd(preview) }
                }
                .buttonStyle(.borderedProminent)
                .accessibilityIdentifier(UITestID.AgentDiscovery.addByAddressButton)
            } else {
                Button(isResolving ? "Looking up…" : "Look up") {
                    Task { await lookUp() }
                }
                .buttonStyle(.borderedProminent)
                .disabled(addAgentInput.isEmpty || isResolving)
                .accessibilityIdentifier(UITestID.AgentDiscovery.addByAddressButton)
            }
        }
    }

    // MARK: - List

    private var agentList: some View {
        List {
            ForEach(filtered) { rec in
                NavigationLink {
                    AgentCardDetailView(
                        identity: rec.address,
                        card: rec.agentCard() ?? AppState.placeholderAssistantCard(),
                        appState: appState
                    )
                } label: {
                    agentRow(rec)
                }
                .accessibilityIdentifier(UITestID.AgentDiscovery.listCell(agentId: rec.address))
            }
            .onDelete { offsets in
                let targets = offsets.map { filtered[$0].address }
                Task { for address in targets { await appState.removeAgent(address: address) } }
            }

            Section { addAgentField }
        }
        .listStyle(.plain)
    }

    private func agentRow(_ rec: SavedAgentRecord) -> some View {
        let card = rec.agentCard()
        return HStack(spacing: 12) {
            AvatarView(participantType: "agent", size: 44)
            VStack(alignment: .leading, spacing: 4) {
                Text(rec.displayName).font(.headline).lineLimit(1)
                Text(rec.address).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                if let card {
                    let tags = card.skills.flatMap { $0.tags ?? [] }
                    if !tags.isEmpty {
                        HStack(spacing: 6) {
                            ForEach(Array(tags.prefix(3)), id: \.self) { tag in
                                Text(tag)
                                    .font(.caption.bold())
                                    .padding(.horizontal, 8).padding(.vertical, 4)
                                    .background(Color(.tertiarySystemFill))
                                    .clipShape(Capsule())
                            }
                        }
                    }
                }
            }
        }
        .padding(.vertical, 4)
    }

    // MARK: - Actions

    private func lookUp() async {
        isResolving = true
        addAgentError = nil
        preview = nil
        let result = await appState.resolveAgent(addAgentInput)
        switch result {
        case .success(let resolved): preview = resolved
        case .failure(let err): addAgentError = err.userMessage
        }
        isResolving = false
    }

    private func confirmAdd(_ resolved: AppState.ResolvedAgent) async {
        await appState.saveAgent(address: resolved.address, card: resolved.card)
        addAgentInput = ""
        preview = nil
        addAgentError = nil
    }
}
