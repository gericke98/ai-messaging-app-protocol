import SwiftUI

/// Displays the latest agent briefing with collapsible sections.
/// Shows what memory was shared with an agent when it joined a thread.
struct BriefingView: View {
    let appState: AppState

    private var briefing: AgentBriefing? { appState.latestBriefing }

    var body: some View {
        Group {
            if let briefing {
                briefingContent(briefing)
            } else {
                emptyState
            }
        }
    }

    // MARK: - Briefing Content

    @ViewBuilder
    private func briefingContent(_ briefing: AgentBriefing) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                // Header
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Text("Agent Briefing")
                            .font(.headline)
                        Spacer()
                        scopeBadge(briefing.scope)
                    }
                    HStack(spacing: 16) {
                        Label(briefing.consentLevel, systemImage: "lock.shield")
                            .font(.caption)
                            .foregroundStyle(Color(.secondaryLabel))
                        Label(formatDate(briefing.generatedAt), systemImage: "clock")
                            .font(.caption)
                            .foregroundStyle(Color(.secondaryLabel))
                    }
                    if let tokens = briefing.tokenBudgetUsed {
                        Text("\(tokens) tokens")
                            .font(.caption2.monospacedDigit())
                            .foregroundStyle(Color(.tertiaryLabel))
                    }
                }

                // Sections
                if let identity = briefing.sections.identity, !identity.isEmpty {
                    identitySection(identity)
                }
                if let prefs = briefing.sections.preferences, !prefs.isEmpty {
                    nodeSummarySection("Preferences", icon: "slider.horizontal.3", items: prefs, color: .green)
                }
                if let expertise = briefing.sections.expertise, !expertise.isEmpty {
                    nodeSummarySection("Expertise", icon: "star", items: expertise, color: .orange)
                }
                if let goals = briefing.sections.activeGoals, !goals.isEmpty {
                    nodeSummarySection("Active Goals", icon: "target", items: goals, color: .purple)
                }
                if let patterns = briefing.sections.interactionPatterns, !patterns.isEmpty {
                    nodeSummarySection("Interaction Patterns", icon: "waveform.path", items: patterns, color: .teal)
                }
                if let history = briefing.sections.relevantHistory, !history.isEmpty {
                    nodeSummarySection("Relevant History", icon: "clock.arrow.circlepath", items: history, color: .blue)
                }
            }
            .padding(24)
        }
    }

    // MARK: - Identity Section

    @ViewBuilder
    private func identitySection(_ identity: [String: AnyCodableValue]) -> some View {
        CollapsibleSection(title: "Identity", icon: "person.fill", color: .blue) {
            ForEach(Array(identity.keys.sorted()), id: \.self) { key in
                if let value = identity[key] {
                    HStack(alignment: .top) {
                        Text(key.replacingOccurrences(of: "_", with: " ").capitalized)
                            .font(.caption.bold())
                            .foregroundStyle(Color(.secondaryLabel))
                            .frame(width: 100, alignment: .leading)
                        Text(displayValue(value))
                            .font(.body)
                    }
                }
            }
        }
    }

    // MARK: - Node Summary Section

    @ViewBuilder
    private func nodeSummarySection(
        _ title: String,
        icon: String,
        items: [BriefingNodeSummary],
        color: Color
    ) -> some View {
        CollapsibleSection(title: title, icon: icon, color: color) {
            ForEach(items) { item in
                HStack {
                    Text(item.label)
                        .font(.body)
                        .lineLimit(2)
                    Spacer()
                    HStack(spacing: 3) {
                        Circle()
                            .fill(confidenceColor(item.confidence))
                            .frame(width: 6, height: 6)
                        Text(String(format: "%.0f%%", item.confidence * 100))
                            .font(.caption2.monospacedDigit())
                            .foregroundStyle(Color(.secondaryLabel))
                    }
                }
                .padding(.vertical, 2)
            }
        }
    }

    // MARK: - Scope Badge

    @ViewBuilder
    private func scopeBadge(_ scope: String) -> some View {
        Text(scope)
            .font(.caption.bold())
            .padding(.vertical, 3)
            .padding(.horizontal, 8)
            .background(scopeColor(scope).opacity(0.15))
            .foregroundStyle(scopeColor(scope))
            .clipShape(Capsule())
    }

    // MARK: - Empty State

    @ViewBuilder
    private var emptyState: some View {
        VStack(spacing: 12) {
            Image(systemName: "doc.text.magnifyingglass")
                .font(.system(size: 48))
                .foregroundStyle(Color(.tertiaryLabel))
            Text("No briefing yet")
                .font(.title2.bold())
            Text("A briefing is generated when an agent joins a thread")
                .font(.subheadline)
                .foregroundStyle(Color(.secondaryLabel))
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Helpers

    private func scopeColor(_ scope: String) -> Color {
        switch scope {
        case "full": return .green
        case "domain-scoped": return .blue
        default: return .orange
        }
    }

    private func confidenceColor(_ confidence: Float) -> Color {
        if confidence >= 0.8 { return .green }
        if confidence >= 0.5 { return .yellow }
        return .red
    }

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

// MARK: - Collapsible Section Component

/// Reusable collapsible section with icon and color accent.
private struct CollapsibleSection<Content: View>: View {
    let title: String
    let icon: String
    let color: Color
    @ViewBuilder let content: () -> Content

    @State private var isExpanded = true

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Button {
                withAnimation(.easeInOut(duration: 0.2)) {
                    isExpanded.toggle()
                }
            } label: {
                HStack {
                    Image(systemName: icon)
                        .foregroundStyle(color)
                    Text(title)
                        .font(.subheadline.bold())
                    Spacer()
                    Image(systemName: isExpanded ? "chevron.up" : "chevron.down")
                        .font(.caption)
                        .foregroundStyle(Color(.tertiaryLabel))
                }
                .padding(12)
            }
            .buttonStyle(.plain)

            if isExpanded {
                VStack(alignment: .leading, spacing: 8) {
                    content()
                }
                .padding(.horizontal, 12)
                .padding(.bottom, 12)
            }
        }
        .background(Color(.secondarySystemGroupedBackground))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }
}
