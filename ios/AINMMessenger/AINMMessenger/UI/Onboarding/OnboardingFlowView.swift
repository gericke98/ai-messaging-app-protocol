import SwiftUI

/// Multi-step account signup flow.
///
/// Drives the create-account lifecycle that `AppState.startSignup` / `verifyCode` /
/// `finishSignup` orchestrate: collect server + identity, send a verification email,
/// enter the code (verified immediately — consuming the OTP once and holding the returned
/// enrollment token), set a recovery PIN, optionally gate behind Face ID, then run the
/// bind key → enroll escrow → connect pipeline against the held token. On success the root
/// view observes `appState.connectionState == .connected` and swaps to the conversation list.
///
/// Verify is split out from bind/connect so a transient bind/escrow/connect failure retries
/// against the already-held token (never re-verifying the consumed OTP, which would 401).
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
    @State private var enrollmentToken: String = ""
    @State private var pin: String = ""
    @State private var confirmPin: String = ""
    @State private var faceIDChoice: Bool = false
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
            Button {
                Task {
                    busy = true
                    // Verify (and CONSUME) the OTP exactly once here; hold the enrollment
                    // token so later bind/escrow/connect retries never re-verify.
                    if let token = await appState.verifyCode(
                        serverURL: serverURL, email: email, code: code) {
                        enrollmentToken = token
                        step = .pin
                    }
                    // On failure we stay on `.code` (code wrong/expired — the only legitimate
                    // reason to stay here); appState.onboardingError carries the reason.
                    busy = false
                }
            } label: {
                stepLabel("Continue")
            }
            .disabled(code.count < 6 || busy)

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
                    faceIDChoice = true
                    step = .working
                }
            }
            Button("Skip for now") {
                faceIDChoice = false
                step = .working
            }
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
            let ok = await appState.finishSignup(
                serverURL: serverURL,
                username: username,
                enrollmentToken: enrollmentToken,
                pin: pin,
                faceIDEnabled: faceIDChoice
            )
            // On success the root view swaps to ConversationListView automatically.
            // On failure, return to the Face ID step (which leads back to `.working`) so the
            // user retries bind/escrow/connect — NEVER `.code`, since the OTP was already
            // consumed by verifyCode and re-verifying it would 401. onboardingError has the reason.
            if !ok { step = .faceID }
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
