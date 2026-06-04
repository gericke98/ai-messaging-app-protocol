import SwiftUI

/// Conversation List screen (Screen 1 per UI-SPEC).
/// Displays threads from GRDB with real-time observation, empty state with CTA,
/// connection status subtitle, unread dot badges, and navigation to thread detail.
struct ConversationListView: View {
    @Bindable var appState: AppState
    @State private var viewModel: ConversationListViewModel?
    @State private var showMemoryGraph = false
    @State private var showAssistantChatError = false
    @State private var showAccountSwitcher = false

    var body: some View {
        VStack(spacing: 0) {
            reconnectingBanner
            contentView
        }
            .navigationTitle("Conversations")
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    ConnectionStatusView(connectionState: appState.connectionState)
                }
                ToolbarItem(placement: .topBarTrailing) {
                    HStack(spacing: 12) {
                        Button {
                            showAccountSwitcher = true
                        } label: {
                            Image(systemName: "person.crop.circle")
                                .accessibilityLabel("Accounts")
                        }
                        Button {
                            showMemoryGraph = true
                        } label: {
                            Image(systemName: "brain.head.profile")
                                .accessibilityLabel("Memory graph")
                        }
                        NavigationLink {
                            AgentDiscoveryView(appState: appState)
                        } label: {
                            Image(systemName: "person.2.badge.gearshape")
                                .accessibilityLabel("Discover agents")
                        }
                        Button {
                            Task {
                                if let threadId = await appState.startAssistantThread() {
                                    appState.selectedThreadId = threadId
                                } else {
                                    showAssistantChatError = true
                                }
                            }
                        } label: {
                            Image(systemName: "sparkles")
                                .accessibilityLabel("Chat with Assistant")
                        }
                        .disabled(appState.connectionState != .connected)
                        Button {
                            appState.showNewThreadComposer = true
                        } label: {
                            Image(systemName: "square.and.pencil")
                                .accessibilityLabel("New conversation")
                        }
                    }
                }
            }
            .alert("Couldn't start assistant chat", isPresented: $showAssistantChatError) {
                Button("OK", role: .cancel) {}
            } message: {
                Text("Please make sure you're connected and try again.")
            }
            .navigationDestination(item: $appState.selectedThreadId) { threadId in
                MessageThreadView(appState: appState, threadId: threadId)
            }
            .onAppear {
                if viewModel == nil, let db = appState.appDatabase {
                    let repo = ThreadRepository(dbPool: db.dbPool)
                    let vm = ConversationListViewModel(repository: repo)
                    self.viewModel = vm
                    vm.startObserving()
                }
            }
            .onDisappear {
                viewModel?.stopObserving()
            }
            .sheet(isPresented: $appState.showNewThreadComposer) {
                NewThreadComposerView(appState: appState)
            }
            .sheet(isPresented: $showMemoryGraph) {
                MemoryGraphView(appState: appState)
            }
            .sheet(isPresented: $showAccountSwitcher) {
                AccountSwitcherView(appState: appState)
            }
    }

    /// Thin, non-blocking banner shown while the client is re-establishing a dropped
    /// connection. Tied to `connectionState` only — `sessionPhase` stays `.active`, so the
    /// list and its data remain visible and interactive underneath.
    @ViewBuilder
    private var reconnectingBanner: some View {
        if appState.sessionPhase == .active, appState.connectionState == .connecting {
            HStack(spacing: 8) {
                ProgressView()
                    .controlSize(.small)
                Text("Reconnecting…")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 6)
            .background(.thinMaterial)
            .accessibilityElement(children: .combine)
            .accessibilityLabel("Reconnecting")
            .transition(.move(edge: .top).combined(with: .opacity))
        }
    }

    @ViewBuilder
    private var contentView: some View {
        if let viewModel, !viewModel.conversations.isEmpty {
            List {
                ForEach(viewModel.conversations, id: \.id) { conversation in
                    Button {
                        appState.selectedThreadId = conversation.id
                    } label: {
                        ConversationRowView(
                            conversation: conversation,
                            previewText: viewModel.previewText(for: conversation),
                            relativeTime: viewModel.relativeTimestamp(
                                for: conversation.lastMessageTimestamp ?? conversation.createdAt
                            )
                        )
                    }
                    .buttonStyle(.plain)
                    .listRowInsets(EdgeInsets())
                    .listRowSeparator(.visible)
                    .accessibilityIdentifier(UITestID.ConversationList.cell(threadId: conversation.id))
                }
            }
            .listStyle(.plain)
            .refreshable {
                // Pull-to-refresh: in write-through cache model, the list
                // auto-updates from GRDB observation. Server re-sync is a
                // future enhancement.
            }
        } else if let viewModel, viewModel.isLoading {
            ProgressView()
        } else {
            emptyStateView
        }
    }

    /// Empty state per UI-SPEC D-07
    private var emptyStateView: some View {
        VStack(spacing: 16) {
            Spacer()
            Image(systemName: "bubble.left.and.text.bubble.right")
                .font(.system(size: 60))
                .foregroundStyle(Color.accentColor)
            Text("No conversations yet")
                .font(.title2.bold())
            Text("Start a conversation with humans or AI agents")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
            Button {
                appState.showNewThreadComposer = true
            } label: {
                Text("Start a Conversation")
                    .frame(maxWidth: .infinity)
                    .frame(height: 44)
            }
            .buttonStyle(.borderedProminent)
            .padding(.horizontal, 32)
            Spacer()
        }
    }
}
