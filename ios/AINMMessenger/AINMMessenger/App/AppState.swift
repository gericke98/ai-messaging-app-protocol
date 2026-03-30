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

    enum ConnectionDisplayState: Sendable {
        case disconnected
        case connecting
        case connected
        case error
    }

    private var client: AINMClient?
    private var appDatabase: AppDatabase?

    /// Initialize the database and protocol client.
    /// Call once at app launch before any network operations.
    func setup() throws {
        let db = try AppDatabase.makeDefault()
        self.appDatabase = db
        self.client = AINMClient(appDatabase: db)
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
    }

    /// Send a test message (Phase 10 development aid).
    func sendTestMessage() async throws {
        guard let client else { return }
        let envelope = MessageEnvelope(
            id: UUID().uuidString,  // Simplified for Phase 10; UUIDv7 in Phase 11
            from: connectedIdentity ?? "",
            threadId: "test-thread",
            timestamp: ISO8601DateFormatter.ainm.string(from: Date()),
            type: .message,
            content: [.text(ContentBlock.TextContent(
                version: "1.0",
                source: "user",
                text: "Test message from iOS",
                mentions: nil,
                fallback: "Test message from iOS"
            ))],
            signature: nil,
            parentId: nil
        )
        try await client.sendMessage(envelope: envelope)
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
