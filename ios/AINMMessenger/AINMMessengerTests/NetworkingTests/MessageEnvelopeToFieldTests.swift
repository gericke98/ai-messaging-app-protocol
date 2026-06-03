import XCTest
@testable import AINMMessenger

final class MessageEnvelopeToFieldTests: XCTestCase {
    private func sample(to: String?) -> MessageEnvelope {
        MessageEnvelope(
            id: "m1", from: "alice@localhost", threadId: "t1",
            timestamp: "2026-06-02T00:00:00.000Z", type: .message,
            content: [], signature: nil, parentId: nil, to: to
        )
    }
    func testToEncodedWhenPresent() throws {
        let data = try JSONEncoder().encode(sample(to: "assistant@localhost"))
        let json = try JSONSerialization.jsonObject(with: data) as! [String: Any]
        XCTAssertEqual(json["to"] as? String, "assistant@localhost")
    }
    func testToOmittedWhenNil() throws {
        let data = try JSONEncoder().encode(sample(to: nil))
        let json = try JSONSerialization.jsonObject(with: data) as! [String: Any]
        XCTAssertNil(json["to"], "`to` must be omitted (not null) when unset")
    }
    func testRoundTrip() throws {
        let data = try JSONEncoder().encode(sample(to: "assistant@localhost"))
        let decoded = try JSONDecoder().decode(MessageEnvelope.self, from: data)
        XCTAssertEqual(decoded.to, "assistant@localhost")
    }
}
