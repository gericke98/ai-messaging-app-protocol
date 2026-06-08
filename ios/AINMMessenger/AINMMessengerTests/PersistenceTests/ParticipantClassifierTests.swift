import XCTest
@testable import AINMMessenger

final class ParticipantClassifierTests: XCTestCase {

    func testSelfIsAlwaysHuman() {
        let t = ParticipantClassifier.classify(
            address: "me@localhost", isSelf: true, agentAddress: "me@localhost",
            savedAgentAddresses: ["me@localhost"], contentSource: "agent")
        XCTAssertEqual(t, "human")   // self precedence beats every agent signal
    }

    func testThreadAgentAddressIsAgent() {
        let t = ParticipantClassifier.classify(
            address: "assistant@localhost", isSelf: false, agentAddress: "assistant@localhost",
            savedAgentAddresses: [], contentSource: nil)
        XCTAssertEqual(t, "agent")
    }

    func testSavedAgentIsAgent() {
        let t = ParticipantClassifier.classify(
            address: "researcher@acme.test", isSelf: false, agentAddress: nil,
            savedAgentAddresses: ["researcher@acme.test"], contentSource: nil)
        XCTAssertEqual(t, "agent")
    }

    func testContentSourceAgentIsAgent() {
        let t = ParticipantClassifier.classify(
            address: "x@host", isSelf: false, agentAddress: nil,
            savedAgentAddresses: [], contentSource: "agent")
        XCTAssertEqual(t, "agent")
    }

    func testUnknownDefaultsHuman() {
        let t = ParticipantClassifier.classify(
            address: "stranger@host", isSelf: false, agentAddress: "assistant@localhost",
            savedAgentAddresses: ["researcher@acme.test"], contentSource: "user")
        XCTAssertEqual(t, "human")
    }

    func testNilContentSourceStillResolves() {
        let t = ParticipantClassifier.classify(
            address: "stranger@host", isSelf: false, agentAddress: nil,
            savedAgentAddresses: [], contentSource: nil)
        XCTAssertEqual(t, "human")
    }
}
