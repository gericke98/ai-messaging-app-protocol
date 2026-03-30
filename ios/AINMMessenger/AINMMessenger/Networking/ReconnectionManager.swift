import Foundation

/// Exponential backoff reconnection manager per research decisions D-06 and D-07.
/// Actor-isolated for Swift 6 strict concurrency safety.
///
/// Backoff sequence: 1s, 2s, 4s, 8s, 16s, 30s, 30s... (capped at 30 seconds).
/// Per D-07: reconnection always triggers a full re-handshake (stateless).
actor ReconnectionManager {
    private let baseDelay: TimeInterval = 1.0    // 1 second
    private let maxDelay: TimeInterval = 30.0    // 30 second cap per D-06
    private let multiplier: Double = 2.0
    private var currentAttempt: Int = 0
    private var isReconnecting: Bool = false

    /// Calculate next delay using exponential backoff: 1s, 2s, 4s, 8s, 16s, 30s, 30s...
    func nextDelay() -> TimeInterval {
        let delay = min(baseDelay * pow(multiplier, Double(currentAttempt)), maxDelay)
        currentAttempt += 1
        return delay
    }

    /// Reset backoff counter (call after successful connection).
    func reset() {
        currentAttempt = 0
        isReconnecting = false
    }

    /// Attempt reconnection with backoff.
    /// Calls the provided closure after waiting the backoff delay.
    /// Per D-07: always performs full re-handshake (stateless).
    func scheduleReconnect(action: @escaping @Sendable () async throws -> Void) async {
        guard !isReconnecting else { return }
        isReconnecting = true

        let delay = nextDelay()
        try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))

        guard !Task.isCancelled else {
            isReconnecting = false
            return
        }

        do {
            try await action()
            reset()  // Success -- reset backoff
        } catch {
            isReconnecting = false
            // Caller should call scheduleReconnect again
        }
    }
}
