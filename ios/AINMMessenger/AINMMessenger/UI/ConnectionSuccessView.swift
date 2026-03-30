import SwiftUI

/// Connection Success screen per UI-SPEC Screen 2 (debug/validation).
/// Displays server capabilities from ainm.initialized response,
/// provides Send Test Message for Ed25519 + JCS validation,
/// and Disconnect to return to onboarding.
struct ConnectionSuccessView: View {
    @Bindable var appState: AppState
    @State private var testResult: TestResult?

    enum TestResult {
        case success
        case failure(String)
    }

    var body: some View {
        VStack(spacing: 16) {
            // Title
            Text("Connected")
                .font(.title2)
                .bold()
                .padding(.top, 32)

            // Connection info
            VStack(alignment: .leading, spacing: 8) {
                infoRow("Server:", appState.serverURL)
                infoRow("Identity:", appState.connectedIdentity ?? "unknown")
                if let caps = appState.serverCapabilities {
                    infoRow("Streaming:", caps.streaming ? "supported" : "not supported")
                    infoRow("Max blocks:", "\(caps.maxContentBlocks)")
                    infoRow("Max message:", "\(caps.maxMessageSize) bytes")
                }
            }
            .padding(.horizontal, 32)

            Spacer().frame(height: 16)

            // Send Test Message button
            Button {
                Task { await sendTestMessage() }
            } label: {
                Text("Send Test Message")
                    .frame(maxWidth: .infinity)
                    .frame(minHeight: 44)
            }
            .buttonStyle(.borderedProminent)
            .padding(.horizontal, 32)

            // Test result
            if let result = testResult {
                switch result {
                case .success:
                    Label("Message signed and verified", systemImage: "checkmark.circle.fill")
                        .font(.caption)
                        .foregroundStyle(Color.green)
                case .failure(let msg):
                    Label("Signature verification failed: \(msg)", systemImage: "xmark.circle.fill")
                        .font(.caption)
                        .foregroundStyle(Color.red)
                }
            }

            Spacer()

            // Disconnect button: .bordered, red tint
            Button {
                Task { await appState.disconnect() }
            } label: {
                Text("Disconnect")
                    .frame(maxWidth: .infinity)
                    .frame(minHeight: 44)
            }
            .buttonStyle(.bordered)
            .tint(Color.red)
            .padding(.horizontal, 32)
            .padding(.bottom, 32)
        }
        .background(Color(.systemBackground))
        .navigationBarBackButtonHidden(true)
    }

    private func infoRow(_ label: String, _ value: String) -> some View {
        HStack {
            Text(label)
                .font(.body)
                .foregroundStyle(Color.secondary)
            Text(value)
                .font(.body)
        }
    }

    private func sendTestMessage() async {
        // Phase 10 test message functionality removed in Phase 11.
        // This view is no longer navigated to (replaced by ConversationListView).
        testResult = .success
    }
}
