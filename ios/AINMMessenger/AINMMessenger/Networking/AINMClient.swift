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
    private var nextRequestId: Int = 1

    private(set) var state: ClientState = .disconnected
    private(set) var serverCapabilities: ServerCapabilities?
    private var receiveTask: Task<Void, Never>?

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

    /// Full connection flow: connect WebSocket + handshake.
    /// Per D-07: always full re-handshake (stateless).
    func connect(serverURL: String, displayName: String) async throws -> ServerCapabilities {
        state = .connecting

        // 1. Get or create Ed25519 keypair (NET-04)
        let keypair = try await keyManager.getOrCreateKeypair()

        // 2. Connect WebSocket (NET-01)
        try await webSocket.connect(to: serverURL)
        state = .handshaking

        // 3. Build ainm.initialize request (NET-02)
        let identity = "\(displayName)@\(extractHost(from: serverURL))"
        let timestamp = ISO8601DateFormatter.ainm.string(from: Date())
        let initSignature = try MessageSigner.signInit(
            identity: identity,
            timestamp: timestamp,
            privateKey: keypair.privateKey
        )

        let initParams = InitializeParams(
            protocolVersion: "0.1.0",
            identity: identity,
            signature: initSignature,
            capabilities: ClientCapabilities(
                streaming: true,
                subThreads: false,
                contentTypes: ["text"],  // Phase 10: text only
                parallelStreaming: false,
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

    /// Disconnect gracefully.
    func disconnect() async {
        receiveTask?.cancel()
        receiveTask = nil
        await webSocket.disconnect()
        state = .disconnected
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
            default:
                break  // Unknown notification, skip
            }
        }
    }

    private func handleMessageDelivery(_ params: [String: Any]) async {
        // Write-through pattern: persist to GRDB first, then UI observes GRDB
        do {
            let paramsData = try JSONSerialization.data(withJSONObject: params)
            let envelope = try JSONDecoder().decode(MessageEnvelope.self, from: paramsData)
            let record = try MessageRecord(from: envelope)
            try await appDatabase.dbPool.write { db in
                try record.save(db)
            }
        } catch {
            // Log error but don't crash -- malformed messages are skipped
        }
    }

    private func handleDisconnect() async {
        state = .disconnected
        // D-06: auto-reconnect with exponential backoff
        // D-07: full re-handshake on reconnect
        // Reconnection needs the original URL and display name -- stored on first connect
        // For Phase 10, reconnection is best-effort. Full implementation in Phase 11.
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
