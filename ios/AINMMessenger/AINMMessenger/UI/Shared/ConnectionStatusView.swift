import SwiftUI

/// Subtle connection status indicator shown as navigation bar subtitle.
/// Per UI-SPEC D-13: green Connected (fades after 5s), yellow Reconnecting..., red Offline.
struct ConnectionStatusView: View {
    let connectionState: AppState.ConnectionDisplayState
    @State private var isVisible: Bool = true
    @State private var fadeTask: Task<Void, Never>?

    var body: some View {
        Group {
            switch connectionState {
            case .connected:
                if isVisible {
                    statusLabel(text: "Connected", color: .green, systemName: "circle.fill")
                }
            case .connecting:
                statusLabel(text: "Reconnecting...", color: .yellow, systemName: "circle.fill")
            case .disconnected, .error:
                statusLabel(text: "Offline", color: .red, systemName: "circle.fill")
            }
        }
        .onChange(of: connectionState) { oldValue, newValue in
            fadeTask?.cancel()
            if newValue == .connected {
                isVisible = true
                fadeTask = Task {
                    try? await Task.sleep(for: .seconds(5))
                    guard !Task.isCancelled else { return }
                    withAnimation(.easeOut(duration: 0.3)) {
                        isVisible = false
                    }
                }
            } else {
                isVisible = true
            }
        }
    }

    private func statusLabel(text: String, color: Color, systemName: String) -> some View {
        HStack(spacing: 4) {
            Image(systemName: systemName)
                .font(.system(size: 6))
                .foregroundStyle(color)
            Text(text)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }
}
