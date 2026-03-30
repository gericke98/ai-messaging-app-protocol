import SwiftUI

/// Message composer bar per UI-SPEC Screen 2a (D-12).
/// Auto-growing text field with send button, disabled when offline.
struct ComposerView: View {
    var isOffline: Bool = false
    var onSend: (String) -> Void

    @State private var text: String = ""

    private var canSend: Bool {
        !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !isOffline
    }

    var body: some View {
        HStack(alignment: .bottom, spacing: 8) {
            TextField(
                isOffline ? "Offline -- messages will send when reconnected" : "Message...",
                text: $text,
                axis: .vertical
            )
            .lineLimit(1...4)
            .font(.body)
            .disabled(isOffline)

            Button {
                let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !trimmed.isEmpty else { return }
                onSend(trimmed)
                text = ""
            } label: {
                Image(systemName: "arrow.up.circle.fill")
                    .font(.system(size: 32))
                    .foregroundStyle(canSend ? Color.accentColor : .secondary)
            }
            .disabled(!canSend)
            .accessibilityLabel("Send message")
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(Color(.systemBackground))
        .overlay(alignment: .top) {
            Divider()
        }
    }
}
