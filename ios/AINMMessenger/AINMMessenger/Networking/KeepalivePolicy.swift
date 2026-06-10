import Foundation

/// Keepalive policy per transport.md Section 4.4: the client SHOULD send
/// `ainm.ping` every 30 seconds during idle periods, and treat a missing
/// `ainm.pong` within 10 seconds as a dead connection (then reconnect).
struct KeepalivePolicy: Sendable {
    /// Idle window before a ping is sent.
    var interval: TimeInterval = 30
    /// How long to wait for the pong before declaring the connection dead.
    var pongTimeout: TimeInterval = 10

    /// True when no inbound traffic has arrived for at least `interval`.
    /// Inbound frames are the liveness signal — a chatty connection needs no pings.
    func isIdle(lastTraffic: Date, now: Date) -> Bool {
        now.timeIntervalSince(lastTraffic) >= interval
    }

    /// What to do when the app returns to the foreground. iOS freezes timers
    /// during suspension, so the keepalive loop and the bounded reconnect loop
    /// can both be stale the moment the user comes back.
    enum ForegroundAction: Equatable {
        /// The socket looks connected — ping immediately (a socket killed
        /// during suspension is detected in ≤pongTimeout, not interval+timeout).
        case verifyNow
        /// Dead with no recovery in flight — start a fresh reconnect cycle.
        case reconnect
        /// Recovery already running, or the user signed out — do nothing.
        case none
    }

    static func foregroundAction(
        connected: Bool, reconnecting: Bool, userDisconnected: Bool
    ) -> ForegroundAction {
        if connected { return .verifyNow }
        if reconnecting || userDisconnected { return .none }
        return .reconnect
    }

    /// The JSON-RPC ping request frame (answered server-side by `send_pong`).
    static func pingFrame(id: Int) -> [String: Any] {
        [
            "jsonrpc": "2.0",
            "id": id,
            "method": "ainm.ping",
            "params": [String: String](),
        ]
    }
}
