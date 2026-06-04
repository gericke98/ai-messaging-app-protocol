import SwiftUI

/// Face-ID unlock gate shown on resume when `sessionPhase == .locked` (the user enrolled with
/// `faceIDEnabled`). `serverURL` / `displayName` are already restored by `AppState.bootstrap()`,
/// so a successful biometric check just connects and advances to `.active`. On failure the user
/// can retry the prompt or sign out (returning to onboarding).
struct SessionGateView: View {
    @Bindable var appState: AppState
    @State private var failed = false
    @State private var working = true

    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: "lock.shield")
                .font(.system(size: 48))
                .foregroundStyle(.secondary)
            Text("Unlock AINM")
                .font(.title2.bold())
            if working {
                ProgressView()
            }
            if failed {
                Button("Try Again") { Task { await unlock() } }
                    .buttonStyle(.borderedProminent)
                Button("Sign Out", role: .destructive) { appState.signOut() }
            }
        }
        .padding()
        .task { await unlock() }
    }

    private func unlock() async {
        failed = false
        working = true
        do {
            try await BiometricManager.authenticate(reason: "Unlock AINM")
            await appState.connect()
            appState.sessionPhase = .active
        } catch {
            working = false
            failed = true
        }
    }
}
