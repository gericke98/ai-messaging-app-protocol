import XCTest
@testable import AINMMessenger

final class MessageGroupDirectedToTests: XCTestCase {

    private func mentionBlocks(_ address: String) -> [ContentBlock] {
        [.text(.init(version: "1.0", source: "agent", text: "@x",
                     mentions: [.init(address: address, offset: 0, length: 2)], fallback: "@x"))]
    }
    private let plainBlocks: [ContentBlock] =
        [.text(.init(version: "1.0", source: "agent", text: "hello", mentions: nil, fallback: "hello"))]

    func testToAddressWins() {
        let result = MessageGroup.deriveDirectedTo(
            toAddress: "researcher@acme.test", contentBlocks: mentionBlocks("other@host"),
            senderAddress: "assistant@localhost", currentUserIdentity: "me@localhost"
        )
        XCTAssertEqual(result, "researcher@acme.test")
    }

    func testFallsBackToFirstMentionWhenToAddressNil() {
        let result = MessageGroup.deriveDirectedTo(
            toAddress: nil, contentBlocks: mentionBlocks("researcher@acme.test"),
            senderAddress: "assistant@localhost", currentUserIdentity: "me@localhost"
        )
        XCTAssertEqual(result, "researcher@acme.test")
    }

    func testEmptyToAddressFallsThroughToMention() {
        let result = MessageGroup.deriveDirectedTo(
            toAddress: "", contentBlocks: mentionBlocks("researcher@acme.test"),
            senderAddress: "assistant@localhost", currentUserIdentity: "me@localhost"
        )
        XCTAssertEqual(result, "researcher@acme.test")
    }

    func testNilWhenNoTargetAtAll() {
        let result = MessageGroup.deriveDirectedTo(
            toAddress: nil, contentBlocks: plainBlocks,
            senderAddress: "assistant@localhost", currentUserIdentity: "me@localhost"
        )
        XCTAssertNil(result)
    }

    func testSelfTargetSuppressed() {
        let result = MessageGroup.deriveDirectedTo(
            toAddress: "assistant@localhost", contentBlocks: plainBlocks,
            senderAddress: "assistant@localhost", currentUserIdentity: "me@localhost"
        )
        XCTAssertNil(result)
    }

    func testCurrentUserTargetSuppressed() {
        let result = MessageGroup.deriveDirectedTo(
            toAddress: "me@localhost", contentBlocks: plainBlocks,
            senderAddress: "assistant@localhost", currentUserIdentity: "me@localhost"
        )
        XCTAssertNil(result)
    }
}
