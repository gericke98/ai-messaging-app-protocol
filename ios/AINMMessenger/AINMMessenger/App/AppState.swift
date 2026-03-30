import Foundation
import Observation

/// Root observable state for the AINM Messenger app.
/// Drives UI with connection lifecycle, server capabilities, and error state.
/// Uses @Observable per research decision D-05 (modern Swift patterns, iOS 17+).
@MainActor
@Observable
final class AppState {
    var connectionState: ConnectionDisplayState = .disconnected
    var serverURL: String = "wss://localhost:8080"
    var displayName: String = ""
    var serverCapabilities: ServerCapabilities?
    var errorMessage: String?
    var connectedIdentity: String?
    var selectedThreadId: String?
    var showNewThreadComposer: Bool = false
    var currentViewingThreadId: String?
    var scrollToMessageId: String?

    /// Observable streaming state bridging AINMClient notifications to SwiftUI.
    var streamingViewModel = StreamingViewModel()

    // MARK: - Presence Tracking

    /// Represents an active typing/processing indicator for a participant.
    struct PresenceEntry: Identifiable {
        let id: String  // identity
        let name: String
        let isAgent: Bool
        let threadId: String?
        var lastUpdated: Date = Date()
    }

    /// Active presence indicators keyed by identity.
    var activePresence: [String: PresenceEntry] = [:]
    private var presenceCleanupTask: Task<Void, Never>?

    // MARK: - Memory Context

    /// Context frames keyed by threadId, updated from server push notifications.
    var contextFrames: [String: ContextFrame] = [:]

    // MARK: - Tool Call State

    /// Tool call states keyed by threadId, each containing an array of ToolCallState per call_id.
    var toolCallStates: [String: [ToolCallState]] = [:]
    private var toolCallTimeoutTasks: [String: Task<Void, Never>] = [:]

    enum ConnectionDisplayState: Sendable {
        case disconnected
        case connecting
        case connected
        case error
    }

    private var client: AINMClient?
    var appDatabase: AppDatabase?

    /// Initialize the database and protocol client.
    /// Call once at app launch before any network operations.
    func setup() throws {
        let db = try AppDatabase.makeDefault()
        self.appDatabase = db
        let client = AINMClient(appDatabase: db)
        self.client = client

        // Wire streaming callbacks from AINMClient to StreamingViewModel
        Task {
            await client.setStreamCallbacks(
                onStart: { [weak self] stream in
                    Task { @MainActor in
                        self?.streamingViewModel.streamStarted(stream)
                    }
                },
                onDelta: { [weak self] streamId, blockIndex, content in
                    Task { @MainActor in
                        self?.streamingViewModel.updateStreamContent(
                            streamId: streamId,
                            blockIndex: blockIndex,
                            content: content
                        )
                    }
                },
                onEnd: { [weak self] stream, status in
                    Task { @MainActor in
                        self?.streamingViewModel.streamEnded(
                            streamId: stream.streamId,
                            status: status
                        )
                    }
                },
                onError: { [weak self] stream in
                    Task { @MainActor in
                        self?.streamingViewModel.removeStream(streamId: stream.streamId)
                    }
                }
            )

            // Wire presence callback from protocol client
            await client.setPresenceCallback { [weak self] params in
                Task { @MainActor in
                    self?.handlePresence(params)
                }
            }
            await client.setMessageDeliveredCallback { [weak self] identity in
                Task { @MainActor in
                    self?.activePresence.removeValue(forKey: identity)
                }
            }

            // Wire tool event callback from protocol client
            await client.setToolEventCallback { [weak self] threadId, event in
                Task { @MainActor in
                    self?.handleToolEvent(threadId: threadId, event: event)
                }
            }

            // Wire context frame callback for memory transparency
            await client.setContextFrameCallback { [weak self] frame in
                Task { @MainActor in
                    self?.contextFrames[frame.threadId] = frame
                }
            }
        }
    }

    /// Connect to the AINM server with the configured URL and display name.
    /// Transitions: disconnected -> connecting -> connected (or error).
    func connect() async {
        guard let client else { return }
        connectionState = .connecting
        errorMessage = nil
        do {
            let capabilities = try await client.connect(
                serverURL: serverURL,
                displayName: displayName
            )
            serverCapabilities = capabilities
            connectedIdentity = "\(displayName)@\(URL(string: serverURL)?.host ?? "localhost")"
            connectionState = .connected
        } catch {
            connectionState = .error
            errorMessage = mapErrorMessage(error)
        }
    }

    /// Disconnect from the AINM server.
    func disconnect() async {
        await client?.disconnect()
        connectionState = .disconnected
        serverCapabilities = nil
        connectedIdentity = nil
        activePresence.removeAll()
        presenceCleanupTask?.cancel()
    }

    /// Send a signed message envelope via the protocol client.
    func sendMessage(envelope: MessageEnvelope) async throws {
        guard let client else { return }
        try await client.sendMessage(envelope: envelope)
    }

    /// Create a new thread and send the first message via the protocol client.
    func createThread(title: String?, firstMessageText: String) async throws -> String {
        guard let client, let identity = connectedIdentity else {
            throw AINMClient.ClientError.notConnected
        }
        return try await client.createThread(title: title, firstMessageText: firstMessageText, identity: identity)
    }

    /// Create a sub-thread branching from a specific message.
    func createSubThread(parentThreadId: String, parentMessageId: String) async throws -> String {
        guard let client, let identity = connectedIdentity else {
            throw AINMClient.ClientError.notConnected
        }
        return try await client.createSubThread(
            parentThreadId: parentThreadId,
            parentMessageId: parentMessageId,
            identity: identity
        )
    }

    /// Cancel all active streams in a thread. Marks them as cancelling in the view model
    /// and sends cancel requests via the protocol client.
    func cancelStreams(inThread threadId: String) async {
        for stream in streamingViewModel.activeStreamsForThread(threadId) {
            streamingViewModel.markCancelling(streamId: stream.id)
        }
        try? await client?.cancelAllStreams(inThread: threadId)
    }

    // MARK: - ActionBlock Sending

    /// Construct and send an ActionBlock message in response to interactive block interactions.
    /// Wires from ContentBlockRenderer onAction -> AppState -> AINMClient.sendMessage.
    func sendActionBlock(
        refMessageId: String,
        refBlockIndex: Int,
        actionId: String,
        data: [String: Any],
        fallbackText: String,
        threadId: String
    ) async throws {
        guard let identity = connectedIdentity else { throw AINMClient.ClientError.notConnected }

        let actionContent = ContentBlock.ActionContent(
            version: "1.0",
            source: "user",
            refMessageId: refMessageId,
            refBlockIndex: refBlockIndex,
            actionId: actionId,
            data: AnyCodableValue.fromAny(data),
            fallback: fallbackText
        )

        let envelope = MessageEnvelope(
            id: UUID().uuidString,
            from: identity,
            threadId: threadId,
            timestamp: ISO8601DateFormatter.ainm.string(from: Date()),
            type: .action,
            content: [.action(actionContent)],
            signature: nil,
            parentId: nil
        )
        try await sendMessage(envelope: envelope)
    }

    // MARK: - Tool Call Handling

    /// Handle incoming tool call event and update observable state.
    func handleToolEvent(threadId: String, event: ToolCallEvent) {
        var states = toolCallStates[threadId] ?? []

        if let index = states.firstIndex(where: { $0.id == event.callId }) {
            // Update existing tool call state
            let state = states[index]
            if let progress = event.progress {
                state.progress = progress
            }
            if let status = event.status {
                state.status = status == "error" ? .error : .completed
                state.resultSummary = event.resultSummary
                state.durationMs = event.durationMs
                state.errorMessage = event.errorMessage
                // Cancel timeout task
                toolCallTimeoutTasks[event.callId]?.cancel()
                toolCallTimeoutTasks.removeValue(forKey: event.callId)
            }
        } else {
            // Create new tool call state (started event)
            let state = ToolCallState(
                id: event.callId,
                name: event.name ?? "Unknown tool",
                status: .running,
                input: event.input
            )
            states.append(state)
            toolCallStates[threadId] = states

            // Schedule 5-minute timeout
            let callId = event.callId
            toolCallTimeoutTasks[callId] = Task { @MainActor [weak self] in
                try? await Task.sleep(for: .seconds(300))
                guard !Task.isCancelled else { return }
                guard let self else { return }
                if let threadStates = self.toolCallStates[threadId],
                   let idx = threadStates.firstIndex(where: { $0.id == callId }),
                   threadStates[idx].status == .running {
                    threadStates[idx].status = .timedOut
                }
                self.toolCallTimeoutTasks.removeValue(forKey: callId)
            }
        }

        if toolCallStates[threadId] == nil {
            toolCallStates[threadId] = states
        }
    }

    /// Get tool calls for a thread, sorted by startedAt.
    func toolCalls(forThread threadId: String) -> [ToolCallState] {
        (toolCallStates[threadId] ?? []).sorted { $0.startedAt < $1.startedAt }
    }

    // MARK: - Memory Context

    /// Return the latest context frame for a given thread.
    func contextFrame(forThread threadId: String) -> ContextFrame? {
        contextFrames[threadId]
    }

    /// Query archival memory facts from the server.
    func queryMemory(query: String) async throws -> [ContextFrame.ArchivalFact] {
        guard let client else { throw AINMClient.ClientError.notConnected }
        let result = try await client.memoryQuery(query: query)
        return result.facts
    }

    /// Evict (delete) a specific archival memory fact from the server.
    func evictMemoryFact(factId: String) async throws {
        guard let client else { throw AINMClient.ClientError.notConnected }
        try await client.memoryEvict(factId: factId)
    }

    // MARK: - Presence Handling

    /// Handle incoming presence notification from the protocol client.
    func handlePresence(_ params: PresenceNotifyParams) {
        let name = params.identity.components(separatedBy: "@").first ?? params.identity
        if params.status == "processing" || params.status == "typing" {
            activePresence[params.identity] = PresenceEntry(
                id: params.identity,
                name: name,
                isAgent: params.status == "processing",
                threadId: params.threadId,
                lastUpdated: Date()
            )
            schedulePresenceCleanup()
        } else {
            activePresence.removeValue(forKey: params.identity)
        }
    }

    /// Schedule cleanup of stale presence entries (30-second guard per UI-SPEC).
    private func schedulePresenceCleanup() {
        presenceCleanupTask?.cancel()
        presenceCleanupTask = Task { @MainActor in
            try? await Task.sleep(for: .seconds(30))
            guard !Task.isCancelled else { return }
            let now = Date()
            activePresence = activePresence.filter { _, entry in
                now.timeIntervalSince(entry.lastUpdated) < 30
            }
        }
    }

    /// Get presence entries filtered by thread for display in MessageThreadView.
    func presenceEntries(forThread threadId: String) -> [PresenceEntry] {
        activePresence.values.filter { $0.threadId == threadId }.sorted { $0.name < $1.name }
    }

    /// Map errors to user-facing messages matching UI-SPEC copywriting contract.
    private func mapErrorMessage(_ error: Error) -> String {
        switch error {
        case WebSocketConnection.ConnectionError.invalidURL:
            return "Enter a valid WebSocket URL (wss:// or ws://)."
        case WebSocketConnection.ConnectionError.connectionFailed:
            return "Could not connect to server. Check the URL and try again."
        case AINMClient.ClientError.handshakeFailed(let msg):
            return "Server handshake failed. \(msg)"
        default:
            return "Could not connect to server. Check the URL and try again."
        }
    }
}
