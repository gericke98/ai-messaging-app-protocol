import SwiftUI

/// Half-sheet showing memory context for a thread (UI-SPEC Component 6).
/// Displays conversation summary, active context window, archival facts preview, and participants.
struct MemoryContextView: View {
    let threadId: String
    let appState: AppState

    @State private var showArchivalMemory = false
    @State private var showMemoryGraph = false

    private var frame: ContextFrame? { appState.contextFrame(forThread: threadId) }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    compoundMemoryBanner
                    summarySection
                    activeContextSection
                    archivalFactsPreview
                    participantsSection
                }
                .padding(24)
            }
            .navigationTitle("Memory Context")
            .navigationBarTitleDisplayMode(.inline)
            .navigationDestination(isPresented: $showArchivalMemory) {
                ArchivalMemoryView(appState: appState)
            }
            .navigationDestination(isPresented: $showMemoryGraph) {
                MemoryGraphView(appState: appState)
            }
        }
    }

    // MARK: - Compound Memory Banner

    @ViewBuilder
    private var compoundMemoryBanner: some View {
        Button {
            showMemoryGraph = true
        } label: {
            HStack(spacing: 12) {
                Image(systemName: "brain.head.profile")
                    .font(.title2)
                    .foregroundStyle(Color.accentColor)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Memory Graph")
                        .font(.subheadline.bold())
                        .foregroundStyle(Color(.label))
                    if let stats = appState.graphStats {
                        Text("\(stats.nodes) nodes, \(stats.edges) edges")
                            .font(.caption)
                            .foregroundStyle(Color(.secondaryLabel))
                    } else {
                        Text("View your compound memory")
                            .font(.caption)
                            .foregroundStyle(Color(.secondaryLabel))
                    }
                }
                Spacer()
                Image(systemName: "chevron.right")
                    .font(.caption)
                    .foregroundStyle(Color(.tertiaryLabel))
            }
            .padding(16)
            .background(Color(.secondarySystemGroupedBackground))
            .clipShape(RoundedRectangle(cornerRadius: 12))
        }
        .buttonStyle(.plain)
    }

    // MARK: - Summary Section

    @ViewBuilder
    private var summarySection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Summary")
                .font(.subheadline.bold())
            if let summary = frame?.frame.conversationSummary, !summary.isEmpty {
                Text(summary)
                    .font(.body)
            } else {
                Text("No summary available")
                    .font(.body)
                    .foregroundStyle(Color(.secondaryLabel))
            }
        }
    }

    // MARK: - Active Context Section

    @ViewBuilder
    private var activeContextSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Active Context")
                .font(.subheadline.bold())
            Text("\(frame?.frame.activeContext.messageCount ?? 0) messages from \(frame?.frame.activeContext.windowStart ?? "unknown")")
                .font(.caption)
                .foregroundStyle(Color(.secondaryLabel))

            if let messages = frame?.frame.activeContext.recentMessages, !messages.isEmpty {
                VStack(alignment: .leading, spacing: 12) {
                    ForEach(messages.prefix(10)) { message in
                        VStack(alignment: .leading, spacing: 2) {
                            HStack {
                                Text(message.from)
                                    .font(.subheadline.bold())
                                Spacer()
                                Text(message.timestamp)
                                    .font(.caption)
                                    .foregroundStyle(Color(.secondaryLabel))
                            }
                            Text(message.summary)
                                .font(.body)
                        }
                    }
                }
            }
        }
    }

    // MARK: - Archival Facts Preview

    @ViewBuilder
    private var archivalFactsPreview: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Archival Facts")
                    .font(.subheadline.bold())
                Spacer()
                Button {
                    showArchivalMemory = true
                } label: {
                    Text("View All")
                        .font(.caption.bold())
                        .foregroundStyle(Color.accentColor)
                }
            }

            if let facts = frame?.frame.archivalFacts, !facts.isEmpty {
                ForEach(facts.prefix(5)) { fact in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(fact.key)
                            .font(.subheadline.bold())
                        Text(displayValue(fact.value))
                            .font(.body)
                            .lineLimit(1)
                    }
                }
            } else {
                Text("No archival facts")
                    .font(.body)
                    .foregroundStyle(Color(.secondaryLabel))
            }
        }
    }

    // MARK: - Participants Section

    @ViewBuilder
    private var participantsSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Participants")
                .font(.subheadline.bold())

            if let participants = frame?.frame.participants, !participants.isEmpty {
                ForEach(participants, id: \.identity) { participant in
                    HStack(spacing: 8) {
                        AvatarView(participantType: participant.type, size: 24)
                        Text(participant.identity.components(separatedBy: "@").first ?? participant.identity)
                            .font(.subheadline)
                        Spacer()
                        Circle()
                            .fill(participant.presence == "online" ? Color.green : Color.gray)
                            .frame(width: 8, height: 8)
                    }
                }
            } else {
                Text("No participants")
                    .font(.body)
                    .foregroundStyle(Color(.secondaryLabel))
            }
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
}
