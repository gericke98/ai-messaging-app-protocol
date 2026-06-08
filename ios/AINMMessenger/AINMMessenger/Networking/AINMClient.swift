import Foundation
import CryptoKit
import GRDB

/// AINM protocol client that orchestrates WebSocket, crypto, and persistence.
/// Implements the three-step handshake (ainm.initialize -> response -> ainm.initialized),
/// signed message exchange, and the write-through cache pattern where all incoming
/// data flows through GRDB before reaching the UI.
///
/// Actor-isolated for Swift 6 strict concurrency safety.
actor AINMClient {
    enum ClientState: Sendable {
        case disconnected
        case connecting
        case handshaking
        case connected(ServerCapabilities)
        case error(String)
    }

    enum ClientError: Error, Sendable {
        case notConnected
        case handshakeFailed(String)
        case encodingError
        case signingError(Error)
    }

    private let webSocket: WebSocketConnection
    private let reconnection: ReconnectionManager
    private let keyManager: KeyManager
    private let appDatabase: AppDatabase
    private let streamingManager = StreamingManager()
    private var nextRequestId: Int = 1
    private var pendingResponses: [Int: CheckedContinuation<Data, Error>] = [:]

    private(set) var state: ClientState = .disconnected
    private(set) var serverCapabilities: ServerCapabilities?
    private var receiveTask: Task<Void, Never>?

    /// Last-used connection parameters, captured on `connect(...)` so an unexpected
    /// socket drop can re-run the full handshake without re-prompting the user.
    private var lastServerURL: String?
    private var lastDisplayName: String?
    /// The identity string formed at handshake (`displayName@host`); used to mark self on delivery.
    private(set) var connectedIdentity: String?
    /// True while a user-initiated `disconnect()` is in effect; suppresses auto-reconnect
    /// on a drop (Sign out / explicit disconnect must not bounce back online).
    private var isUserDisconnect: Bool = false
    /// Drives the bounded-backoff reconnect loop; cancelled by `disconnect()`.
    private var reconnectTask: Task<Void, Never>?

    /// Coarse connection state for the UI layer (AppState maps this onto its display state).
    enum ConnectionState: Sendable { case reconnecting, connected, disconnected }
    /// Fired when the connection transitions due to a drop/reconnect cycle so AppState can
    /// reflect it without changing `sessionPhase`.
    var onConnectionStateChange: (@Sendable (ConnectionState) -> Void)?

    /// Register the connection-state callback. Called from AppState.setup().
    func setConnectionStateCallback(_ callback: @escaping @Sendable (ConnectionState) -> Void) {
        self.onConnectionStateChange = callback
    }

    /// Callbacks for streaming events. Set by the view layer to receive updates.
    var onStreamStart: ((StreamingManager.ActiveStream) -> Void)?
    var onStreamDelta: ((String, Int, String) -> Void)?  // streamId, blockIndex, fullContent
    var onStreamEnd: ((StreamingManager.ActiveStream, String) -> Void)?  // stream, status
    var onStreamError: ((StreamingManager.ActiveStream) -> Void)?
    var onPresenceNotify: ((PresenceNotifyParams) -> Void)?
    var onMessageDelivered: ((String) -> Void)?  // identity of sender
    var onToolEvent: ((String, ToolCallEvent) -> Void)?  // threadId, event
    var onContextFrame: ((ContextFrame) -> Void)?
    var onBriefing: ((AgentBriefing) -> Void)?
    var onCompilation: ((CompilationNotification) -> Void)?

    /// Set the presence notification callback. Called from AppState.setup().
    func setPresenceCallback(onPresence: @escaping @Sendable (PresenceNotifyParams) -> Void) {
        self.onPresenceNotify = onPresence
    }

    /// Set the message delivered callback to clear typing indicators.
    func setMessageDeliveredCallback(onDelivered: @escaping @Sendable (String) -> Void) {
        self.onMessageDelivered = onDelivered
    }

    /// Set the tool event callback for tool call visibility.
    func setToolEventCallback(onToolEvent: @escaping @Sendable (String, ToolCallEvent) -> Void) {
        self.onToolEvent = onToolEvent
    }

    /// Set the context frame callback for memory transparency.
    func setContextFrameCallback(onFrame: @escaping @Sendable (ContextFrame) -> Void) {
        self.onContextFrame = onFrame
    }

    /// Set the briefing callback for agent onboarding notifications.
    func setBriefingCallback(onBriefing: @escaping @Sendable (AgentBriefing) -> Void) {
        self.onBriefing = onBriefing
    }

    /// Set the compilation callback for memory graph updates.
    func setCompilationCallback(onCompilation: @escaping @Sendable (CompilationNotification) -> Void) {
        self.onCompilation = onCompilation
    }

    init(
        webSocket: WebSocketConnection = WebSocketConnection(),
        reconnection: ReconnectionManager = ReconnectionManager(),
        keyManager: KeyManager = KeyManager(),
        appDatabase: AppDatabase
    ) {
        self.webSocket = webSocket
        self.reconnection = reconnection
        self.keyManager = keyManager
        self.appDatabase = appDatabase
    }

    /// Set all streaming callbacks at once. Called by AppState to wire
    /// StreamingViewModel updates from the actor-isolated AINMClient.
    func setStreamCallbacks(
        onStart: @escaping @Sendable (StreamingManager.ActiveStream) -> Void,
        onDelta: @escaping @Sendable (String, Int, String) -> Void,
        onEnd: @escaping @Sendable (StreamingManager.ActiveStream, String) -> Void,
        onError: @escaping @Sendable (StreamingManager.ActiveStream) -> Void
    ) {
        self.onStreamStart = onStart
        self.onStreamDelta = onDelta
        self.onStreamEnd = onEnd
        self.onStreamError = onError
    }

    /// Full connection flow: connect WebSocket + handshake.
    /// Per D-07: always full re-handshake (stateless).
    func connect(serverURL: String, displayName: String) async throws -> ServerCapabilities {
        state = .connecting

        // Remember params + clear the user-disconnect flag so an unexpected drop after this
        // point re-handshakes against the same server/identity automatically.
        lastServerURL = serverURL
        lastDisplayName = displayName
        isUserDisconnect = false

        // 1. Get or create Ed25519 keypair (NET-04)
        let keypair = try await keyManager.getOrCreateKeypair()

        // 2. Connect WebSocket (NET-01)
        try await webSocket.connect(to: serverURL)
        state = .handshaking

        // 3. Build ainm.initialize request (NET-02)
        let identity = "\(displayName)@\(extractHost(from: serverURL))"
        self.connectedIdentity = identity
        let timestamp = ISO8601DateFormatter.ainm.string(from: Date())
        let initSignature = try MessageSigner.signInit(
            identity: identity,
            timestamp: timestamp,
            privateKey: keypair.privateKey
        )

        let initParams = InitializeParams(
            protocolVersion: "0.1.0",
            identity: identity,
            timestamp: timestamp,
            signature: initSignature,
            capabilities: ClientCapabilities(
                streaming: true,
                subThreads: false,
                contentTypes: ["text", "code", "image", "file", "button", "form", "table", "approval", "action"],
                parallelStreaming: true,
                toolVisibility: false
            )
        )

        let requestId = nextRequestId
        nextRequestId += 1

        let request = try JsonRpcRequest.initialize(
            id: .int(requestId),
            params: initParams
        )

        // 4. Send initialize request
        let requestData = try JSONEncoder().encode(request)
        try await webSocket.send(requestData)

        // 5. Wait for response
        let responseData = try await waitForResponse(id: requestId)
        let response = try JSONDecoder().decode(JsonRpcResponse.self, from: responseData)

        guard response.error == nil else {
            let errMsg = response.error?.message ?? "Unknown handshake error"
            state = .error(errMsg)
            throw ClientError.handshakeFailed(errMsg)
        }

        // 6. Decode server capabilities from result
        guard let resultValue = response.result else {
            state = .error("No result in server response")
            throw ClientError.handshakeFailed("No result in server response")
        }
        let resultData = try JSONEncoder().encode(resultValue)
        let initResult = try JSONDecoder().decode(InitializeResult.self, from: resultData)

        // 7. Send ainm.initialized notification
        let notification = JsonRpcNotification.initialized()
        let notifData = try JSONEncoder().encode(notification)
        try await webSocket.send(notifData)

        // 8. Store capabilities and start receive loop
        serverCapabilities = initResult.capabilities
        state = .connected(initResult.capabilities)
        startReceiveLoop()

        await reconnection.reset()
        return initResult.capabilities
    }

    /// Send a signed message envelope (NET-05).
    func sendMessage(envelope: MessageEnvelope) async throws {
        guard case .connected = state else { throw ClientError.notConnected }

        let keypair = try await keyManager.getOrCreateKeypair()

        // Convert to dictionary for signing
        let envelopeData = try JSONEncoder().encode(envelope)
        guard var dict = try JSONSerialization.jsonObject(with: envelopeData) as? [String: Any] else {
            throw ClientError.encodingError
        }

        // Sign the envelope (NET-05)
        let signature = try MessageSigner.sign(envelope: dict, privateKey: keypair.privateKey)
        dict["signature"] = signature

        // Wrap in JSON-RPC request
        let requestId = nextRequestId
        nextRequestId += 1
        let rpcDict: [String: Any] = [
            "jsonrpc": "2.0",
            "id": requestId,
            "method": "ainm.message.send",
            "params": dict
        ]
        let rpcData = try JSONSerialization.data(withJSONObject: rpcDict)
        try await webSocket.send(rpcData)

        // Persist sent message to GRDB (write-through)
        var signedEnvelope = envelope
        signedEnvelope.signature = signature
        let record = try MessageRecord(from: signedEnvelope)
        try await appDatabase.dbPool.write { db in
            try record.save(db)
        }
    }

    /// Create a new thread and send the first message.
    /// Per threading.md: sends thread.create envelope, then the first message envelope.
    /// Both are persisted to GRDB via write-through.
    func createThread(title: String?, firstMessageText: String, identity: String) async throws -> String {
        guard case .connected = state else { throw ClientError.notConnected }

        let threadId = UUID().uuidString
        let timestamp = ISO8601DateFormatter.ainm.string(from: Date())

        // 1. Persist thread locally first (write-through)
        let threadRecord = ThreadRecord(
            id: threadId,
            title: title,
            createdAt: timestamp,
            createdBy: identity,
            parentThreadId: nil,
            parentMessageId: nil
        )
        try await appDatabase.dbPool.write { db in
            try threadRecord.save(db)
        }

        // 2. Send thread.create via protocol
        let createEnvelope = MessageEnvelope(
            id: UUID().uuidString,
            from: identity,
            threadId: threadId,
            timestamp: timestamp,
            type: .threadCreate,
            content: [],
            signature: nil,
            parentId: nil
        )
        try await sendMessage(envelope: createEnvelope)

        // 3. Send first message
        let messageEnvelope = MessageEnvelope(
            id: UUID().uuidString,
            from: identity,
            threadId: threadId,
            timestamp: ISO8601DateFormatter.ainm.string(from: Date()),
            type: .message,
            content: [.text(ContentBlock.TextContent(
                version: "1.0",
                source: "user",
                text: firstMessageText,
                mentions: nil,
                fallback: firstMessageText
            ))],
            signature: nil,
            parentId: nil
        )
        try await sendMessage(envelope: messageEnvelope)

        return threadId
    }

    /// Create a sub-thread branching from a specific message in a parent thread.
    /// Per threading.md: sub-threads carry parentThreadId and parentMessageId.
    func createSubThread(
        parentThreadId: String,
        parentMessageId: String,
        identity: String
    ) async throws -> String {
        guard case .connected = state else { throw ClientError.notConnected }

        let threadId = UUID().uuidString
        let timestamp = ISO8601DateFormatter.ainm.string(from: Date())

        // Persist sub-thread locally (write-through)
        let threadRecord = ThreadRecord(
            id: threadId,
            title: nil,
            createdAt: timestamp,
            createdBy: identity,
            parentThreadId: parentThreadId,
            parentMessageId: parentMessageId
        )
        try await appDatabase.dbPool.write { db in
            try threadRecord.save(db)
        }

        // Send thread.create with parent fields via protocol
        let createEnvelope = MessageEnvelope(
            id: UUID().uuidString,
            from: identity,
            threadId: threadId,
            timestamp: timestamp,
            type: .threadCreate,
            content: [],
            signature: nil,
            parentId: parentMessageId
        )
        try await sendMessage(envelope: createEnvelope)
        return threadId
    }

    /// Cancel an active stream by sending `ainm.stream.cancel` to the server.
    func cancelStream(streamId: String) async throws {
        guard case .connected = state else { throw ClientError.notConnected }
        let exists = await streamingManager.cancelStream(streamId)
        guard exists else { return }
        let requestId = nextRequestId
        nextRequestId += 1
        let request: [String: Any] = [
            "jsonrpc": "2.0",
            "id": requestId,
            "method": "ainm.stream.cancel",
            "params": ["stream_id": streamId]
        ]
        let data = try JSONSerialization.data(withJSONObject: request)
        try await webSocket.send(data)
    }

    /// Cancel all active streams in a given thread.
    func cancelAllStreams(inThread threadId: String) async throws {
        let streamIds = await streamingManager.activeStreamIds(forThread: threadId)
        for streamId in streamIds {
            try await cancelStream(streamId: streamId)
        }
    }

    /// Query archival memory facts from the server.
    func memoryQuery(query: String, tier: String = "archival") async throws -> MemoryQueryResult {
        guard case .connected = state else { throw ClientError.notConnected }
        let requestId = nextRequestId
        nextRequestId += 1
        let request: [String: Any] = [
            "jsonrpc": "2.0",
            "id": requestId,
            "method": "ainm.memory.query",
            "params": ["query": query, "tier": tier]
        ]
        let data = try JSONSerialization.data(withJSONObject: request)
        try await webSocket.send(data)

        let responseData = try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Data, Error>) in
            pendingResponses[requestId] = continuation
        }
        let response = try JSONDecoder().decode(JsonRpcResponse.self, from: responseData)
        if let error = response.error {
            throw ClientError.handshakeFailed(error.message)
        }
        guard let resultValue = response.result else {
            throw ClientError.handshakeFailed("No result in memory query response")
        }
        let resultData = try JSONEncoder().encode(resultValue)
        return try JSONDecoder().decode(MemoryQueryResult.self, from: resultData)
    }

    /// Evict (delete) a specific archival memory fact from the server.
    func memoryEvict(factId: String) async throws {
        guard case .connected = state else { throw ClientError.notConnected }
        let requestId = nextRequestId
        nextRequestId += 1
        let request: [String: Any] = [
            "jsonrpc": "2.0",
            "id": requestId,
            "method": "ainm.memory.evict",
            "params": ["fact_id": factId]
        ]
        let data = try JSONSerialization.data(withJSONObject: request)
        try await webSocket.send(data)

        let responseData = try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Data, Error>) in
            pendingResponses[requestId] = continuation
        }
        let response = try JSONDecoder().decode(JsonRpcResponse.self, from: responseData)
        if let error = response.error {
            throw ClientError.handshakeFailed(error.message)
        }
    }

    // MARK: - Compound Memory Operations

    /// Query the user's memory graph with optional filters.
    func graphQuery(
        query: String? = nil,
        minConfidence: Float? = nil,
        temporalFilter: String? = nil,
        includeEdges: Bool = true,
        limit: Int? = nil
    ) async throws -> GraphQueryResult {
        guard case .connected = state else { throw ClientError.notConnected }
        var params: [String: Any] = ["include_edges": includeEdges]
        if let query { params["query"] = query }
        if let minConfidence { params["min_confidence"] = minConfidence }
        if let temporalFilter { params["temporal_filter"] = temporalFilter }
        if let limit { params["limit"] = limit }
        return try await sendRpcRequest(method: "ainm.memory.graph.query", params: params)
    }

    /// Traverse the memory graph from a starting node.
    func graphTraverse(
        startNode: String,
        maxDepth: Int = 2,
        direction: String? = nil
    ) async throws -> GraphTraverseResult {
        guard case .connected = state else { throw ClientError.notConnected }
        var params: [String: Any] = ["start_node": startNode, "max_depth": maxDepth]
        if let direction { params["direction"] = direction }
        return try await sendRpcRequest(method: "ainm.memory.graph.traverse", params: params)
    }

    /// Contribute a new node (and optional edges) to the memory graph.
    func graphContribute(
        threadId: String,
        node: [String: Any],
        edges: [[String: Any]]? = nil
    ) async throws -> GraphContributeResult {
        guard case .connected = state else { throw ClientError.notConnected }
        var params: [String: Any] = ["thread_id": threadId, "node": node]
        if let edges { params["edges"] = edges }
        return try await sendRpcRequest(method: "ainm.memory.graph.contribute", params: params)
    }

    /// Request an agent briefing for a thread.
    func requestBriefing(threadId: String, userIdentity: String) async throws -> AgentBriefing {
        guard case .connected = state else { throw ClientError.notConnected }
        let params: [String: Any] = ["thread_id": threadId, "user_identity": userIdentity]
        return try await sendRpcRequest(method: "ainm.memory.briefing", params: params)
    }

    /// Acknowledge receipt of a briefing.
    func acknowledgeBriefing(briefingId: String) async throws {
        guard case .connected = state else { throw ClientError.notConnected }
        let requestId = nextRequestId
        nextRequestId += 1
        let request: [String: Any] = [
            "jsonrpc": "2.0",
            "id": requestId,
            "method": "ainm.memory.briefing.ack",
            "params": ["briefing_id": briefingId]
        ]
        let data = try JSONSerialization.data(withJSONObject: request)
        try await webSocket.send(data)
    }

    /// Grant memory scopes to an agent.
    func grantMemoryScopes(
        agentIdentity: String,
        scopes: [String],
        consentLevel: String = "standard"
    ) async throws -> MemoryGrantResult {
        guard case .connected = state else { throw ClientError.notConnected }
        let params: [String: Any] = [
            "agent_identity": agentIdentity,
            "scopes": scopes,
            "consent_level": consentLevel
        ]
        return try await sendRpcRequest(method: "ainm.memory.grant", params: params)
    }

    /// Revoke memory scopes from an agent.
    func revokeMemoryScopes(
        agentIdentity: String,
        scopes: [String]
    ) async throws -> MemoryRevokeResult {
        guard case .connected = state else { throw ClientError.notConnected }
        let params: [String: Any] = [
            "agent_identity": agentIdentity,
            "scopes": scopes
        ]
        return try await sendRpcRequest(method: "ainm.memory.revoke", params: params)
    }

    /// Export the user's full memory graph in ainm-memory-v1 format.
    func exportMemory(includeSuperseded: Bool = false) async throws -> MemoryExport {
        guard case .connected = state else { throw ClientError.notConnected }
        let params: [String: Any] = ["include_superseded": includeSuperseded]
        return try await sendRpcRequest(method: "ainm.memory.export", params: params)
    }

    /// Run a lint pass on the memory graph.
    func lint() async throws -> LintResult {
        guard case .connected = state else { throw ClientError.notConnected }
        return try await sendRpcRequest(method: "ainm.memory.lint", params: [:])
    }

    // MARK: - RPC Helper

    /// Send a JSON-RPC request and decode the response into the given type.
    private func sendRpcRequest<T: Decodable>(method: String, params: [String: Any]) async throws -> T {
        let requestId = nextRequestId
        nextRequestId += 1
        let request: [String: Any] = [
            "jsonrpc": "2.0",
            "id": requestId,
            "method": method,
            "params": params
        ]
        let data = try JSONSerialization.data(withJSONObject: request)
        try await webSocket.send(data)

        let responseData = try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Data, Error>) in
            pendingResponses[requestId] = continuation
        }
        let response = try JSONDecoder().decode(JsonRpcResponse.self, from: responseData)
        if let error = response.error {
            throw ClientError.handshakeFailed(error.message)
        }
        guard let resultValue = response.result else {
            throw ClientError.handshakeFailed("No result in \(method) response")
        }
        let resultData = try JSONEncoder().encode(resultValue)
        return try JSONDecoder().decode(T.self, from: resultData)
    }

    /// Disconnect gracefully. User-initiated: suppresses auto-reconnect and cancels any
    /// in-flight reconnect loop, so Sign out / explicit disconnect stays disconnected.
    func disconnect() async {
        isUserDisconnect = true
        reconnectTask?.cancel()
        reconnectTask = nil
        await reconnection.reset()
        receiveTask?.cancel()
        receiveTask = nil
        await streamingManager.clearAllStreams()
        // Cancel any pending response continuations
        for (_, continuation) in pendingResponses {
            continuation.resume(throwing: ClientError.notConnected)
        }
        pendingResponses.removeAll()
        await webSocket.disconnect()
        state = .disconnected
        onConnectionStateChange?(.disconnected)
    }

    // MARK: - Private

    private func waitForResponse(id: Int) async throws -> Data {
        for await result in await webSocket.receiveMessages() {
            switch result {
            case .success(let data):
                // Check if this is our response
                if let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                   let responseId = json["id"] as? Int,
                   responseId == id {
                    return data
                }
                // Not our response -- skip (in production, queue for later)
            case .failure(let error):
                throw error
            }
        }
        throw ClientError.notConnected
    }

    private func startReceiveLoop() {
        receiveTask = Task { [weak self] in
            guard let self else { return }
            for await result in await self.webSocket.receiveMessages() {
                switch result {
                case .success(let data):
                    await self.handleIncomingMessage(data)
                case .failure:
                    // Disconnected -- trigger reconnection per D-06
                    await self.handleDisconnect()
                    break
                }
            }
        }
    }

    private func handleIncomingMessage(_ data: Data) async {
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }

        // Check if it's a notification (no "id" field = notification)
        if json["id"] == nil, let method = json["method"] as? String {
            switch method {
            case "ainm.message.deliver":
                if let params = json["params"] as? [String: Any] {
                    await handleMessageDelivery(params)
                }
            case "ainm.stream.start":
                if let paramsData = extractParams(json),
                   let params = try? JSONDecoder().decode(StreamStartParams.self, from: paramsData) {
                    let stream = await streamingManager.handleStreamStart(params)
                    onStreamStart?(stream)
                }
            case "ainm.stream.delta":
                if let paramsData = extractParams(json),
                   let params = try? JSONDecoder().decode(StreamDeltaParams.self, from: paramsData) {
                    if let result = await streamingManager.handleDelta(params) {
                        onStreamDelta?(result.streamId, result.blockIndex, result.fullContent)
                    }
                }
            case "ainm.stream.end":
                if let paramsData = extractParams(json),
                   let params = try? JSONDecoder().decode(StreamEndParams.self, from: paramsData) {
                    if let stream = await streamingManager.handleStreamEnd(params) {
                        onStreamEnd?(stream, params.status)
                    }
                }
            case "ainm.stream.error":
                if let paramsData = extractParams(json),
                   let params = try? JSONDecoder().decode(StreamErrorParams.self, from: paramsData) {
                    if let stream = await streamingManager.handleStreamError(params) {
                        onStreamError?(stream)
                    }
                }
            case "ainm.presence.notify":
                if let paramsData = extractParams(json),
                   let params = try? JSONDecoder().decode(PresenceNotifyParams.self, from: paramsData) {
                    onPresenceNotify?(params)
                }
            case "ainm.tool.call.started", "ainm.tool.call.progress", "ainm.tool.call.completed":
                if let paramsData = extractParams(json),
                   let event = try? JSONDecoder().decode(ToolCallEvent.self, from: paramsData) {
                    if let params = json["params"] as? [String: Any],
                       let threadId = params["thread_id"] as? String {
                        await handleToolEvent(threadId: threadId, event: event)
                        onToolEvent?(threadId, event)
                    }
                }
            case "ainm.context.frame":
                if let paramsData = extractParams(json),
                   let frame = try? JSONDecoder().decode(ContextFrame.self, from: paramsData) {
                    onContextFrame?(frame)
                }
            case "ainm.memory.briefing":
                if let paramsData = extractParams(json),
                   let briefing = try? JSONDecoder().decode(AgentBriefing.self, from: paramsData) {
                    onBriefing?(briefing)
                }
            case "ainm.memory.compile":
                if let paramsData = extractParams(json),
                   let notification = try? JSONDecoder().decode(CompilationNotification.self, from: paramsData) {
                    onCompilation?(notification)
                }
            default:
                break  // Unknown notification, skip
            }
        } else if let responseId = json["id"] as? Int {
            // It's a response to a pending request
            if let continuation = pendingResponses.removeValue(forKey: responseId) {
                continuation.resume(returning: data)
            }
        }
    }

    /// Record/refresh the participant row for a delivered (or seeded) sender. Best-effort:
    /// classification uses the only client-side signals available. Runs its own write so a
    /// failure here can never regress message persistence.
    static func recordParticipant(
        in appDatabase: AppDatabase,
        threadId: String,
        from address: String,
        timestamp: String,
        contentSource: String?,
        currentUserIdentity: String?,
        threadCreatedBy: String?,
        agentAddress: String?
    ) async throws {
        try await appDatabase.dbPool.write { db in
            let savedAgentAddresses = Set(
                try String.fetchAll(db, sql: "SELECT address FROM savedAgent"))
            let type = ParticipantClassifier.classify(
                address: address,
                isSelf: address == currentUserIdentity,
                agentAddress: agentAddress,
                savedAgentAddresses: savedAgentAddresses,
                contentSource: contentSource)
            let role = (address == threadCreatedBy) ? "creator" : "member"
            try ParticipantRepository.upsert(
                db, threadId: threadId, identity: address,
                type: type, role: role, joinedAt: timestamp)
        }
    }

    private func handleMessageDelivery(_ params: [String: Any]) async {
        // Write-through pattern: persist to GRDB first, then UI observes GRDB
        do {
            // The server wraps the envelope: params = { "message": {…}, "verified": bool }.
            // Decode the inner `message`; fall back to `params` for any path that delivers
            // the envelope directly.
            let envelopeObj = (params["message"] as? [String: Any]) ?? params
            let paramsData = try JSONSerialization.data(withJSONObject: envelopeObj)
            let envelope = try JSONDecoder().decode(MessageEnvelope.self, from: paramsData)
            // Clear typing indicator when participant sends a message
            onMessageDelivered?(envelope.from)
            let record = try MessageRecord(from: envelope)
            try await appDatabase.dbPool.write { db in
                // Ensure the thread exists locally (auto-create if missing)
                let threadExists = try ThreadRecord.fetchOne(db, key: envelope.threadId) != nil
                if !threadExists {
                    let thread = ThreadRecord(
                        id: envelope.threadId,
                        title: nil,
                        createdAt: envelope.timestamp,
                        createdBy: envelope.from,
                        parentThreadId: nil,
                        parentMessageId: nil,
                        hasUnread: true
                    )
                    try thread.save(db)
                }
                try record.save(db)
                // Mark thread as unread for incoming messages
                try db.execute(
                    sql: "UPDATE thread SET hasUnread = 1 WHERE id = ?",
                    arguments: [envelope.threadId]
                )
            }
            // Best-effort: record the sender in the thread roster. A failure here must never
            // drop the message, so it runs in its own write and swallows errors.
            let contentSource: String? = envelope.content.compactMap { block -> String? in
                if case .text(let c) = block { return c.source }
                return nil
            }.first
            let threadCreatedBy = (try? await appDatabase.dbPool.read { db in
                try ThreadRecord.fetchOne(db, key: envelope.threadId)?.createdBy
            }) ?? nil
            let agentAddress = (try? await appDatabase.dbPool.read { db in
                try ThreadRecord.fetchOne(db, key: envelope.threadId)?.agentAddress
            }) ?? nil
            try? await AINMClient.recordParticipant(
                in: appDatabase, threadId: envelope.threadId, from: envelope.from,
                timestamp: envelope.timestamp, contentSource: contentSource,
                currentUserIdentity: connectedIdentity,
                threadCreatedBy: threadCreatedBy, agentAddress: agentAddress)
        } catch {
            // Log error but don't crash -- malformed messages are skipped
        }
    }

    private func handleDisconnect() async {
        await streamingManager.clearAllStreams()

        // Only auto-reconnect when the drop was unexpected (not a user sign-out/disconnect)
        // and we have the params to re-handshake. Otherwise just settle into .disconnected.
        guard !isUserDisconnect,
              let serverURL = lastServerURL,
              let displayName = lastDisplayName else {
            state = .disconnected
            onConnectionStateChange?(.disconnected)
            return
        }

        // D-06: auto-reconnect with bounded exponential backoff via ReconnectionManager.
        // D-07: each attempt is a full re-handshake (connect(...) is stateless).
        startReconnectLoop(serverURL: serverURL, displayName: displayName)
    }

    /// Drive a bounded-backoff reconnect loop: sleep ReconnectionManager.nextDelay(), then
    /// re-run the full handshake; stop on success; stop at the backoff ceiling as
    /// `.disconnected`. A user `disconnect()` cancels this task.
    private func startReconnectLoop(serverURL: String, displayName: String) {
        guard reconnectTask == nil else { return }
        state = .connecting
        onConnectionStateChange?(.reconnecting)

        reconnectTask = Task { [weak self] in
            guard let self else { return }
            // The 30s cap is reached after 6 attempts (1,2,4,8,16,30); allow a bounded
            // number of capped retries before giving up so the loop can't spin forever.
            let maxAttempts = 10
            for _ in 0..<maxAttempts {
                let delay = await self.reconnection.nextDelay()
                try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
                if Task.isCancelled { return }
                if await self.isUserDisconnect { return }
                do {
                    _ = try await self.connect(serverURL: serverURL, displayName: displayName)
                    // connect() resets the backoff and starts a fresh receive loop on success.
                    await self.finishReconnect(success: true)
                    return
                } catch {
                    // Stay in the loop; next iteration waits the next backoff delay.
                    if Task.isCancelled { return }
                    if await self.isUserDisconnect { return }
                }
            }
            await self.finishReconnect(success: false)
        }
    }

    /// Clear the reconnect task and surface the terminal state of a reconnect cycle.
    private func finishReconnect(success: Bool) async {
        reconnectTask = nil
        if success {
            onConnectionStateChange?(.connected)
        } else {
            state = .disconnected
            await reconnection.reset()
            onConnectionStateChange?(.disconnected)
        }
    }

    /// Persist tool call event to GRDB. Upserts based on callId.
    private func handleToolEvent(threadId: String, event: ToolCallEvent) async {
        let now = ISO8601DateFormatter.ainm.string(from: Date())
        do {
            try await appDatabase.dbPool.write { db in
                if var existing = try ToolCallRecord.fetchOne(db, key: event.callId) {
                    // Update existing record
                    if let status = event.status {
                        existing.status = status == "error" ? "error" : "completed"
                    }
                    if let resultSummary = event.resultSummary {
                        existing.resultSummary = resultSummary
                    }
                    if let durationMs = event.durationMs {
                        existing.durationMs = durationMs
                    }
                    if let errorMessage = event.errorMessage {
                        existing.errorMessage = errorMessage
                    }
                    if event.status != nil {
                        existing.completedAt = now
                    }
                    try existing.update(db)
                } else {
                    // Insert new record (started event)
                    var inputJson: String?
                    if let input = event.input {
                        let data = try JSONEncoder().encode(input)
                        inputJson = String(data: data, encoding: .utf8)
                    }
                    let record = ToolCallRecord(
                        callId: event.callId,
                        threadId: threadId,
                        name: event.name ?? "Unknown tool",
                        status: "running",
                        input: inputJson,
                        resultSummary: nil,
                        durationMs: nil,
                        errorMessage: nil,
                        startedAt: now,
                        completedAt: nil
                    )
                    try record.insert(db)
                }
            }
        } catch {
            // Log but don't crash -- tool events are non-critical
        }
    }

    /// Extract the "params" dictionary from a JSON-RPC message and serialize to Data.
    private func extractParams(_ json: [String: Any]) -> Data? {
        guard let params = json["params"] as? [String: Any] else { return nil }
        return try? JSONSerialization.data(withJSONObject: params)
    }

    private func extractHost(from urlString: String) -> String {
        URL(string: urlString)?.host ?? "localhost"
    }
}

// MARK: - ISO8601 Formatter

extension ISO8601DateFormatter {
    /// AINM protocol timestamp formatter: RFC 3339 with fractional seconds.
    nonisolated(unsafe) static let ainm: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()
}
