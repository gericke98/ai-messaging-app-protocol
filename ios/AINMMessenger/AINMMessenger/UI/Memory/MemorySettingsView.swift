import SwiftUI

/// Memory access control and management settings.
/// Allows consent level changes, export, and lint operations.
struct MemorySettingsView: View {
    let appState: AppState

    @State private var consentLevel = "standard"
    @State private var isExporting = false
    @State private var isLinting = false
    @State private var exportData: MemoryExport?
    @State private var showExportSheet = false
    @State private var showLintResults = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                consentSection
                actionsSection
                if let lint = appState.lintResult {
                    lintSection(lint)
                }
                graphStatsSection
            }
            .padding(24)
        }
    }

    // MARK: - Consent Level

    @ViewBuilder
    private var consentSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Default Consent Level")
                .font(.subheadline.bold())
            Text("Controls what memory is shared with agents by default")
                .font(.caption)
                .foregroundStyle(Color(.secondaryLabel))

            Picker("Consent", selection: $consentLevel) {
                Text("Minimal").tag("minimal")
                Text("Standard").tag("standard")
                Text("Full").tag("full")
            }
            .pickerStyle(.segmented)

            consentDescription
        }
        .padding(16)
        .background(Color(.secondarySystemGroupedBackground))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    @ViewBuilder
    private var consentDescription: some View {
        switch consentLevel {
        case "minimal":
            Label("Identity only", systemImage: "lock.fill")
                .font(.caption)
                .foregroundStyle(Color.orange)
        case "full":
            Label("All scopes: identity, preferences, expertise, goals, history, patterns", systemImage: "lock.open.fill")
                .font(.caption)
                .foregroundStyle(Color.green)
        default:
            Label("Identity, preferences, expertise, goals", systemImage: "lock.shield")
                .font(.caption)
                .foregroundStyle(Color.blue)
        }
    }

    // MARK: - Actions

    @ViewBuilder
    private var actionsSection: some View {
        VStack(spacing: 12) {
            // Export
            Button {
                Task { await exportMemory() }
            } label: {
                HStack {
                    Image(systemName: "square.and.arrow.up")
                    Text("Export Memory Graph")
                    Spacer()
                    if isExporting {
                        ProgressView()
                    }
                }
                .padding(16)
                .background(Color(.secondarySystemGroupedBackground))
                .clipShape(RoundedRectangle(cornerRadius: 12))
            }
            .buttonStyle(.plain)
            .disabled(isExporting)

            // Lint
            Button {
                Task { await runLint() }
            } label: {
                HStack {
                    Image(systemName: "checkmark.shield")
                    Text("Run Memory Lint")
                    Spacer()
                    if isLinting {
                        ProgressView()
                    }
                }
                .padding(16)
                .background(Color(.secondarySystemGroupedBackground))
                .clipShape(RoundedRectangle(cornerRadius: 12))
            }
            .buttonStyle(.plain)
            .disabled(isLinting)
        }
        .sheet(isPresented: $showExportSheet) {
            if let exportData {
                ExportShareView(export: exportData)
            }
        }
    }

    // MARK: - Lint Results

    @ViewBuilder
    private func lintSection(_ lint: LintResult) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Lint Results")
                    .font(.subheadline.bold())
                Spacer()
                Text(formatDate(lint.timestamp))
                    .font(.caption)
                    .foregroundStyle(Color(.secondaryLabel))
            }

            // Summary badges
            HStack(spacing: 12) {
                statBadge("Nodes", count: lint.summary.totalNodes, color: .blue)
                statBadge("Edges", count: lint.summary.totalEdges, color: .blue)
                if lint.summary.contradictions > 0 {
                    statBadge("Conflicts", count: lint.summary.contradictions, color: .red)
                }
                if lint.summary.orphanNodes > 0 {
                    statBadge("Orphans", count: lint.summary.orphanNodes, color: .orange)
                }
                if lint.summary.staleNodes > 0 {
                    statBadge("Stale", count: lint.summary.staleNodes, color: .yellow)
                }
            }

            // Findings list
            if !lint.findings.isEmpty {
                ForEach(lint.findings) { finding in
                    findingRow(finding)
                }
            } else {
                Label("No issues found", systemImage: "checkmark.circle.fill")
                    .font(.body)
                    .foregroundStyle(Color.green)
            }
        }
        .padding(16)
        .background(Color(.secondarySystemGroupedBackground))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    @ViewBuilder
    private func findingRow(_ finding: LintFinding) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: findingIcon(finding.type))
                .foregroundStyle(findingColor(finding.severity))
                .font(.caption)
                .frame(width: 16)
            VStack(alignment: .leading, spacing: 2) {
                Text(finding.type.replacingOccurrences(of: "_", with: " ").capitalized)
                    .font(.caption.bold())
                Text(finding.description)
                    .font(.caption)
                    .foregroundStyle(Color(.secondaryLabel))
            }
        }
    }

    // MARK: - Graph Stats

    @ViewBuilder
    private var graphStatsSection: some View {
        if let stats = appState.graphStats {
            VStack(alignment: .leading, spacing: 8) {
                Text("Graph Statistics")
                    .font(.subheadline.bold())
                HStack(spacing: 16) {
                    statBadge("Nodes", count: stats.nodes, color: .blue)
                    statBadge("Edges", count: stats.edges, color: .purple)
                }
            }
            .padding(16)
            .background(Color(.secondarySystemGroupedBackground))
            .clipShape(RoundedRectangle(cornerRadius: 12))
        }
    }

    // MARK: - Components

    @ViewBuilder
    private func statBadge(_ label: String, count: Int, color: Color) -> some View {
        VStack(spacing: 2) {
            Text("\(count)")
                .font(.title3.bold().monospacedDigit())
                .foregroundStyle(color)
            Text(label)
                .font(.caption2)
                .foregroundStyle(Color(.secondaryLabel))
        }
    }

    // MARK: - Actions

    private func exportMemory() async {
        isExporting = true
        defer { isExporting = false }
        if let export = await appState.exportMemory() {
            exportData = export
            showExportSheet = true
        }
    }

    private func runLint() async {
        isLinting = true
        defer { isLinting = false }
        await appState.runLint()
    }

    // MARK: - Helpers

    private func findingIcon(_ type: String) -> String {
        switch type {
        case "contradiction": return "exclamationmark.triangle.fill"
        case "orphan": return "questionmark.circle"
        case "stale": return "clock.badge.exclamationmark"
        default: return "info.circle"
        }
    }

    private func findingColor(_ severity: String) -> Color {
        switch severity {
        case "warning": return .orange
        case "error": return .red
        default: return .blue
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

// MARK: - Export Share View

/// Presents the exported memory JSON for sharing via system share sheet.
private struct ExportShareView: View {
    let export: MemoryExport
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    HStack {
                        Label(export.format, systemImage: "doc.text")
                            .font(.headline)
                        Spacer()
                        Text("v\(export.version)")
                            .font(.caption.monospacedDigit())
                            .foregroundStyle(Color(.secondaryLabel))
                    }

                    HStack(spacing: 16) {
                        Label("\(export.metadata.nodeCount) nodes", systemImage: "circle.grid.3x3")
                            .font(.caption)
                        Label("\(export.metadata.edgeCount) edges", systemImage: "line.diagonal")
                            .font(.caption)
                    }
                    .foregroundStyle(Color(.secondaryLabel))

                    // JSON preview
                    if let data = try? JSONEncoder.prettyPrinting.encode(export),
                       let json = String(data: data, encoding: .utf8) {
                        Text(json)
                            .font(.caption.monospaced())
                            .textSelection(.enabled)
                            .padding(12)
                            .background(Color(.tertiarySystemFill))
                            .clipShape(RoundedRectangle(cornerRadius: 8))
                    }
                }
                .padding(24)
            }
            .navigationTitle("Memory Export")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }
                }
                ToolbarItem(placement: .topBarLeading) {
                    ShareLink(item: exportJSON) {
                        Image(systemName: "square.and.arrow.up")
                    }
                }
            }
        }
    }

    private var exportJSON: String {
        if let data = try? JSONEncoder.prettyPrinting.encode(export),
           let json = String(data: data, encoding: .utf8) {
            return json
        }
        return "{}"
    }
}

// MARK: - JSONEncoder Extension

private extension JSONEncoder {
    static let prettyPrinting: JSONEncoder = {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        return encoder
    }()
}
