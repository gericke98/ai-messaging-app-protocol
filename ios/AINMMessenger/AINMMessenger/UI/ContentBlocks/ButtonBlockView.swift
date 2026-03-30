import SwiftUI

/// Renders a ButtonBlock with primary/secondary/danger styles and optional confirmation dialog.
/// Per UI-SPEC Component 5: full-width, 44pt height, 10pt corner radius, semantic styles.
/// Action callback wired in Plan 04; defaults to nil for display-only rendering.
struct ButtonBlockView: View {
    let content: ContentBlock.ButtonContent
    var onAction: ((_ actionId: String, _ data: [String: Any]) -> Void)? = nil
    @State private var isSending = false
    @State private var showConfirmation = false

    private var isDisabled: Bool {
        content.disabled == true || isSending
    }

    private var backgroundColor: Color {
        if isDisabled {
            return Color(.tertiarySystemFill)
        }
        switch content.style ?? "primary" {
        case "secondary":
            return Color(.secondarySystemFill)
        case "danger":
            return Color.red
        default: // "primary"
            return Color.accentColor
        }
    }

    private var foregroundColor: Color {
        if isDisabled {
            return Color(.tertiaryLabel)
        }
        switch content.style ?? "primary" {
        case "secondary":
            return Color(.label)
        default: // "primary", "danger"
            return Color.white
        }
    }

    var body: some View {
        Button {
            if content.confirmation != nil {
                showConfirmation = true
            } else {
                performAction()
            }
        } label: {
            Group {
                if isSending {
                    ProgressView()
                        .frame(width: 14, height: 14)
                        .tint(foregroundColor)
                } else {
                    Text(content.label)
                        .font(.body)
                        .fontWeight(.bold)
                }
            }
            .frame(maxWidth: .infinity)
            .frame(height: 44)
            .foregroundStyle(foregroundColor)
            .background(backgroundColor)
            .clipShape(RoundedRectangle(cornerRadius: 10))
        }
        .disabled(isDisabled)
        .confirmationDialog(
            content.confirmation ?? "",
            isPresented: $showConfirmation,
            titleVisibility: .visible
        ) {
            Button(content.label) {
                performAction()
            }
            Button("Go Back", role: .cancel) {}
        }
        .accessibilityLabel(content.label)
        .accessibilityHint("Double tap to \(content.label)")
        .accessibilityAddTraits(isDisabled ? [] : .isButton)
    }

    private func performAction() {
        isSending = true
        onAction?(content.actionId, [:])
    }
}
