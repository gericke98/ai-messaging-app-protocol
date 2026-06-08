import XCTest
@testable import AINMMessenger

final class ParticipantClassifierTests: XCTestCase {

    func testSelfIsAlwaysHuman() {
        let t = ParticipantClassifier.classify(
            address: "me@localhost", isSelf: true, agentAddress: "me@localhost",
            savedAgentAddresses: ["me@localhost"])
        XCTAssertEqual(t, "human")   // self precedence beats every agent signal
    }

    func testThreadAgentAddressIsAgent() {
        let t = ParticipantClassifier.classify(
            address: "assistant@localhost", isSelf: false, agentAddress: "assistant@localhost",
            savedAgentAddresses: [])
        XCTAssertEqual(t, "agent")
    }

    func testSavedAgentIsAgent() {
        let t = ParticipantClassifier.classify(
            address: "researcher@acme.test", isSelf: false, agentAddress: nil,
            savedAgentAddresses: ["researcher@acme.test"])
        XCTAssertEqual(t, "agent")
    }

    func testUnknownDefaultsHuman() {
        let t = ParticipantClassifier.classify(
            address: "stranger@host", isSelf: false, agentAddress: "assistant@localhost",
            savedAgentAddresses: ["researcher@acme.test"])
        XCTAssertEqual(t, "human")
    }

    func testNoSignalsDefaultsHuman() {
        let t = ParticipantClassifier.classify(
            address: "stranger@host", isSelf: false, agentAddress: nil,
            savedAgentAddresses: [])
        XCTAssertEqual(t, "human")
    }
}
