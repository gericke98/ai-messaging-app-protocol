import SwiftUI

/// Expanded detail content for a tool call indicator per UI-SPEC Component 1.
/// Shows input JSON, progress, result summary, and error sections.
struct ToolCallDetailView: View {
    let state: ToolCallState

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Input section
            if let input = state.input, !input.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Input")
                        .font(.caption.bold())
                        .foregroundStyle(Color(.label))
                    ScrollView {
                        Text(prettyPrintJSON(input))
                            .font(.caption.monospaced())
                            .foregroundStyle(Color(.label))
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .padding(8)
                    .frame(maxHeight: 200)
                    .background(Color(.tertiarySystemBackground))
                    .clipShape(RoundedRectangle(cornerRadius: 6))
                    .accessibilityIdentifier(UITestID.ToolCall.detailInputJson)
                }
            }

            // Progress section
            if let progress = state.progress {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Progress")
                        .font(.caption.bold())
                        .foregroundStyle(Color(.label))
                    if let detail = progress.detail {
                        Text(detail)
                            .font(.caption)
                            .foregroundStyle(Color(.label))
                    }
                    if let total = progress.total, total > 0 {
                        ProgressView(
                            value: Double(progress.current ?? 0),
                            total: Double(total)
                        )
                        .tint(Color.accentColor)
                    }
                }
            }

            // Result section
            if state.status == .completed, let resultSummary = state.resultSummary {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Result")
                        .font(.caption.bold())
                        .foregroundStyle(Color(.label))
                    Text(resultSummary)
                        .font(.caption)
                        .foregroundStyle(Color(.label))
                        .accessibilityIdentifier(UITestID.ToolCall.detailResult)
                }
            }

            // Error section
            if state.status == .error, let errorMessage = state.errorMessage {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Error")
                        .font(.caption.bold())
                        .foregroundStyle(Color(.label))
                    Text(errorMessage)
                        .font(.caption)
                        .foregroundStyle(Color.red)
                }
            }
        }
    }

    /// Pretty-print a dictionary of AnyCodableValue as JSON.
    private func prettyPrintJSON(_ dict: [String: AnyCodableValue]) -> String {
        guard let data = try? JSONEncoder().encode(dict),
              let jsonObject = try? JSONSerialization.jsonObject(with: data),
              let prettyData = try? JSONSerialization.data(
                withJSONObject: jsonObject,
                options: [.prettyPrinted, .sortedKeys]
              ),
              let prettyString = String(data: prettyData, encoding: .utf8) else {
            return "{}"
        }
        return prettyString
    }
}
