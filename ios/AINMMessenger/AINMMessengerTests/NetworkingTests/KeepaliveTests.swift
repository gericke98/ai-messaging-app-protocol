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
