import XCTest
@testable import AINMMessenger

final class A2ACapsuleFolderTests: XCTestCase {

    let me = "me@localhost"
    let assistant = "assistant@localhost"
    let researcher = "researcher@localhost"

    /// Build a MessageGroup with `msgCount` messages from `sender`.
    private func group(id: String, sender: String, type: String, fromUser: Bool,
                       directedTo: String?, msgCount: Int) -> MessageGroup {
        let msgs = (0..<msgCount).map { k in
            MessageRecord(id: "\(id)-m\(k)", threadId: "t", fromAddress: sender,
                          timestamp: "2026-06-08T10:00:0\(k).000Z", type: "message",
                          contentJson: "[]", signature: "s", serverTimestamp: nil)
        }
        return MessageGroup(id: id, senderAddress: sender, senderType: type,
                            senderName: sender.components(separatedBy: "@").first ?? sender,
                            messages: msgs, isFromCurrentUser: fromUser, directedTo: directedTo)
    }
    private func userGroup(_ id: String) -> MessageGroup {
        group(id: id, sender: me, type: "human", fromUser: true, directedTo: nil, msgCount: 1)
    }
    private func agentGroup(_ id: String, _ who: String, directedTo: String? = nil, msgCount: Int = 1) -> MessageGroup {
        group(id: id, sender: who, type: "agent", fromUser: false, directedTo: directedTo, msgCount: msgCount)
    }

    private func capsules(_ items: [ThreadItem]) -> [A2ACapsule] {
        items.compactMap { if case .capsule(let c) = $0 { return c } else { return nil } }
    }

    func test_boundedRunOf3WithTwoAgents_folds() {
        let groups = [
            userGroup("u1"),
            agentGroup("a1", assistant), agentGroup("a2", researcher), agentGroup("a3", assistant),
            agentGroup("a4", assistant, directedTo: me)   // to-human → bounds + stays inline
        ]
        let items = A2ACapsuleFolder.fold(groups: groups, currentUserIdentity: me, isMultiParticipant: true)
        let caps = capsules(items)
        XCTAssertEqual(caps.count, 1)
        XCTAssertEqual(caps[0].id, "a1")
        XCTAssertEqual(caps[0].messageCount, 3)
        XCTAssertEqual(caps[0].agents, [assistant, researcher])
        XCTAssertEqual(items.count, 3)   // .group(u1), .capsule, .group(a4)
    }

    func test_runOf2_doesNotFold() {
        let groups = [userGroup("u1"), agentGroup("a1", assistant), agentGroup("a2", researcher), userGroup("u2")]
        let items = A2ACapsuleFolder.fold(groups: groups, currentUserIdentity: me, isMultiParticipant: true)
        XCTAssertTrue(capsules(items).isEmpty)
    }

    func test_singleAgentRun_doesNotFold() {
        let groups = [userGroup("u1"), agentGroup("a1", assistant, msgCount: 3), userGroup("u2")]
        let items = A2ACapsuleFolder.fold(groups: groups, currentUserIdentity: me, isMultiParticipant: true)
        XCTAssertTrue(capsules(items).isEmpty)
    }

    func test_notMultiParticipant_neverFolds() {
        let groups = [userGroup("u1"),
                      agentGroup("a1", assistant), agentGroup("a2", researcher), agentGroup("a3", assistant),
                      userGroup("u2")]
        let items = A2ACapsuleFolder.fold(groups: groups, currentUserIdentity: me, isMultiParticipant: false)
        XCTAssertTrue(capsules(items).isEmpty)
        XCTAssertEqual(items.count, groups.count)
    }

    func test_toHumanAgentMessageBoundsRun() {
        let groups = [userGroup("u1"),
                      agentGroup("a1", assistant), agentGroup("a2", researcher),
                      agentGroup("a3", assistant, directedTo: me),
                      userGroup("u2")]
        let items = A2ACapsuleFolder.fold(groups: groups, currentUserIdentity: me, isMultiParticipant: true)
        XCTAssertTrue(capsules(items).isEmpty)
    }

    func test_trailingUnboundedRun_staysInline() {
        let groups = [userGroup("u1"),
                      agentGroup("a1", assistant), agentGroup("a2", researcher), agentGroup("a3", assistant)]
        let items = A2ACapsuleFolder.fold(groups: groups, currentUserIdentity: me, isMultiParticipant: true)
        XCTAssertTrue(capsules(items).isEmpty)
        XCTAssertEqual(items.count, 4)
    }

    func test_twoSeparateBursts_foldIndependently() {
        let groups = [
            userGroup("u1"),
            agentGroup("a1", assistant), agentGroup("a2", researcher), agentGroup("a3", assistant),
            userGroup("u2"),
            agentGroup("b1", researcher), agentGroup("b2", assistant), agentGroup("b3", researcher),
            userGroup("u3")
        ]
        let items = A2ACapsuleFolder.fold(groups: groups, currentUserIdentity: me, isMultiParticipant: true)
        let caps = capsules(items)
        XCTAssertEqual(caps.count, 2)
        XCTAssertEqual(caps[0].id, "a1")
        XCTAssertEqual(caps[1].id, "b1")
    }
}
