import Foundation
import Testing
@testable import AINMMessenger

/// Keepalive policy (transport.md Section 4.4): ping every 30s during idle;
/// no pong within 10s means the connection is dead and the client reconnects.
@Suite("Keepalive")
struct KeepaliveTests {
    @Test("spec defaults: 30s idle interval, 10s pong timeout")
    func specDefaults() {
        let policy = KeepalivePolicy()
        #expect(policy.interval == 30)
        #expect(policy.pongTimeout == 10)
    }

    @Test("idle exactly at and beyond the interval; not before")
    func idleBoundaries() {
        let policy = KeepalivePolicy()
        let last = Date(timeIntervalSince1970: 1_000)
        #expect(policy.isIdle(lastTraffic: last, now: last.addingTimeInterval(30)))
        #expect(policy.isIdle(lastTraffic: last, now: last.addingTimeInterval(31)))
        #expect(!policy.isIdle(lastTraffic: last, now: last.addingTimeInterval(29.9)))
    }

    @Test("foreground action: verify live sockets, restart dead ones, respect sign-out")
    func foregroundActions() {
        // Connected: the socket may have died during suspension — ping NOW
        // instead of waiting out the idle window.
        #expect(
            KeepalivePolicy.foregroundAction(
                connected: true, reconnecting: false, userDisconnected: false) == .verifyNow)
        // Dead with no loop running (e.g. the bounded reconnect exhausted its
        // attempts against a frozen radio while suspended): start over.
        #expect(
            KeepalivePolicy.foregroundAction(
                connected: false, reconnecting: false, userDisconnected: false) == .reconnect)
        // A reconnect loop already in flight: leave it alone.
        #expect(
            KeepalivePolicy.foregroundAction(
                connected: false, reconnecting: true, userDisconnected: false) == .none)
        // The user signed out: foreground must NOT resurrect the session.
        #expect(
            KeepalivePolicy.foregroundAction(
                connected: false, reconnecting: false, userDisconnected: true) == .none)
    }

    @Test("ping frame is a JSON-RPC request the server's send_pong answers")
    func pingFrameShape() throws {
        let frame = KeepalivePolicy.pingFrame(id: 99)
        #expect(frame["jsonrpc"] as? String == "2.0")
        #expect(frame["id"] as? Int == 99)
        #expect(frame["method"] as? String == "ainm.ping")
        #expect(frame["params"] != nil)
        // Must serialize (the client sends it via JSONSerialization).
        let data = try JSONSerialization.data(withJSONObject: frame)
        #expect(!data.isEmpty)
    }
}
