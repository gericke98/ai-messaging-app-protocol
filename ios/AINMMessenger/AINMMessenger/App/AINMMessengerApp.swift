import SwiftUI

@main
struct AINMMessengerApp: App {
    @State private var appState = AppState()

    var body: some Scene {
        WindowGroup {
            NavigationStack {
                Group {
                    switch appState.connectionState {
                    case .connected:
                        ConversationListView(appState: appState)
                    default:
                        OnboardingView(appState: appState)
                    }
                }
            }
            .onAppear {
                try? appState.setup()
            }
        }
    }
}
