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
