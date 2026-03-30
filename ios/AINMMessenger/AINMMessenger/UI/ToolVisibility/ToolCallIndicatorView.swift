import SwiftUI

/// Collapsible tool call indicator per UI-SPEC Component 1.
/// Shows tool name, lifecycle status icon, and duration in collapsed state.
/// Expands to show input, progress, result, or error via ToolCallDetailView.
struct ToolCallIndicatorView: View {
    @Bindable var state: ToolCallState

    var body: some View {
        VStack(spacing: 0) {
            // Collapsed row (always visible)
            Button {
                if UIAccessibility.isReduceMotionEnabled {
                    state.isExpanded.toggle()
                } else {
                    withAnimation(.easeInOut(duration: 0.2)) {
                        state.isExpanded.toggle()
                    }
                }
            } label: {
                HStack(spacing: 8) {
                    statusIcon
                    Text(state.name)
                        .font(.subheadline.bold())
                        .foregroundStyle(Color(.label))
                        .lineLimit(1)
                    Spacer()
                    if state.status == .completed || state.status == .error,
                       let durationMs = state.durationMs {
                        Text(durationMs.formattedDuration)
                            .font(.caption)
                            .foregroundStyle(Color(.secondaryLabel))
                    }
                    Image(systemName: "chevron.right")
                        .font(.system(size: 12))
                        .foregroundStyle(Color(.tertiaryLabel))
                        .rotationEffect(.degrees(state.isExpanded ? 90 : 0))
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .frame(minHeight: 44)
                .background(Color(.tertiarySystemBackground))
                .clipShape(
                    UnevenRoundedRectangle(
                        topLeadingRadius: 8,
                        bottomLeadingRadius: state.isExpanded ? 0 : 8,
                        bottomTrailingRadius: state.isExpanded ? 0 : 8,
                        topTrailingRadius: 8
                    )
                )
            }
            .buttonStyle(.plain)
            .accessibilityLabel("\(state.name), \(accessibilityStatus)")
            .accessibilityValue(accessibilityValue)
            .accessibilityHint("Double tap to view details")

            // Expanded detail area
            if state.isExpanded {
                ToolCallDetailView(state: state)
                    .padding(12)
                    .background(Color(.secondarySystemBackground))
                    .clipShape(
                        UnevenRoundedRectangle(
                            topLeadingRadius: 0,
                            bottomLeadingRadius: 8,
                            bottomTrailingRadius: 8,
                            topTrailingRadius: 0
                        )
                    )
            }
        }
    }

    @ViewBuilder
    private var statusIcon: some View {
        switch state.status {
        case .running:
            ProgressView()
                .frame(width: 14, height: 14)
                .tint(Color(.systemOrange))
        case .completed:
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 16))
                .foregroundStyle(Color(.systemGreen))
        case .error:
            Image(systemName: "xmark.circle.fill")
                .font(.system(size: 16))
                .foregroundStyle(Color.red)
        case .timedOut:
            Image(systemName: "clock.badge.questionmark")
                .font(.system(size: 16))
                .foregroundStyle(Color(.secondaryLabel))
        }
    }

    private var accessibilityStatus: String {
        switch state.status {
        case .running: return "Running"
        case .completed: return "Completed"
        case .error: return "Error"
        case .timedOut: return "Timed out"
        }
    }

    private var accessibilityValue: String {
        switch state.status {
        case .running: return "In progress"
        case .completed:
            if let ms = state.durationMs {
                return "Completed in \(ms.formattedDuration)"
            }
            return "Completed"
        case .error: return state.errorMessage ?? "Error occurred"
        case .timedOut: return "Timed out after 5 minutes"
        }
    }
}

/// Groups multiple tool call indicators vertically with 4pt spacing.
struct ToolCallGroupView: View {
    let toolCalls: [ToolCallState]

    var body: some View {
        if !toolCalls.isEmpty {
            VStack(spacing: 4) {
                ForEach(toolCalls) { state in
                    ToolCallIndicatorView(state: state)
                }
            }
            .padding(.horizontal, 16)
        }
    }
}
