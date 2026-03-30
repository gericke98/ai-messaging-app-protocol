import SwiftUI

/// Message Thread screen (Screen 2 per UI-SPEC).
/// Displays paginated messages with scroll management, grouped by sender,
/// with a composer bar at the bottom.
struct MessageThreadView: View {
    @Bindable var appState: AppState
    let threadId: String
    @State private var viewModel: MessageThreadViewModel?
    @State private var scrollPosition = ScrollPosition(idType: String.self)

    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                LazyVStack(spacing: 16) {
                    if let viewModel {
                        if viewModel.isLoadingMore {
                            ProgressView("Loading older messages...")
                                .padding()
                        }

                        ForEach(viewModel.messageGroups) { group in
                            MessageGroupView(
                                group: group,
                                contentBlocksProvider: { viewModel.contentBlocks(for: $0) },
                                timestampFormatter: { viewModel.formatTimestamp($0) },
                                onAction: { messageId, blockIndex, actionId, data in
                                    handleAction(messageId: messageId, blockIndex: blockIndex, actionId: actionId, data: data)
                                }
                            )
                        }

                        // Streaming messages (ephemeral, below permanent messages)
                        ForEach(appState.streamingViewModel.activeStreamsForThread(threadId)) { stream in
                            StreamingMessageView(state: stream)
                        }

                        // Typing/processing indicators from active presence
                        ForEach(appState.presenceEntries(forThread: threadId)) { entry in
                            TypingIndicatorView(name: entry.name, isAgent: entry.isAgent)
                        }
                    }
                }
                .padding(.vertical, 8)
            }
            .scrollPosition($scrollPosition)
            .defaultScrollAnchor(.bottom)
            .onScrollGeometryChange(for: Bool.self) { geometry in
                geometry.contentOffset.y <= 20
            } action: { _, isNearTop in
                if isNearTop {
                    Task { await viewModel?.loadOlderMessages() }
                }
            }

            // Stop Generating button (visible only when streams are active)
            if appState.streamingViewModel.hasActiveStreams(inThread: threadId) {
                StopGeneratingButton(
                    isStopping: appState.streamingViewModel.activeStreamsForThread(threadId)
                        .contains { $0.status == .cancelling },
                    onStop: {
                        Task { await appState.cancelStreams(inThread: threadId) }
                    }
                )
                .frame(maxWidth: .infinity)
                .padding(.vertical, 4)
            }

            ComposerView(
                isOffline: appState.connectionState != .connected,
                onSend: { text in
                    Task { await sendMessage(text: text) }
                }
            )
        }
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .principal) {
                VStack(spacing: 2) {
                    Text(viewModel?.threadTitle ?? "Conversation")
                        .font(.subheadline.bold())
                    if let count = viewModel?.participantCount, count > 0 {
                        Text("\(count) participants")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
        .onAppear {
            appState.currentViewingThreadId = threadId
            if viewModel == nil, let db = appState.appDatabase {
                let vm = MessageThreadViewModel(
                    threadId: threadId,
                    currentUserIdentity: appState.connectedIdentity ?? "",
                    database: db
                )
                vm.streamingViewModel = appState.streamingViewModel
                self.viewModel = vm
                Task { await vm.loadInitial() }
            }
        }
        .onDisappear {
            appState.currentViewingThreadId = nil
            viewModel?.stopObserving()
        }
    }

    /// Handle interactive block action from ContentBlockRenderer onAction chain.
    /// Routes: MessageThreadView -> MessageGroupView -> AgentMessageView -> ContentBlockRenderer -> block view.
    private func handleAction(messageId: String, blockIndex: Int, actionId: String, data: [String: Any]) {
        let fallback = "Action: \(actionId)"
        Task {
            try? await appState.sendActionBlock(
                refMessageId: messageId,
                refBlockIndex: blockIndex,
                actionId: actionId,
                data: data,
                fallbackText: fallback,
                threadId: threadId
            )
        }
    }

    private func sendMessage(text: String) async {
        guard let identity = appState.connectedIdentity else { return }
        let envelope = MessageEnvelope(
            id: UUID().uuidString,
            from: identity,
            threadId: threadId,
            timestamp: ISO8601DateFormatter.ainm.string(from: Date()),
            type: .message,
            content: [.text(ContentBlock.TextContent(
                version: "1.0",
                source: "user",
                text: text,
                mentions: nil,
                fallback: text
            ))],
            signature: nil,
            parentId: nil
        )
        try? await appState.sendMessage(envelope: envelope)
    }
}
