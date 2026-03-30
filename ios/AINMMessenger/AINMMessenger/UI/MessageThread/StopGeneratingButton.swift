import SwiftUI

/// Cancel button shown above the composer when agent streams are active (UI-SPEC Component 3).
/// Capsule-shaped, 36pt height, shows "Stop Generating" or "Stopping..." when cancellation is in progress.
struct StopGeneratingButton: View {
    var isStopping: Bool = false
    var onStop: () -> Void

    var body: some View {
        Button(action: onStop) {
            HStack(spacing: 6) {
                Image(systemName: "stop.fill")
                    .font(.system(size: 12))
                Text(isStopping ? "Stopping..." : "Stop Generating")
                    .font(.subheadline.bold())
            }
            .foregroundStyle(Color(.label))
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .frame(height: 36)
            .background(Color(.secondarySystemBackground))
            .clipShape(Capsule())
            .overlay(Capsule().stroke(Color(.separator), lineWidth: 1))
        }
        .disabled(isStopping)
        .accessibilityLabel("Stop generating response")
    }
}
