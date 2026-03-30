import SwiftUI

/// Renders a TableBlock with horizontal scroll, sortable columns, and optional row actions.
/// Per UI-SPEC Component 7: card container, 12pt corner radius, alternating row backgrounds.
/// Action callback wired in Plan 04; defaults to nil for display-only rendering.
struct TableBlockView: View {
    let content: ContentBlock.TableContent
    var onAction: ((_ actionId: String, _ data: [String: Any]) -> Void)? = nil
    @State private var sortColumn: String? = nil
    @State private var sortAscending: Bool = true
    @State private var sortTapCount: Int = 0

    private var sortedRows: [[AnyCodableValue]] {
        guard let column = sortColumn,
              let colIndex = content.columns.firstIndex(where: { $0.key == column }) else {
            return content.rows
        }
        return content.rows.sorted { row1, row2 in
            let val1 = colIndex < row1.count ? row1[colIndex] : .null
            let val2 = colIndex < row2.count ? row2[colIndex] : .null
            let cmp = compareValues(val1, val2)
            return sortAscending ? cmp : !cmp
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let title = content.title {
                Text(title)
                    .font(.headline)
                    .fontWeight(.bold)
                    .padding(.horizontal, 16)
                    .padding(.top, 12)
                    .padding(.bottom, 8)
            }

            if content.rows.isEmpty {
                Text("No rows to display")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(16)
            } else {
                ScrollView(.horizontal, showsIndicators: true) {
                    VStack(spacing: 0) {
                        headerRow
                        ForEach(Array(sortedRows.enumerated()), id: \.offset) { index, row in
                            dataRow(row, index: index)
                        }
                    }
                }
            }
        }
        .background(Color(.secondarySystemBackground))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    // MARK: - Header

    private var headerRow: some View {
        HStack(spacing: 0) {
            ForEach(Array(content.columns.enumerated()), id: \.offset) { _, column in
                headerCell(for: column)
            }
            if content.actions != nil {
                Text("ACTIONS")
                    .font(.caption)
                    .fontWeight(.bold)
                    .foregroundStyle(.secondary)
                    .frame(minWidth: 80, alignment: .leading)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
            }
        }
        .background(Color(.tertiarySystemBackground))
    }

    private func headerCell(for column: ContentBlock.TableColumn) -> some View {
        let isSortable = column.sortable == true || content.sortable == true

        return Button {
            if isSortable {
                handleSortTap(column: column.key)
            }
        } label: {
            HStack(spacing: 4) {
                Text(column.label.uppercased())
                    .font(.caption)
                    .fontWeight(.bold)
                    .foregroundStyle(.secondary)

                if isSortable && sortColumn == column.key {
                    Image(systemName: sortAscending ? "chevron.up" : "chevron.down")
                        .font(.system(size: 8))
                        .foregroundStyle(.secondary)
                }
            }
            .frame(minWidth: 80, alignment: alignment(for: column.align))
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
        }
        .disabled(!isSortable)
    }

    // MARK: - Data Rows

    private func dataRow(_ row: [AnyCodableValue], index: Int) -> some View {
        VStack(spacing: 0) {
            if index > 0 {
                Rectangle()
                    .fill(Color(.separator))
                    .frame(height: 0.5)
            }
            HStack(spacing: 0) {
                ForEach(Array(content.columns.enumerated()), id: \.offset) { colIndex, column in
                    Text(displayValue(colIndex < row.count ? row[colIndex] : .null))
                        .font(.subheadline)
                        .frame(minWidth: 80, alignment: alignment(for: column.align))
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                }
                if let actions = content.actions {
                    HStack(spacing: 8) {
                        ForEach(Array(actions.enumerated()), id: \.offset) { _, action in
                            Button {
                                var rowData: [String: Any] = [:]
                                for (colIndex, col) in content.columns.enumerated() where colIndex < row.count {
                                    rowData[col.key] = displayValue(row[colIndex])
                                }
                                onAction?(action.actionId, rowData)
                            } label: {
                                Text(action.label)
                                    .font(.caption)
                                    .fontWeight(.bold)
                                    .foregroundStyle(actionForegroundColor(action.style))
                                    .padding(.horizontal, 8)
                                    .frame(height: 28)
                                    .background(actionBackgroundColor(action.style))
                                    .clipShape(Capsule())
                            }
                        }
                    }
                    .frame(minWidth: 80)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                }
            }
            .background(index % 2 == 0 ? Color(.systemBackground) : Color(.secondarySystemBackground))
        }
    }

    // MARK: - Helpers

    private func alignment(for align: String?) -> Alignment {
        switch align {
        case "right": return .trailing
        case "center": return .center
        default: return .leading
        }
    }

    private func handleSortTap(column: String) {
        if sortColumn == column {
            sortTapCount += 1
            if sortTapCount == 1 {
                sortAscending = false
            } else {
                // Clear sort
                sortColumn = nil
                sortTapCount = 0
                sortAscending = true
            }
        } else {
            sortColumn = column
            sortAscending = true
            sortTapCount = 0
        }
    }

    private func displayValue(_ value: AnyCodableValue) -> String {
        switch value {
        case .null: return ""
        case .bool(let v): return v ? "Yes" : "No"
        case .int(let v): return String(v)
        case .double(let v): return String(v)
        case .string(let v): return v
        case .array: return "[...]"
        case .object: return "{...}"
        }
    }

    private func compareValues(_ lhs: AnyCodableValue, _ rhs: AnyCodableValue) -> Bool {
        switch (lhs, rhs) {
        case (.string(let a), .string(let b)): return a < b
        case (.int(let a), .int(let b)): return a < b
        case (.double(let a), .double(let b)): return a < b
        case (.int(let a), .double(let b)): return Double(a) < b
        case (.double(let a), .int(let b)): return a < Double(b)
        default: return displayValue(lhs) < displayValue(rhs)
        }
    }

    private func actionBackgroundColor(_ style: String?) -> Color {
        switch style {
        case "danger": return Color.red
        case "secondary": return Color(.secondarySystemFill)
        default: return Color.accentColor
        }
    }

    private func actionForegroundColor(_ style: String?) -> Color {
        switch style {
        case "secondary": return Color(.label)
        default: return .white
        }
    }
}
