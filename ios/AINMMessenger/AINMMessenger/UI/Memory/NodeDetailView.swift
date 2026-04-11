import SwiftUI

/// Detail view for a single compound memory node.
/// Shows full properties, connected edges, provenance trail, and temporal history.
struct NodeDetailView: View {
    let node: MemoryNode
    let viewModel: MemoryGraphViewModel

    private var connectedEdges: [MemoryEdge] {
        viewModel.edges(for: node.nodeId)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                headerSection
                propertiesSection
                edgesSection
                provenanceSection
                temporalSection
                metadataSection
            }
            .padding(24)
        }
        .navigationTitle("Node Detail")
        .navigationBarTitleDisplayMode(.inline)
    }

    // MARK: - Header

    @ViewBuilder
    private var headerSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(node.label)
                .font(.title2.bold())

            HStack(spacing: 12) {
                typeBadge(node.type)
                categoryBadge(node.category)
                Spacer()
                confidenceView
            }
        }
    }

    @ViewBuilder
    private var confidenceView: some View {
        VStack(alignment: .trailing, spacing: 2) {
            Text(String(format: "%.0f%%", node.confidence * 100))
                .font(.title3.bold().monospacedDigit())
                .foregroundStyle(confidenceColor)
            Text("confidence")
                .font(.caption2)
                .foregroundStyle(Color(.secondaryLabel))
        }
    }

    // MARK: - Properties

    @ViewBuilder
    private var propertiesSection: some View {
        if let props = node.properties, case .object(let dict) = props, !dict.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text("Properties")
                    .font(.subheadline.bold())

                ForEach(Array(dict.keys.sorted()), id: \.self) { key in
                    if let value = dict[key] {
                        HStack(alignment: .top) {
                            Text(key)
                                .font(.caption.bold())
                                .foregroundStyle(Color(.secondaryLabel))
                                .frame(width: 100, alignment: .leading)
                            Text(displayValue(value))
                                .font(.body)
                        }
                    }
                }
            }
            .padding(16)
            .background(Color(.secondarySystemGroupedBackground))
            .clipShape(RoundedRectangle(cornerRadius: 12))
        }
    }

    // MARK: - Edges

    @ViewBuilder
    private var edgesSection: some View {
        if !connectedEdges.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text("Connections")
                    .font(.subheadline.bold())
                Text("\(connectedEdges.count) edges")
                    .font(.caption)
                    .foregroundStyle(Color(.secondaryLabel))

                ForEach(connectedEdges) { edge in
                    edgeRow(edge)
                }
            }
        }
    }

    @ViewBuilder
    private func edgeRow(_ edge: MemoryEdge) -> some View {
        let isSource = edge.sourceNode == node.nodeId
        let otherNode = viewModel.connectedNode(edge: edge, from: node.nodeId)

        HStack(spacing: 8) {
            Image(systemName: isSource ? "arrow.right.circle" : "arrow.left.circle")
                .font(.caption)
                .foregroundStyle(Color.accentColor)

            VStack(alignment: .leading, spacing: 2) {
                Text(edge.relationship.replacingOccurrences(of: "_", with: " "))
                    .font(.caption.bold())
                    .foregroundStyle(Color.accentColor)
                if let other = otherNode {
                    Text(other.label)
                        .font(.body)
                        .lineLimit(1)
                } else {
                    Text(isSource ? edge.targetNode : edge.sourceNode)
                        .font(.caption.monospaced())
                        .foregroundStyle(Color(.tertiaryLabel))
                        .lineLimit(1)
                }
            }

            Spacer()

            if let weight = edge.weight {
                Text(String(format: "w:%.1f", weight))
                    .font(.caption2.monospacedDigit())
                    .foregroundStyle(Color(.tertiaryLabel))
            }
        }
        .padding(12)
        .background(Color(.tertiarySystemFill))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    // MARK: - Provenance

    @ViewBuilder
    private var provenanceSection: some View {
        if let source = node.source, case .object(let dict) = source, !dict.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text("Provenance")
                    .font(.subheadline.bold())

                if let contributor = dict["contributed_by"], case .string(let name) = contributor {
                    provenanceRow("Contributed by", value: name.components(separatedBy: "@").first ?? name)
                }
                if let threadId = dict["thread_id"], case .string(let tid) = threadId {
                    provenanceRow("Thread", value: String(tid.prefix(8)) + "...")
                }
                if let extractedAt = dict["extracted_at"], case .string(let date) = extractedAt {
                    provenanceRow("Extracted", value: formatDate(date))
                }
                if let messageId = dict["message_id"], case .string(let mid) = messageId {
                    provenanceRow("Message", value: String(mid.prefix(8)) + "...")
                }
            }
            .padding(16)
            .background(Color(.secondarySystemGroupedBackground))
            .clipShape(RoundedRectangle(cornerRadius: 12))
        }
    }

    @ViewBuilder
    private func provenanceRow(_ label: String, value: String) -> some View {
        HStack {
            Text(label)
                .font(.caption.bold())
                .foregroundStyle(Color(.secondaryLabel))
            Spacer()
            Text(value)
                .font(.caption)
        }
    }

    // MARK: - Temporal

    @ViewBuilder
    private var temporalSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Temporal")
                .font(.subheadline.bold())

            provenanceRow("Valid from", value: formatDate(node.temporal.validFrom))

            if let until = node.temporal.validUntil {
                provenanceRow("Valid until", value: formatDate(until))
            }

            if let superseded = node.temporal.supersededBy {
                HStack {
                    Text("Superseded by")
                        .font(.caption.bold())
                        .foregroundStyle(Color(.secondaryLabel))
                    Spacer()
                    Text(String(superseded.prefix(8)) + "...")
                        .font(.caption.monospaced())
                        .foregroundStyle(Color.orange)
                }
            }
        }
        .padding(16)
        .background(Color(.secondarySystemGroupedBackground))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    // MARK: - Metadata

    @ViewBuilder
    private var metadataSection: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Node ID")
                .font(.caption2)
                .foregroundStyle(Color(.tertiaryLabel))
            Text(node.nodeId)
                .font(.caption.monospaced())
                .foregroundStyle(Color(.secondaryLabel))
                .textSelection(.enabled)
        }
    }

    // MARK: - Components

    @ViewBuilder
    private func typeBadge(_ type: String) -> some View {
        HStack(spacing: 4) {
            typeIcon(type)
            Text(type.capitalized)
                .font(.caption.bold())
        }
        .padding(.vertical, 4)
        .padding(.horizontal, 8)
        .background(Color(.tertiarySystemFill))
        .clipShape(Capsule())
    }

    @ViewBuilder
    private func categoryBadge(_ category: String) -> some View {
        Text(category)
            .font(.caption.bold())
            .padding(.vertical, 4)
            .padding(.horizontal, 8)
            .background(categoryColor(category).opacity(0.15))
            .foregroundStyle(categoryColor(category))
            .clipShape(Capsule())
    }

    @ViewBuilder
    private func typeIcon(_ type: String) -> some View {
        switch type {
        case "semantic":
            Image(systemName: "brain.head.profile").foregroundStyle(Color.blue)
        case "episodic":
            Image(systemName: "clock.arrow.circlepath").foregroundStyle(Color.purple)
        case "procedural":
            Image(systemName: "gearshape.2").foregroundStyle(Color.orange)
        default:
            Image(systemName: "circle").foregroundStyle(Color.gray)
        }
    }

    // MARK: - Helpers

    private var confidenceColor: Color {
        if node.confidence >= 0.8 { return .green }
        if node.confidence >= 0.5 { return .yellow }
        return .red
    }

    private func categoryColor(_ category: String) -> Color {
        switch category {
        case "fact": return .blue
        case "preference": return .green
        case "skill": return .orange
        case "event": return .purple
        case "decision": return .red
        case "pattern": return .teal
        case "workflow": return .indigo
        case "tool": return .mint
        default: return .gray
        }
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
