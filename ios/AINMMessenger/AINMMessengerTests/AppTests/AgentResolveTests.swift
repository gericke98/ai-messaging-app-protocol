import XCTest
@testable import AINMMessenger

final class AgentResolveTests: XCTestCase {
    func testBareNameDefaultsToServerHost() {
        let p = AppState.parseAgentInput("claude-code", serverHost: "localhost")
        XCTAssertEqual(p?.address, "claude-code@localhost")
        XCTAssertNil(p?.directCardURL)
    }

    func testFullAddressPassesThrough() {
        let p = AppState.parseAgentInput("helper@localhost", serverHost: "localhost")
        XCTAssertEqual(p?.address, "helper@localhost")
        XCTAssertNil(p?.directCardURL)
    }

    func testCardURLDerivesAddressAndKeepsURL() {
        let url = "http://localhost/.well-known/ainm/helper/agent-card.json"
        let p = AppState.parseAgentInput(url, serverHost: "localhost")
        XCTAssertEqual(p?.address, "helper@localhost")
        XCTAssertEqual(p?.directCardURL, url)
    }

    func testWhitespaceTrimmedAndEmptyRejected() {
        XCTAssertEqual(AppState.parseAgentInput("  helper@localhost  ", serverHost: "localhost")?.address,
                       "helper@localhost")
        XCTAssertNil(AppState.parseAgentInput("   ", serverHost: "localhost"))
        XCTAssertNil(AppState.parseAgentInput("name@", serverHost: "localhost"))
        XCTAssertNil(AppState.parseAgentInput("@host", serverHost: "localhost"))
    }

    func testMalformedCardURLRejected() {
        XCTAssertNil(AppState.parseAgentInput("http://localhost/wrong/path.json", serverHost: "localhost"))
    }
}
