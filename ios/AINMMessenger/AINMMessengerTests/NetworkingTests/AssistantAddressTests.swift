import XCTest
@testable import AINMMessenger

/// Unit tests for `AppState.assistantAddress(forServerURL:)` — the pure helper that
/// derives the built-in agent address `assistant@<host>` from a WebSocket server URL.
/// The host must equal the server's `server_domain` (account binding is `username@<host>`).
final class AssistantAddressTests: XCTestCase {
    func testDerivesAssistantAddressFromServerURL() {
        XCTAssertEqual(AppState.assistantAddress(forServerURL: "ws://localhost:8080"), "assistant@localhost")
        XCTAssertEqual(AppState.assistantAddress(forServerURL: "wss://ainm.example.com:443"), "assistant@ainm.example.com")
    }

    func testNilWhenNoHost() {
        XCTAssertNil(AppState.assistantAddress(forServerURL: "not a url"))
    }
}
