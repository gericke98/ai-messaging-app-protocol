import SwiftUI
import GRDB

@main
struct AINMMessengerApp: App {
    @State private var appState = AppState()
    @State private var showThreadNotFound = false

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
            .onOpenURL { url in
                handleDeepLink(url)
            }
            .alert("Thread Not Found", isPresented: $showThreadNotFound) {
                Button("OK", role: .cancel) { }
            } message: {
                Text("This thread is not available on your device.")
            }
        }
    }

    private func handleDeepLink(_ url: URL) {
        guard let route = DeepLinkHandler.parse(url) else { return }
        switch route {
        case .thread(let threadId):
            Task {
                if await threadExists(threadId) {
                    appState.selectedThreadId = threadId
                } else {
                    showThreadNotFound = true
                }
            }
        case .message(let threadId, let messageId):
            Task {
                if await threadExists(threadId) {
                    appState.scrollToMessageId = messageId
                    appState.selectedThreadId = threadId
                } else {
                    showThreadNotFound = true
                }
            }
        }
    }

    private func threadExists(_ threadId: String) async -> Bool {
        guard let db = appState.appDatabase else { return false }
        do {
            return try await db.dbPool.read { db in
                try ThreadRecord.fetchOne(db, key: threadId) != nil
            }
        } catch {
            return false
        }
    }
}
