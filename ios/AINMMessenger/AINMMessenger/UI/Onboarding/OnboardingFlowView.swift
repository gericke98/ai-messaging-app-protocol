import SwiftUI

/// Multi-step account signup flow.
///
/// Drives the create-account lifecycle that `AppState.startSignup` / `completeSignup`
/// orchestrate: collect server + identity, send a verification email, enter the code,
/// set a recovery PIN, optionally gate behind Face ID, then run the
/// verify → bind key → enroll escrow → connect pipeline. On success the root view
/// observes `appState.connectionState == .connected` and swaps to the conversation list.
///
/// Identity correctness: the WebSocket handshake builds `username@<host-of-serverURL>`
/// and the server bound the account to `username@<server_domain>`. The two must match,
/// hence the caption under the Server URL field.
struct OnboardingFlowView: View {
    @Bindable var appState: AppState

    private enum Step {
        case intro
        case identity   // email + username on one screen (signup needs both)
        case code
        case pin
        case faceID
        case working
    }

    @State private var step: Step = .intro
    @State private var serverURL: String = ""
    @State private var email: String = ""
    @State private var username: String = ""
    @State private var code: String = ""
    @State private var pin: String = ""
    @State private var confirmPin: String = ""
    @State private var showRecovery = false
    @State private var busy = false

    var body: some View {
        NavigationStack {
            Form {
                switch step {
                case .intro: introStep
                case .identity: identityStep
                case .code: codeStep
                case .pin: pinStep
                case .faceID: faceIDStep
                case .working: workingStep
                }

                if let error = appState.onboardingError {
                    Section {
                        Text(error)
                            .font(.caption)
                            .foregroundStyle(Color.red)
                    }
                }
            }
            .navigationTitle("Create Account")
            .navigationBarTitleDisplayMode(.inline)
            .sheet(isPresented: $showRecovery) {
                RecoveryFlowView(appState: appState)
            }
            .onAppear {
                if serverURL.isEmpty { serverURL = appState.serverURL }
            }
        }
    }

    // MARK: - Steps

    @ViewBuilder private var introStep: some View {
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
        Section {
            Button("Create account") {
                appState.onboardingError = nil
                step = .identity
            }
            .disabled(serverURL.isEmpty)

            Button("Recover an existing account") {
                appState.onboardingError = nil
                showRecovery = true
            }
        }
    }

    @ViewBuilder private var identityStep: some View {
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
                    let ok = await appState.startSignup(
                        serverURL: serverURL, email: email, username: username)
                    busy = false
                    if ok { step = .code }
                }
            } label: {
                stepLabel("Send code")
            }
            .disabled(email.isEmpty || username.isEmpty || busy)

            Button("Back") { step = .intro }
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
            Text("Your PIN protects account recovery. Choose at least 4 digits.")
                .font(.caption)
                .foregroundStyle(.secondary)
            SecureField("PIN", text: $pin)
                .keyboardType(.numberPad)
            SecureField("Confirm PIN", text: $confirmPin)
                .keyboardType(.numberPad)
            if !confirmPin.isEmpty && pin != confirmPin {
                Text("PINs do not match.")
                    .font(.caption)
                    .foregroundStyle(Color.red)
            }
        }
        Section {
            Button("Continue") {
                appState.onboardingError = nil
                step = .faceID
            }
            .disabled(!pinValid)

            Button("Back") { step = .code }
        }
    }

    @ViewBuilder private var faceIDStep: some View {
        Section("Face ID") {
            Text("Enable Face ID for quick, secure access. This is optional.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        Section {
            Button("Enable Face ID") {
                Task {
                    // Best-effort: failure or unavailability is non-fatal — proceed either way.
                    try? await BiometricManager.authenticate(reason: "Confirm it's you")
                    step = .working
                }
            }
            Button("Skip for now") { step = .working }
            Button("Back") { step = .pin }
        }
    }

    @ViewBuilder private var workingStep: some View {
        Section {
            HStack(spacing: 12) {
                ProgressView()
                Text("Setting up your account…")
            }
        }
        .task {
            let ok = await appState.completeSignup(
                serverURL: serverURL,
                email: email,
                username: username,
                code: code,
                pin: pin
            )
            // On success the root view swaps to ConversationListView automatically.
            // On failure, return to the code step so the user can correct and retry;
            // appState.onboardingError carries the reason.
            if !ok { step = .code }
        }
    }

    // MARK: - Helpers

    @ViewBuilder private func stepLabel(_ title: String) -> some View {
        if busy {
            HStack(spacing: 8) {
                ProgressView()
                Text(title)
            }
        } else {
            Text(title)
        }
    }

    private var pinValid: Bool {
        pin.count >= 4 && pin == confirmPin
    }
}
