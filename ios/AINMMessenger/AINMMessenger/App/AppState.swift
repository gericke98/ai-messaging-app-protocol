import Foundation
import Observation
#if canImport(UIKit)
import UIKit
#endif

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
    /// Error surfaced by the account onboarding / recovery flows (separate from the
    /// connection `errorMessage` so the onboarding UI can present it independently).
    var onboardingError: String?
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

    // MARK: - Compound Memory

    /// Latest compound memory graph fetched from server, cached locally.
    var memoryGraph: GraphSnapshot?
    /// Latest briefing received (server push or request).
    var latestBriefing: AgentBriefing?
    /// Latest lint result from graph health check.
    var lintResult: LintResult?
    /// Cached graph stats (node count, edge count) for quick display.
    var graphStats: (nodes: Int, edges: Int)?

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

    /// Launch route decided by `bootstrap()`. Drives the root view in `AINMMessengerApp`:
    /// `.onboarding` → OnboardingFlowView, `.locked` → SessionGateView (Face ID unlock),
    /// `.active` → ConversationListView.
    enum SessionPhase: Sendable { case onboarding, locked, active }
    var sessionPhase: SessionPhase = .onboarding

    private var client: AINMClient?
    var appDatabase: AppDatabase?

    /// Source of truth for which accounts exist and which is active.
    let registry = AccountRegistry()

    /// In-memory mirror of the registry for SwiftUI (refreshed via `refreshAccounts()`).
    var accounts: [AccountSummary] = []
    var activeAccount: AccountSummary?

    /// Re-read the registry into the observable mirrors.
    func refreshAccounts() {
        accounts = registry.all()
        activeAccount = registry.active()
    }

    /// The active account's key manager. Defaults to the legacy slot (`address: nil`) so the
    /// UITest seed path and pre-migration reads work; `activate(address:)` reassigns it to the
    /// active account's address-keyed slot. Reassigned only from the @MainActor, so mutation is safe.
    private(set) var keyManager = KeyManager()

    // MARK: - Persisted Session Identity

    /// UserDefaults keys for the non-secret persisted session. The secret (the Ed25519
    /// private key) lives in the Keychain via `keyManager`; only the routing identity
    /// (server URL, username) and the Face ID preference are stored here.
    private enum SessionKey {
        static let serverURL = "ainm.session.serverURL"
        static let username  = "ainm.session.username"
        static let faceID    = "ainm.session.faceIDEnabled"
    }

    private func persistSession(serverURL: String, username: String, faceIDEnabled: Bool) {
        let d = UserDefaults.standard
        d.set(serverURL, forKey: SessionKey.serverURL)
        d.set(username, forKey: SessionKey.username)
        d.set(faceIDEnabled, forKey: SessionKey.faceID)
    }

    var persistedServerURL: String? { UserDefaults.standard.string(forKey: SessionKey.serverURL) }
    var persistedUsername: String? { UserDefaults.standard.string(forKey: SessionKey.username) }
    var faceIDEnabled: Bool { UserDefaults.standard.bool(forKey: SessionKey.faceID) }

    func clearPersistedSession() {
        let d = UserDefaults.standard
        [SessionKey.serverURL, SessionKey.username, SessionKey.faceID].forEach { d.removeObject(forKey: $0) }
    }

    /// Sign out of the current session: clear the persisted routing identity, optionally
    /// forget the Keychain key, drop the connection, and clear the in-memory identity.
    /// `sessionPhase` reset is wired in a later task (Task 1.3).
    func signOut(forgetKey: Bool = false) {
        clearPersistedSession()
        if forgetKey { Task { try? await keyManager.deleteKeypair() } }
        Task { await disconnect() }
        connectedIdentity = nil
        sessionPhase = .onboarding
    }

    /// Decide the launch route from the registry. Resume requires an active account whose
    /// key is present in the Keychain AND whose per-account database opened; otherwise fall
    /// back to onboarding. The Face-ID gate (SessionGateView) applies only when the account
    /// opted into `faceIDEnabled`.
    func bootstrap() async {
        refreshAccounts()
        guard let account = registry.active() else {
            sessionPhase = .onboarding
            return
        }
        await activate(address: account.address)
        // activate() leaves appDatabase nil if the per-account DB failed to open, and the
        // handshake needs the stored key — either failure means we cannot resume cleanly.
        guard appDatabase != nil, await keyManager.hasStoredKeypair() else {
            sessionPhase = .onboarding
            return
        }
        serverURL = account.serverURL
        displayName = account.username
        if account.faceIDEnabled {
            sessionPhase = .locked            // SessionGateView runs Face ID then connects
        } else {
            sessionPhase = .active
            await connect()
        }
    }

    /// Initialize the database and protocol client for the UITest seed path and any caller
    /// that wants the legacy single-account layout. Normal launches go through `bootstrap()`
    /// → `activate(address:)` instead.
    func setup() throws {
        let db = try AppDatabase.makeDefault()
        self.appDatabase = db
        self.client = makeClient(keyManager: keyManager, appDatabase: db)
    }

    /// Build an AINMClient for the given key manager + database and wire all of its callbacks
    /// into this AppState. Used by `setup()` (legacy/UITest) and `activate(address:)` (multi-account).
    /// Wiring runs in a detached Task because the client's setters are `async`.
    private func makeClient(keyManager: KeyManager, appDatabase db: AppDatabase) -> AINMClient {
        let client = AINMClient(keyManager: keyManager, appDatabase: db)
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
            await client.setToolEventCallback { [weak self] threadId, event in
                Task { @MainActor in
                    self?.handleToolEvent(threadId: threadId, event: event)
                }
            }
            await client.setContextFrameCallback { [weak self] frame in
                Task { @MainActor in
                    self?.contextFrames[frame.threadId] = frame
                }
            }
            await client.setBriefingCallback { [weak self] briefing in
                Task { @MainActor in
                    self?.latestBriefing = briefing
                }
            }
            await client.setCompilationCallback { [weak self] _ in
                Task { @MainActor in
                    await self?.fetchMemoryGraph()
                }
            }
            // CRITICAL: this only touches `connectionState` — `sessionPhase` stays `.active`
            // throughout a drop/reconnect so the user is never bounced to onboarding/lock.
            await client.setConnectionStateCallback { [weak self] connState in
                Task { @MainActor in
                    guard let self else { return }
                    switch connState {
                    case .reconnecting:
                        self.connectionState = .connecting
                    case .connected:
                        self.connectionState = .connected
                    case .disconnected:
                        self.connectionState = .disconnected
                    }
                }
            }
        }
        return client
    }

    /// Point AppState at `address`'s key slot + database, rebuilding the protocol client.
    /// Does NOT connect — callers (`bootstrap`, `switchAccount`, onboarding) connect after.
    /// Resets per-account in-memory caches so the previous account's data never bleeds through.
    func activate(address: String) async {
        let km = KeyManager(address: address)
        self.keyManager = km
        let db = try? AppDatabase.make(forAccount: address)
        self.appDatabase = db
        if let db {
            self.client = makeClient(keyManager: km, appDatabase: db)
        } else {
            self.client = nil
        }
        // Clear cross-account in-memory state.
        contextFrames.removeAll()
        toolCallStates.removeAll()
        memoryGraph = nil
        latestBriefing = nil
        graphStats = nil
        lintResult = nil
        activePresence.removeAll()
        presenceCleanupTask?.cancel()
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

    // MARK: - Account Onboarding & Recovery

    /// Outcome of a recovery attempt, surfaced to the UI for tailored messaging.
    enum RecoveryOutcome: Sendable {
        case connected
        case wrongPin
        case destroyed
        case failed
    }

    /// Derive the REST base URL from the stored WebSocket `serverURL`.
    ///
    /// `AccountService` needs an HTTP origin (e.g. `http://localhost:8080`) and only
    /// appends `accounts/...` paths. We map ws→http / wss→https and strip a trailing
    /// `/ws` path so the account endpoints resolve at the server root.
    private func restBaseURL(from serverURL: String) -> URL? {
        guard var comps = URLComponents(string: serverURL) else { return nil }
        switch comps.scheme {
        case "ws": comps.scheme = "http"
        case "wss": comps.scheme = "https"
        default: break
        }
        if comps.path == "/ws" || comps.path == "/ws/" { comps.path = "" }
        return comps.url
    }

    /// A human-readable label for the bound device key. Optional metadata for the server.
    private func deviceLabelString() -> String? {
        #if canImport(UIKit)
        return UIDevice.current.name
        #else
        return "AINM Messenger"
        #endif
    }

    /// Normalize any thrown error into a user-facing string for `onboardingError`.
    private func onboardingMessage(for error: Error) -> String {
        (error as? LocalizedError)?.errorDescription ?? "\(error)"
    }

    /// Trigger the signup email (sends the verification code). Separate from
    /// ``verifyCode(serverURL:email:code:)`` / ``finishSignup(serverURL:username:enrollmentToken:pin:faceIDEnabled:)``
    /// so the UI can advance to the code-entry step before the user has a code.
    func startSignup(serverURL: String, email: String, username: String) async -> Bool {
        guard let base = restBaseURL(from: serverURL) else {
            onboardingError = "Enter a valid server URL."
            return false
        }
        onboardingError = nil
        do {
            _ = try await AccountService(baseURL: base).signup(email: email, username: username)
            return true
        } catch {
            onboardingError = onboardingMessage(for: error)
            return false
        }
    }

    /// Step 1 of signup: verify the email OTP (consumes it once) and return the enrollment token.
    /// Returns `nil` and sets `onboardingError` on failure. This is the ONLY step that consumes
    /// the code, so it must run exactly once — `finishSignup` reuses the returned token.
    func verifyCode(serverURL: String, email: String, code: String) async -> String? {
        guard let base = restBaseURL(from: serverURL) else {
            onboardingError = "Enter a valid server URL."
            return nil
        }
        onboardingError = nil
        do {
            return try await AccountService(baseURL: base).verify(email: email, code: code)
        } catch {
            onboardingError = onboardingMessage(for: error)
            return nil
        }
    }

    /// Step 2 of signup: bind the device key → enroll the PIN escrow → connect, using the
    /// already-obtained enrollment token (does NOT re-verify), so it is safe to retry after a
    /// transient bind/escrow/connect failure without stranding on a consumed OTP.
    ///
    /// The bound key and the handshake key are the same (shared `keyManager`), and the identity
    /// becomes `username@host` because `connect()` reads `displayName`/`serverURL`, which we set
    /// here. Returns `true` once the enforced handshake succeeds.
    func finishSignup(serverURL: String, username: String, enrollmentToken: String, pin: String, faceIDEnabled: Bool) async -> Bool {
        guard let base = restBaseURL(from: serverURL) else {
            onboardingError = "Enter a valid server URL."
            return false
        }
        onboardingError = nil
        let account = AccountService(baseURL: base)
        do {
            let kp = try await keyManager.getOrCreateKeypair()
            try await account.bindKey(
                enrollmentToken: enrollmentToken,
                publicKeyB64: kp.publicKeyBase64url,
                deviceLabel: deviceLabelString()
            )
            try await EscrowClient(account: account, keyManager: keyManager)
                .enrollAndUpload(enrollmentToken: enrollmentToken, pin: pin)
            // Identity must be username@host to match the server-bound key; set both fields
            // so connect() builds it correctly, then handshake against the enforced server.
            self.serverURL = serverURL
            self.displayName = username
            await connect()
            if connectionState == .connected {
                persistSession(serverURL: serverURL, username: username, faceIDEnabled: faceIDEnabled)
                sessionPhase = .active
            }
            return connectionState == .connected
        } catch {
            onboardingError = onboardingMessage(for: error)
            return false
        }
    }

    /// Trigger the recovery email (sends the verification code).
    func startRecovery(serverURL: String, email: String) async -> Bool {
        guard let base = restBaseURL(from: serverURL) else {
            onboardingError = "Enter a valid server URL."
            return false
        }
        onboardingError = nil
        do {
            try await AccountService(baseURL: base).recoverStart(email: email)
            return true
        } catch {
            onboardingError = onboardingMessage(for: error)
            return false
        }
    }

    /// Complete recovery: verify the email code → recover the device key with the PIN (one
    /// OPRF-guarded attempt) → connect. Distinguishes wrong-PIN and escrow-destroyed for UX.
    ///
    /// On success the recovered key is restored into the same `keyManager` the handshake uses,
    /// so the identity (`username@host`, derived by `connect()` from `displayName`/`serverURL`)
    /// matches the account's server-bound key.
    func completeRecovery(serverURL: String, email: String, username: String, code: String, pin: String, faceIDEnabled: Bool = false) async -> RecoveryOutcome {
        guard let base = restBaseURL(from: serverURL) else {
            onboardingError = "Enter a valid server URL."
            return .failed
        }
        onboardingError = nil
        let account = AccountService(baseURL: base)
        do {
            let token = try await account.recoverVerify(email: email, code: code)
            try await EscrowClient(account: account, keyManager: keyManager)
                .recover(recoveryToken: token, pin: pin)
            self.serverURL = serverURL
            self.displayName = username
            await connect()
            if connectionState == .connected {
                persistSession(serverURL: serverURL, username: username, faceIDEnabled: faceIDEnabled)
                sessionPhase = .active
                return .connected
            }
            return .failed
        } catch EscrowClient.RecoverError.wrongPin {
            return .wrongPin
        } catch EscrowClient.RecoverError.destroyed {
            onboardingError = "Recovery is no longer possible (too many attempts)."
            return .destroyed
        } catch {
            onboardingError = onboardingMessage(for: error)
            return .failed
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

    /// Derive the built-in agent address `assistant@<host>` from a WS server URL.
    /// nil if the URL has no host. The host must equal the server's `server_domain`
    /// (account binding is `username@<host>`).
    nonisolated static func assistantAddress(forServerURL serverURL: String) -> String? {
        guard let host = URL(string: serverURL)?.host else { return nil }
        return "assistant@\(host)"
    }

    /// Look up the target agent address for a thread, if any. Reads the persisted
    /// `ThreadRecord.agentAddress` directly from GRDB (AppState keeps no in-memory
    /// thread list — the thread list UI observes the database). Returns nil for
    /// human↔human threads (and when there is no database / no such thread).
    private func agentAddress(forThread threadId: String) async -> String? {
        guard let db = appDatabase else { return nil }
        return try? await db.dbPool.read { db in
            try ThreadRecord.fetchOne(db, key: threadId)?.agentAddress
        }
    }

    /// Send a signed message envelope via the protocol client.
    ///
    /// If the message targets a thread bound to an agent (the persisted
    /// `ThreadRecord.agentAddress` is non-nil) and the envelope does not already
    /// carry an explicit recipient, the agent address is stamped as `to` so the
    /// server routes the message to that agent. Human↔human threads leave `to` nil,
    /// so the envelope encodes byte-identically to before (the encoder omits nil `to`).
    func sendMessage(envelope: MessageEnvelope) async throws {
        guard let client else { return }
        var envelope = envelope
        if envelope.to == nil, let agent = await agentAddress(forThread: envelope.threadId) {
            envelope.to = agent
        }
        try await client.sendMessage(envelope: envelope)
    }

    /// Create a new thread and send the first message via the protocol client.
    /// In UAT mode (DEBUG + AINMClient not connected) falls back to a local-only write so
    /// the "Start Conversation" flow still lands the user in the new thread.
    func createThread(title: String?, firstMessageText: String) async throws -> String {
        guard let identity = connectedIdentity else {
            throw AINMClient.ClientError.notConnected
        }
        if let client {
            do {
                return try await client.createThread(title: title, firstMessageText: firstMessageText, identity: identity)
            } catch AINMClient.ClientError.notConnected {
                #if DEBUG
                // Fall through to local-only creation.
                #else
                throw AINMClient.ClientError.notConnected
                #endif
            }
        }
        #if DEBUG
        guard let db = appDatabase else { throw AINMClient.ClientError.notConnected }
        let threadId = UUID().uuidString
        let timestamp = ISO8601DateFormatter.ainm.string(from: Date())
        let record = ThreadRecord(
            id: threadId,
            title: title,
            createdAt: timestamp,
            createdBy: identity,
            parentThreadId: nil,
            parentMessageId: nil
        )
        try await db.dbPool.write { db in
            try record.save(db)
        }
        return threadId
        #else
        throw AINMClient.ClientError.notConnected
        #endif
    }

    /// Create + persist a thread addressed to the built-in assistant agent.
    /// Returns the new threadId, or nil if the agent address can't be derived
    /// (no host in `serverURL`), if there is no identity, or if there is no database.
    ///
    /// Reuses the exact persistence path of ``createThread(title:firstMessageText:)``'s
    /// local write: a fresh UUID id, the `ISO8601DateFormatter.ainm` timestamp, and a
    /// direct `ThreadRecord.save` on `appDatabase.dbPool`. The only difference is that
    /// `agentAddress` is set, which makes ``sendMessage(envelope:)`` stamp `to` for
    /// every message in this thread. AppState keeps no in-memory thread list, so there
    /// is no separate published state to update — the thread list UI observes GRDB.
    @discardableResult
    func startAssistantThread() async -> String? {
        guard let agent = AppState.assistantAddress(forServerURL: serverURL) else { return nil }
        guard let identity = connectedIdentity else { return nil }
        guard let db = appDatabase else { return nil }
        let threadId = UUID().uuidString
        let timestamp = ISO8601DateFormatter.ainm.string(from: Date())
        let record = ThreadRecord(
            id: threadId,
            title: "Assistant",
            createdAt: timestamp,
            createdBy: identity,
            parentThreadId: nil,
            parentMessageId: nil,
            agentAddress: agent
        )
        do {
            try await db.dbPool.write { db in
                try record.save(db)
            }
        } catch {
            return nil
        }
        return threadId
    }

    /// Create a sub-thread branching from a specific message.
    /// In UAT mode (DEBUG + no live client handshake) falls back to a local-only write
    /// so sub-thread creation flows can be exercised without a connected server.
    func createSubThread(parentThreadId: String, parentMessageId: String) async throws -> String {
        guard let identity = connectedIdentity else {
            throw AINMClient.ClientError.notConnected
        }
        if let client {
            do {
                return try await client.createSubThread(
                    parentThreadId: parentThreadId,
                    parentMessageId: parentMessageId,
                    identity: identity
                )
            } catch AINMClient.ClientError.notConnected {
                #if DEBUG
                // Fall through to local-only creation below.
                #else
                throw AINMClient.ClientError.notConnected
                #endif
            }
        }
        #if DEBUG
        // Local-only sub-thread creation for UAT runs where the AINMClient isn't
        // connected. Writes a ThreadRecord directly so navigation + banner assertions
        // work against real DB state.
        guard let db = appDatabase else { throw AINMClient.ClientError.notConnected }
        let subThreadId = UUID().uuidString
        let timestamp = ISO8601DateFormatter.ainm.string(from: Date())
        let record = ThreadRecord(
            id: subThreadId,
            title: nil,
            createdAt: timestamp,
            createdBy: identity,
            parentThreadId: parentThreadId,
            parentMessageId: parentMessageId
        )
        try await db.dbPool.write { db in
            try record.save(db)
        }
        return subThreadId
        #else
        throw AINMClient.ClientError.notConnected
        #endif
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

    // MARK: - Compound Memory Operations

    /// Fetch the full memory graph from the server and cache locally.
    func fetchMemoryGraph() async {
        guard let client else { return }
        do {
            let result = try await client.graphQuery(includeEdges: true, limit: 500)
            let snapshot = GraphSnapshot(nodes: result.nodes, edges: result.edges)
            memoryGraph = snapshot
            graphStats = (result.totalCount, result.edges.count)

            // Sync to local GRDB cache
            if let db = appDatabase {
                let repo = MemoryRepository(dbPool: db.dbPool)
                try await repo.replaceAll(nodes: result.nodes, edges: result.edges)
            }
        } catch {
            // Non-fatal — compound memory is optional
        }
    }

    /// Run a lint pass on the memory graph.
    func runLint() async {
        guard let client else { return }
        do {
            lintResult = try await client.lint()
        } catch {
            // Non-fatal
        }
    }

    /// Export the user's memory graph in ainm-memory-v1 format.
    func exportMemory() async -> MemoryExport? {
        guard let client else { return nil }
        return try? await client.exportMemory()
    }

    /// Grant memory scopes to an agent.
    func grantScopes(agent: String, scopes: [String], consentLevel: String = "standard") async {
        guard let client else { return }
        _ = try? await client.grantMemoryScopes(
            agentIdentity: agent,
            scopes: scopes,
            consentLevel: consentLevel
        )
    }

    /// Revoke memory scopes from an agent.
    func revokeScopes(agent: String, scopes: [String]) async {
        guard let client else { return }
        _ = try? await client.revokeMemoryScopes(
            agentIdentity: agent,
            scopes: scopes
        )
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
