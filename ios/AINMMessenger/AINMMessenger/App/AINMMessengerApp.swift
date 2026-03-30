import SwiftUI

@main
struct AINMMessengerApp: App {
    @State private var appState = AppState()

    var body: some Scene {
        WindowGroup {
            NavigationStack {
                OnboardingView(appState: appState)
            }
            .onAppear {
                try? appState.setup()
            }
        }
    }
}
