import SwiftUI

/// Multi-step account recovery flow.
///
/// Recovery rebuilds the device key from the PIN-guarded escrow and reconnects under the
/// existing identity. The identity is `username@<host-of-serverURL>`, so recovery REQUIRES
/// the username (unlike a password reset) — the handshake cannot be rebuilt without it.
///
/// Each `completeRecovery` attempt consumes one server-side OPRF attempt. After too many
/// wrong PINs the escrow is destroyed and recovery becomes permanently unavailable.
struct RecoveryFlowView: View {
    @Bindable var appState: AppState
    @Environment(\.dismiss) private var dismiss

    private enum Step {
        case identity   // server + email + username
        case code
        case pin
        case working
        case destroyed
    }

    @State private var step: Step = .identity
    @State private var serverURL: String = ""
    @State private var email: String = ""
    @State private var username: String = ""
    @State private var code: String = ""
    @State private var pin: String = ""
    @State private var busy = false
    @State private var wrongPin = false

    var body: some View {
        NavigationStack {
            Form {
                switch step {
                case .identity: identityStep
                case .code: codeStep
                case .pin: pinStep
                case .working: workingStep
                case .destroyed: destroyedStep
                }

                if step != .destroyed, let error = appState.onboardingError {
                    Section {
                        Text(error)
                            .font(.caption)
                            .foregroundStyle(Color.red)
                    }
                }
            }
            .navigationTitle("Recover Account")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
            .onAppear {
                if serverURL.isEmpty { serverURL = appState.serverURL }
            }
        }
    }

    // MARK: - Steps

    @ViewBuilder private var identityStep: some View {
        Section {
            VStack(alignment: .leading, spacing: 4) {
                Text("Server URL").font(.subheadline)
                TextField("wss://localhost:8080", text: $serverURL)
                    .keyboardType(.URL)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                Text("The server URL's host must match the server's domain.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        Section("Your account") {
            TextField("Email", text: $email)
                .keyboardType(.emailAddress)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
            TextField("Username", text: $username)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
        }
        Section {
            Button {
                Task {
                    busy = true
                    let ok = await appState.startRecovery(serverURL: serverURL, email: email)
                    busy = false
                    if ok { step = .code }
                }
            } label: {
                if busy {
                    HStack(spacing: 8) { ProgressView(); Text("Send recovery code") }
                } else {
                    Text("Send recovery code")
                }
            }
            .disabled(serverURL.isEmpty || email.isEmpty || username.isEmpty || busy)
        }
    }

    @ViewBuilder private var codeStep: some View {
        Section("Verification code") {
            Text("Enter the 6-digit code sent to \(email).")
                .font(.caption)
                .foregroundStyle(.secondary)
            TextField("123456", text: $code)
                .keyboardType(.numberPad)
                .textContentType(.oneTimeCode)
        }
        Section {
            Button("Continue") {
                appState.onboardingError = nil
                step = .pin
            }
            .disabled(code.count < 6)

            Button("Back") { step = .identity }
        }
    }

    @ViewBuilder private var pinStep: some View {
        Section("Recovery PIN") {
            SecureField("PIN", text: $pin)
                .keyboardType(.numberPad)
            if wrongPin {
                Text("Incorrect PIN, try again. Each attempt is limited.")
                    .font(.caption)
                    .foregroundStyle(Color.red)
            }
        }
        Section {
            Button("Recover account") {
                wrongPin = false
                appState.onboardingError = nil
                step = .working
            }
            .disabled(pin.count < 4)

            Button("Back") { step = .code }
        }
    }

    @ViewBuilder private var workingStep: some View {
        Section {
            HStack(spacing: 12) {
                ProgressView()
                Text("Recovering your account…")
            }
        }
        .task {
            let outcome = await appState.completeRecovery(
                serverURL: serverURL,
                email: email,
                username: username,
                code: code,
                pin: pin
            )
            switch outcome {
            case .connected:
                // Root view swaps to ConversationListView automatically.
                break
            case .wrongPin:
                wrongPin = true
                pin = ""
                step = .pin
            case .destroyed:
                step = .destroyed
            case .failed:
                // appState.onboardingError carries the reason; let them retry the PIN.
                step = .pin
            }
        }
    }

    @ViewBuilder private var destroyedStep: some View {
        Section {
            Text("Recovery is no longer available (too many attempts).")
                .font(.subheadline)
                .foregroundStyle(Color.red)
        }
        Section {
            Button("Go back") { dismiss() }
        }
    }
}
