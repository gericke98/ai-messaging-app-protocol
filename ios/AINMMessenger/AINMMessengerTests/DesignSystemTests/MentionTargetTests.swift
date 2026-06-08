import XCTest
@testable import AINMMessenger

final class MentionTargetTests: XCTestCase {

    private func textBlock(_ s: String, mentions: [ContentBlock.Mention]?) -> ContentBlock {
        .text(.init(version: "1.0", source: "agent", text: s, mentions: mentions, fallback: s))
    }

    func testReturnsFirstMentionAddress() {
        let blocks = [textBlock("hey @researcher", mentions: [
            .init(address: "researcher@acme.test", offset: 4, length: 11)
        ])]
        XCTAssertEqual(MentionTarget.firstMentionAddress(in: blocks), "researcher@acme.test")
    }

    func testReturnsLowestOffsetMentionWithinABlock() {
        let blocks = [textBlock("@b and @a", mentions: [
            .init(address: "a@host", offset: 7, length: 2),
            .init(address: "b@host", offset: 0, length: 2)
        ])]
        XCTAssertEqual(MentionTarget.firstMentionAddress(in: blocks), "b@host")
    }

    func testNilWhenNoMentions() {
        XCTAssertNil(MentionTarget.firstMentionAddress(in: [textBlock("plain", mentions: nil)]))
        XCTAssertNil(MentionTarget.firstMentionAddress(in: [textBlock("empty", mentions: [])]))
    }

    func testScansToFirstBlockThatHasMentions() {
        let blocks = [
            textBlock("no mentions here", mentions: nil),
            textBlock("now @x", mentions: [.init(address: "x@host", offset: 4, length: 2)])
        ]
        XCTAssertEqual(MentionTarget.firstMentionAddress(in: blocks), "x@host")
    }

    func testNilForNonTextBlocks() {
        let code = ContentBlock.code(.init(version: "1.0", source: "agent", language: "swift",
                                           code: "let x = 1", filename: nil, fallback: "code"))
        XCTAssertNil(MentionTarget.firstMentionAddress(in: [code]))
    }
}
