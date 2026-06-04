import XCTest
@testable import AINMMessenger

/// Locks the client/server agent-card contract: the client must build the SAME path the
/// server serves (`/.well-known/ainm/{name}/agent-card.json` — no `agents/` segment) and
/// must map the WebSocket scheme to HTTP correctly (ws→http for the localhost dev server).
final class AgentCardURLTests: XCTestCase {
    func testCanonicalCardURLUsesServerSchemeHostAndPort() {
        // ws://localhost:8080 → http + preserves port 8080 (the dev-server case the bug missed).
        XCTAssertEqual(
            AgentCardService.canonicalCardURL(forAddress: "assistant@localhost", serverURL: "ws://localhost:8080"),
            "http://localhost:8080/.well-known/ainm/assistant/agent-card.json"
        )
        // wss://host:443 → https, explicit port preserved.
        XCTAssertEqual(
            AgentCardService.canonicalCardURL(forAddress: "claude-code@ainm.example.com", serverURL: "wss://ainm.example.com:443"),
            "https://ainm.example.com:443/.well-known/ainm/claude-code/agent-card.json"
        )
        // wss://host with no explicit port → https, no port segment.
        XCTAssertEqual(
            AgentCardService.canonicalCardURL(forAddress: "a@host", serverURL: "wss://host"),
            "https://host/.well-known/ainm/a/agent-card.json"
        )
    }

    func testCanonicalCardURLRejectsMalformed() {
        XCTAssertNil(AgentCardService.canonicalCardURL(forAddress: "nohost", serverURL: "ws://localhost:8080"))
        XCTAssertNil(AgentCardService.canonicalCardURL(forAddress: "@localhost", serverURL: "ws://localhost:8080"))
        XCTAssertNil(AgentCardService.canonicalCardURL(forAddress: "name@", serverURL: "ws://localhost:8080"))
        XCTAssertNil(AgentCardService.canonicalCardURL(forAddress: "a@host", serverURL: "garbage"))
    }

    func testHttpSchemeFromServerURL() {
        XCTAssertEqual(AgentCardService.httpScheme(forServerURL: "ws://localhost:8080"), "http")
        XCTAssertEqual(AgentCardService.httpScheme(forServerURL: "wss://ainm.example.com:443"), "https")
        XCTAssertEqual(AgentCardService.httpScheme(forServerURL: "https://x"), "https")
    }
}
