import SwiftUI

/// Searchable, grouped list of compound memory nodes.
/// Groups by type (semantic/episodic/procedural) with category badges,
/// confidence indicators, and navigation to node detail.
struct NodeListView: View {
    let viewModel: MemoryGraphViewModel

    @State private var selectedNode: MemoryNode?

    var body: some View {
        Group {
            if viewModel.isLoading && viewModel.nodes.isEmpty {
                ProgressView("Loading nodes...")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if viewModel.filteredNodes.isEmpty {
                emptyState
            } else {
                nodeList
            }
        }
        .searchable(text: Bindable(viewModel).searchText, prompt: "Search nodes...")
        .sheet(item: $selectedNode) { node in
            NavigationStack {
                NodeDetailView(node: node, viewModel: viewModel)
            }
            .presentationDetents([.large])
        }
    }

    // MARK: - Node List

    @ViewBuilder
    private var nodeList: some View {
        List {
            // Filter chips
            if !viewModel.availableCategories.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        filterChip("All", isSelected: viewModel.selectedCategoryFilter == nil) {
                            viewModel.selectedCategoryFilter = nil
                            viewModel.applyFilters()
                        }
                        ForEach(viewModel.availableCategories, id: \.self) { category in
                            filterChip(category, isSelected: viewModel.selectedCategoryFilter == category) {
                                viewModel.selectedCategoryFilter = category
                                viewModel.applyFilters()
                            }
                        }
                    }
                    .padding(.horizontal, 4)
                }
                .listRowInsets(EdgeInsets())
                .listRowBackground(Color.clear)
                .listRowSeparator(.hidden)
            }

            // Grouped sections
            ForEach(viewModel.nodesByType, id: \.type) { group in
                Section {
                    ForEach(group.nodes) { node in
                        nodeRow(node)
                            .contentShape(Rectangle())
                            .onTapGesture { selectedNode = node }
                    }
                } header: {
                    HStack {
                        typeIcon(group.type)
                        Text(group.type.capitalized)
                            .font(.subheadline.bold())
                        Spacer()
                        Text("\(group.nodes.count)")
                            .font(.caption.monospacedDigit())
                            .foregroundStyle(Color(.tertiaryLabel))
                    }
                }
            }
        }
        .listStyle(.plain)
    }

    // MARK: - Node Row

    @ViewBuilder
    private func nodeRow(_ node: MemoryNode) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(node.label)
                    .font(.body)
                    .lineLimit(2)
                Spacer()
                confidenceBadge(node.confidence)
            }
            HStack(spacing: 6) {
                categoryBadge(node.category)
                if let tags = node.tags, case .array(let arr) = tags {
                    ForEach(arr.prefix(3), id: \.self) { tag in
                        if case .string(let s) = tag {
                            Text(s)
                                .font(.caption2)
                                .padding(.vertical, 1)
                                .padding(.horizontal, 5)
                                .background(Color(.tertiarySystemFill))
                                .clipShape(Capsule())
                        }
                    }
                }
                Spacer()
                temporalIndicator(node.temporal)
            }
        }
        .padding(.vertical, 4)
    }

    // MARK: - Components

    @ViewBuilder
    private func categoryBadge(_ category: String) -> some View {
        Text(category)
            .font(.caption2.bold())
            .padding(.vertical, 2)
            .padding(.horizontal, 6)
            .background(categoryColor(category).opacity(0.15))
            .foregroundStyle(categoryColor(category))
            .clipShape(Capsule())
    }

    @ViewBuilder
    private func confidenceBadge(_ confidence: Float) -> some View {
        HStack(spacing: 3) {
            Circle()
                .fill(confidenceColor(confidence))
                .frame(width: 6, height: 6)
            Text(String(format: "%.0f%%", confidence * 100))
                .font(.caption2.monospacedDigit())
                .foregroundStyle(Color(.secondaryLabel))
        }
    }

    @ViewBuilder
    private func temporalIndicator(_ temporal: TemporalValidity) -> some View {
        if temporal.validUntil != nil {
            Image(systemName: "clock.badge.xmark")
                .font(.caption2)
                .foregroundStyle(Color(.tertiaryLabel))
        } else if temporal.supersededBy != nil {
            Image(systemName: "arrow.triangle.branch")
                .font(.caption2)
                .foregroundStyle(Color.orange)
        }
    }

    @ViewBuilder
    private func typeIcon(_ type: String) -> some View {
        switch type {
        case "semantic":
            Image(systemName: "brain.head.profile")
                .foregroundStyle(Color.blue)
        case "episodic":
            Image(systemName: "clock.arrow.circlepath")
                .foregroundStyle(Color.purple)
        case "procedural":
            Image(systemName: "gearshape.2")
                .foregroundStyle(Color.orange)
        default:
            Image(systemName: "circle")
                .foregroundStyle(Color.gray)
        }
    }

    @ViewBuilder
    private func filterChip(_ label: String, isSelected: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(label.capitalized)
                .font(.caption.bold())
                .padding(.vertical, 6)
                .padding(.horizontal, 12)
                .background(isSelected ? Color.accentColor : Color(.tertiarySystemFill))
                .foregroundStyle(isSelected ? Color.white : Color(.label))
                .clipShape(Capsule())
        }
        .buttonStyle(.plain)
    }

    // MARK: - Empty State

    @ViewBuilder
    private var emptyState: some View {
        VStack(spacing: 12) {
            Image(systemName: "brain")
                .font(.system(size: 48))
                .foregroundStyle(Color(.tertiaryLabel))
            Text("No memory nodes")
                .font(.title2.bold())
            Text("Your compound memory will grow as you interact with agents")
                .font(.subheadline)
                .foregroundStyle(Color(.secondaryLabel))
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Helpers

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

    private func confidenceColor(_ confidence: Float) -> Color {
        if confidence >= 0.8 { return .green }
        if confidence >= 0.5 { return .yellow }
        return .red
    }
}
