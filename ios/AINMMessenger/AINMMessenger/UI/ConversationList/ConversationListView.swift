import SwiftUI

/// Conversation List screen (Screen 1 per UI-SPEC).
/// Displays threads from GRDB with real-time observation, empty state with CTA,
/// connection status subtitle, unread dot badges, and navigation to thread detail.
struct ConversationListView: View {
    @Bindable var appState: AppState
    @State private var viewModel: ConversationListViewModel?
    @State private var showMemoryGraph = false

    var body: some View {
        contentView
            .navigationTitle("Conversations")
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    ConnectionStatusView(connectionState: appState.connectionState)
                }
                ToolbarItem(placement: .topBarTrailing) {
                    HStack(spacing: 12) {
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
                            appState.showNewThreadComposer = true
                        } label: {
                            Image(systemName: "square.and.pencil")
                                .accessibilityLabel("New conversation")
                        }
                    }
                }
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
