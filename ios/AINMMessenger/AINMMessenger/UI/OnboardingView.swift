import SwiftUI

/// Onboarding screen per UI-SPEC Screen 1.
/// User enters server URL and display name, then taps Connect to initiate
/// Ed25519 keypair generation, WebSocket connection, and AINM handshake.
struct OnboardingView: View {
    @Bindable var appState: AppState

    var body: some View {
        VStack(spacing: 0) {
            Spacer()

            // App icon placeholder
            Image(systemName: "bubble.left.and.bubble.right.fill")
                .font(.system(size: 60))
                .foregroundStyle(Color.accentColor)
                .padding(.bottom, 16)  // md spacing

            // Title: .title2, bold, centered (per UI-SPEC)
            Text("AI-Native Messenger")
                .font(.title2)
                .bold()
                .padding(.bottom, 32)  // xl spacing

            // Form fields
            VStack(spacing: 16) {  // md spacing between fields
                VStack(alignment: .leading, spacing: 4) {  // xs spacing
                    Text("Server URL")
                        .font(.subheadline)
                    TextField("wss://localhost:8080", text: $appState.serverURL)
                        .keyboardType(.URL)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .textFieldStyle(.roundedBorder)
                        .disabled(isConnecting)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Display Name")
                        .font(.subheadline)
                    TextField("Alice", text: $appState.displayName)
                        .textInputAutocapitalization(.words)
                        .textFieldStyle(.roundedBorder)
                        .disabled(isConnecting)
                }
            }
            .padding(.horizontal, 32)  // xl horizontal padding

            // 24pt gap before button (lg spacing per UI-SPEC)
            Spacer().frame(height: 24)

            // Connect button: .borderedProminent, 44pt min height
            Button {
                Task { await appState.connect() }
            } label: {
                HStack {
                    if isConnecting {
                        ProgressView()
                            .tint(.white)
                        Text("Connecting...")
                    } else {
                        Text("Connect")
                    }
                }
                .frame(maxWidth: .infinity)
                .frame(minHeight: 44)  // Apple HIG touch target
            }
            .buttonStyle(.borderedProminent)
            .disabled(!canConnect)
            .padding(.horizontal, 32)
            .accessibilityLabel("Connect to server")

            // Inline error message: .caption, Color.red (per UI-SPEC)
            if let error = appState.errorMessage {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(Color.red)
                    .padding(.top, 8)
                    .padding(.horizontal, 32)
            }

            Spacer()
        }
        .background(Color(.systemBackground))
        .navigationDestination(isPresented: isConnectedBinding) {
            ConnectionSuccessView(appState: appState)
        }
    }

    private var isConnecting: Bool {
        appState.connectionState == .connecting
    }

    private var canConnect: Bool {
        !appState.serverURL.isEmpty &&
        !appState.displayName.isEmpty &&
        !isConnecting
    }

    private var isConnectedBinding: Binding<Bool> {
        Binding(
            get: { appState.connectionState == .connected },
            set: { if !$0 { /* handled by disconnect */ } }
        )
    }
}
