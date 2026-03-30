import SwiftUI

/// Message Thread screen (Screen 2 per UI-SPEC).
/// Displays paginated messages with scroll management, grouped by sender,
/// with a composer bar at the bottom.
struct MessageThreadView: View {
    @Bindable var appState: AppState
    let threadId: String
    @State private var viewModel: MessageThreadViewModel?
    @State private var scrollPosition = ScrollPosition(idType: String.self)
    @State private var showMemoryContext = false
    @State private var parentThreadInfo: (parentThreadId: String, parentMessageId: String, senderName: String)?
    @State private var showMessageNotFound = false

    var body: some View {
        VStack(spacing: 0) {
            // Sub-thread banner (pinned, not scrolling)
            if let info = parentThreadInfo {
                SubThreadBannerView(parentSenderName: info.senderName) {
                    appState.scrollToMessageId = info.parentMessageId
                    appState.selectedThreadId = info.parentThreadId
                }
            }

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
                                },
                                isInSubThread: parentThreadInfo != nil,
                                subThreadInfo: { messageId in
                                    viewModel.subThreadInfo(forMessageId: messageId)
                                },
                                onNavigateToSubThread: { subThreadId in
                                    appState.selectedThreadId = subThreadId
                                },
                                onBranchThread: { messageId in
                                    Task { await branchThread(fromMessageId: messageId) }
                                }
                            )
                        }

                        // Tool call indicators (below permanent messages, above streams)
                        ToolCallGroupView(
                            toolCalls: appState.toolCalls(forThread: threadId)
                                .filter { $0.status == .running }
                        )

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
            ToolbarItem(placement: .topBarTrailing) {
                if viewModel?.hasAgentParticipant == true {
                    Button {
                        showMemoryContext = true
                    } label: {
                        HStack(spacing: 4) {
                            Image(systemName: "brain")
                            Text("Context")
                                .font(.caption.bold())
                        }
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background(Color.accentColor.opacity(0.12))
                        .foregroundStyle(Color.accentColor)
                        .clipShape(RoundedRectangle(cornerRadius: 6))
                    }
                    .accessibilityLabel("View memory context")
                }
            }
        }
        .sheet(isPresented: $showMemoryContext) {
            MemoryContextView(threadId: threadId, appState: appState)
                .presentationDetents([.medium, .large])
        }
        .overlay(alignment: .bottom) {
            if showMessageNotFound {
                Text("Message not found")
                    .font(.subheadline)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                    .background(.ultraThinMaterial)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
                    .padding(.bottom, 80)
                    .transition(.opacity)
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
                Task {
                    await vm.loadInitial()
                    await loadParentThreadInfo(database: db)
                    handleScrollToMessage()
                }
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

    /// Branch a new sub-thread from the given message.
    private func branchThread(fromMessageId messageId: String) async {
        do {
            let subThreadId = try await appState.createSubThread(
                parentThreadId: threadId,
                parentMessageId: messageId
            )
            appState.selectedThreadId = subThreadId
        } catch {
            // Silently fail -- user will see no navigation
        }
    }

    /// Load parent thread info for sub-thread banner display.
    private func loadParentThreadInfo(database: AppDatabase) async {
        do {
            let info: (String, String, String)? = try await database.dbPool.read { db in
                guard let thread = try ThreadRecord.fetchOne(db, key: self.threadId),
                      let parentThreadId = thread.parentThreadId,
                      let parentMessageId = thread.parentMessageId else {
                    return nil
                }
                // Look up the parent message sender name
                let senderName: String
                if let parentMessage = try MessageRecord.fetchOne(db, key: parentMessageId) {
                    senderName = parentMessage.fromAddress.components(separatedBy: "@").first ?? parentMessage.fromAddress
                } else {
                    senderName = thread.createdBy.components(separatedBy: "@").first ?? "Unknown"
                }
                return (parentThreadId, parentMessageId, senderName)
            }
            if let info {
                parentThreadInfo = (parentThreadId: info.0, parentMessageId: info.1, senderName: info.2)
            }
        } catch {
            // Non-critical -- banner just won't show
        }
    }

    /// Handle scroll-to-message from deep links.
    private func handleScrollToMessage() {
        guard let messageId = appState.scrollToMessageId else { return }
        appState.scrollToMessageId = nil
        // Check if the message exists in loaded groups
        let messageExists = viewModel?.messageGroups.contains { group in
            group.messages.contains { $0.id == messageId }
        } ?? false
        if messageExists {
            scrollPosition.scrollTo(id: messageId, anchor: .center)
        } else {
            withAnimation {
                showMessageNotFound = true
            }
            Task {
                try? await Task.sleep(for: .seconds(3))
                withAnimation {
                    showMessageNotFound = false
                }
            }
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
