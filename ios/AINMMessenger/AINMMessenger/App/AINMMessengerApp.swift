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
                        // Placeholder -- ConversationListView created in Plan 02
                        Text("Conversations")
                            .navigationTitle("Conversations")
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
