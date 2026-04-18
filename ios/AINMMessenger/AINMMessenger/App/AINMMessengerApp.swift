import SwiftUI
import GRDB

@main
struct AINMMessengerApp: App {
    @State private var appState = AppState()
    @State private var showThreadNotFound = false
    @State private var bootstrapComplete = false

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
            .overlay(alignment: .topLeading) {
                // Invisible 0-size readiness marker (RESEARCH.md "App readiness signal").
                // XCUITest waits for `UITestID.App.ready` before asserting.
                // `accessibilityHidden(true)` keeps VoiceOver quiet while XCUITest queries
                // on otherElements[...] still see the element.
                if bootstrapComplete {
                    Color.clear
                        .frame(width: 0, height: 0)
                        .accessibilityElement()
                        .accessibilityIdentifier(UITestID.App.ready)
                        .accessibilityHidden(true)
                }
            }
            .task {
                #if DEBUG
                // ---- Phase 14 UAT bootstrap: launch-arg parsing ----
                let args = ProcessInfo.processInfo.arguments
                let seedFixture: String? = {
                    guard let idx = args.firstIndex(of: "-UITestSeed"),
                          idx + 1 < args.count else { return nil }
                    return args[idx + 1]
                }()
                let eventScript: String? = {
                    guard let idx = args.firstIndex(of: "-UITestEvents"),
                          idx + 1 < args.count else { return nil }
                    return args[idx + 1]
                }()

                // Step 1 + 2: Filled by 14-03 (SeedLoader + UITestIdentityBootstrap).
                // Must run BEFORE appState.setup() so the DB file is wiped and the Keychain
                // identity is deterministic before AppDatabase.makeDefault() opens the pool
                // and AINMClient reads the keypair.
                // TODO(14-03): if let seedFixture {
                //     try? SeedLoader.preparePristineDatabase()
                //     try? await UITestIdentityBootstrap.resetIdentity()
                // }
                _ = seedFixture // silence unused warning until 14-03 fills this
                #endif

                try? appState.setup()

                #if DEBUG
                // Step 3: Filled by 14-03 (load fixture rows into GRDB after pool is open).
                // TODO(14-03): if let seedFixture {
                //     try? await SeedLoader.loadFixtureIfRequested(fixture: seedFixture, into: appState)
                // }

                // Step 4: TestEventInjector fires scripted ms-offset events (tool-call
                // lifecycle + context-frame push) through the same @MainActor handlers
                // AINMClient.onToolEvent / onContextFrame invoke in production.
                if let eventScript {
                    await TestEventInjector.scheduleIfRequested(script: eventScript, for: appState)
                }
                #endif

                bootstrapComplete = true
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
