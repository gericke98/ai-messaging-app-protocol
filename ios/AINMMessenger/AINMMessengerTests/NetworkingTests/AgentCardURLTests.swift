import XCTest
@testable import AINMMessenger

/// Locks the client/server agent-card contract: the client must build the SAME path the
/// server serves (`/.well-known/ainm/{name}/agent-card.json` — no `agents/` segment) and
/// must map the WebSocket scheme to HTTP correctly (ws→http for the localhost dev server).
final class AgentCardURLTests: XCTestCase {
    func testCanonicalCardURLMatchesServerRoute() {
        XCTAssertEqual(
            AgentCardService.canonicalCardURL(forAddress: "assistant@localhost", scheme: "http"),
            "http://localhost/.well-known/ainm/assistant/agent-card.json"
        )
        XCTAssertEqual(
            AgentCardService.canonicalCardURL(forAddress: "claude-code@ainm.example.com", scheme: "https"),
            "https://ainm.example.com/.well-known/ainm/claude-code/agent-card.json"
        )
    }

    func testCanonicalCardURLRejectsMalformed() {
        XCTAssertNil(AgentCardService.canonicalCardURL(forAddress: "nohost", scheme: "https"))
        XCTAssertNil(AgentCardService.canonicalCardURL(forAddress: "@localhost", scheme: "https"))
        XCTAssertNil(AgentCardService.canonicalCardURL(forAddress: "name@", scheme: "https"))
    }

    func testHttpSchemeFromServerURL() {
        XCTAssertEqual(AgentCardService.httpScheme(forServerURL: "ws://localhost:8080"), "http")
        XCTAssertEqual(AgentCardService.httpScheme(forServerURL: "wss://ainm.example.com:443"), "https")
        XCTAssertEqual(AgentCardService.httpScheme(forServerURL: "https://x"), "https")
    }
}
