import Foundation

/// WebSocket connection wrapper using URLSessionWebSocketTask.
/// Actor-isolated for Swift 6 strict concurrency safety.
///
/// Connects to the AINM demo server at the /ws path. Provides
/// async send/receive methods with an AsyncStream-based pull model
/// for incoming messages.
actor WebSocketConnection {
    enum ConnectionState: Sendable {
        case disconnected
        case connecting
        case connected
        case disconnecting
    }

    enum ConnectionError: Error, Sendable {
        case notConnected
        case invalidURL
        case connectionFailed(Error)
    }

    private var task: URLSessionWebSocketTask?
    private let session: URLSession
    private(set) var state: ConnectionState = .disconnected

    init(session: URLSession = .shared) {
        self.session = session
    }

    /// Connect to WebSocket URL.
    /// The demo server uses /ws path (not the spec's /ainm/v1).
    func connect(to urlString: String) throws {
        guard let url = URL(string: urlString) else {
            throw ConnectionError.invalidURL
        }
        // Append /ws path for demo server if not already present
        let wsURL: URL
        if url.path.isEmpty || url.path == "/" {
            wsURL = url.appendingPathComponent("ws")
        } else {
            wsURL = url
        }
        state = .connecting
        task = session.webSocketTask(with: wsURL)
        task?.resume()
        state = .connected
    }

    /// Send a JSON-RPC message as text.
    func send(_ data: Data) async throws {
        guard let task, state == .connected else {
            throw ConnectionError.notConnected
        }
        guard let string = String(data: data, encoding: .utf8) else {
            throw ConnectionError.notConnected
        }
        try await task.send(.string(string))
    }

    /// Receive messages as an AsyncStream.
    /// URLSessionWebSocketTask uses a pull model -- must call receive() in a loop.
    func receiveMessages() -> AsyncStream<Result<Data, Error>> {
        let currentTask = self.task
        let getState: @Sendable () async -> ConnectionState = { [weak self] in
            guard let self else { return .disconnected }
            return await self.state
        }

        return AsyncStream { continuation in
            let receiveWork = Task {
                while !Task.isCancelled {
                    let currentState = await getState()
                    guard currentState == .connected else { break }
                    guard let wsTask = currentTask else { break }
                    do {
                        let message = try await wsTask.receive()
                        switch message {
                        case .string(let text):
                            if let data = text.data(using: .utf8) {
                                continuation.yield(.success(data))
                            }
                        case .data(let data):
                            continuation.yield(.success(data))
                        @unknown default:
                            break
                        }
                    } catch {
                        continuation.yield(.failure(error))
                        break
                    }
                }
                continuation.finish()
            }
            continuation.onTermination = { _ in
                receiveWork.cancel()
            }
        }
    }

    /// Graceful disconnect.
    func disconnect() {
        state = .disconnecting
        task?.cancel(with: .normalClosure, reason: nil)
        task = nil
        state = .disconnected
    }
}
