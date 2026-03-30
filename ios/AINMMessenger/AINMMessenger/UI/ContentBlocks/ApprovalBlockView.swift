import SwiftUI

/// Renders an ApprovalBlock with state-dependent visuals (pending, approved, rejected, changes_requested).
/// Per UI-SPEC Component 8: colored card, status icon, option buttons for pending state.
/// Action callback wired in Plan 04; defaults to nil for display-only rendering.
struct ApprovalBlockView: View {
    let content: ContentBlock.ApprovalContent
    var onAction: ((_ actionId: String, _ data: [String: Any]) -> Void)? = nil
    @State private var isSending = false
    @State private var localStatus: String? = nil
    @State private var selectedOption: String? = nil

    private var effectiveStatus: String {
        localStatus ?? content.status
    }

    private var statusColor: Color {
        switch effectiveStatus {
        case "approved": return Color(.systemGreen)
        case "rejected": return Color(.systemRed)
        case "changes_requested": return Color(.systemOrange)
        default: return Color(.systemYellow) // pending
        }
    }

    private var statusIcon: String {
        switch effectiveStatus {
        case "approved": return "checkmark.circle.fill"
        case "rejected": return "xmark.circle.fill"
        case "changes_requested": return "arrow.triangle.2.circlepath"
        default: return "clock.fill"
        }
    }

    private var statusLabel: String {
        switch effectiveStatus {
        case "approved": return "Approved"
        case "rejected": return "Rejected"
        case "changes_requested": return "Changes Requested"
        default: return "Awaiting Approval"
        }
    }

    private var isPending: Bool {
        effectiveStatus == "pending"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Header
            HStack {
                Image(systemName: statusIcon)
                    .font(.system(size: 20))
                    .foregroundStyle(statusColor)

                Text(content.title)
                    .font(.headline)
                    .fontWeight(.bold)

                Spacer()

                Text(statusLabel)
                    .font(.caption)
                    .fontWeight(.bold)
                    .foregroundStyle(statusColor)
            }

            // Description
            if let description = content.description {
                Text(description)
                    .font(.body)
            }

            // Option buttons (pending) or resolved display
            if isPending && !isSending {
                HStack(spacing: 8) {
                    ForEach(content.options) { option in
                        Button {
                            selectedOption = option.value
                            isSending = true
                            onAction?(content.actionId, ["value": option.value])
                        } label: {
                            Text(option.label)
                                .font(.body)
                                .fontWeight(.bold)
                                .frame(maxWidth: .infinity)
                                .frame(height: 44)
                                .foregroundStyle(optionForegroundColor(option.style))
                                .background(optionBackgroundColor(option.style))
                                .clipShape(RoundedRectangle(cornerRadius: 10))
                        }
                        .disabled(isSending)
                    }
                }
                .padding(.top, 4)
            } else if !isPending {
                // Resolved state
                VStack(alignment: .leading, spacing: 4) {
                    HStack(spacing: 4) {
                        Image(systemName: "checkmark")
                            .font(.subheadline)
                            .foregroundStyle(statusColor)
                        Text(resolvedOptionLabel)
                            .font(.subheadline)
                            .fontWeight(.bold)
                            .foregroundStyle(statusColor)
                    }

                    // Show previous options as caption
                    Text(content.options.map(\.label).joined(separator: " / "))
                        .font(.caption)
                        .foregroundStyle(Color(.tertiaryLabel))
                }
            } else if isSending {
                HStack {
                    ProgressView()
                        .frame(width: 14, height: 14)
                    Text("Processing...")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
                .padding(.top, 4)
            }
        }
        .padding(16)
        .background(statusColor.opacity(0.15))
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(statusColor.opacity(0.3), lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .accessibilityElement(children: .combine)
        .accessibilityLabel(accessibilityDescription)
    }

    // MARK: - Helpers

    private var resolvedOptionLabel: String {
        if let selected = selectedOption,
           let option = content.options.first(where: { $0.value == selected }) {
            return option.label
        }
        // Infer from status for server-resolved approvals
        return statusLabel
    }

    private var accessibilityDescription: String {
        if isPending {
            return "Approval: \(content.title), status: \(effectiveStatus)"
        } else {
            return "Resolved as \(effectiveStatus)"
        }
    }

    private func optionBackgroundColor(_ style: String?) -> Color {
        switch style {
        case "danger": return Color.red
        case "secondary": return Color(.secondarySystemFill)
        default: return Color.accentColor
        }
    }

    private func optionForegroundColor(_ style: String?) -> Color {
        switch style {
        case "secondary": return Color(.label)
        default: return .white
        }
    }
}
