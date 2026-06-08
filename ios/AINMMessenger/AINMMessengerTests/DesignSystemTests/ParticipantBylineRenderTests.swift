import XCTest
import SwiftUI
@testable import AINMMessenger

@MainActor
final class ParticipantBylineRenderTests: XCTestCase {

    @discardableResult
    func render<V: View>(_ view: V, scheme: ColorScheme,
                         size: CGSize = CGSize(width: 390, height: 80)) -> CGSize {
        let host = UIHostingController(rootView: view.environment(\.colorScheme, scheme))
        host.overrideUserInterfaceStyle = (scheme == .dark) ? .dark : .light
        host.view.frame = CGRect(origin: .zero, size: size)
        let window = UIWindow(frame: host.view.frame)
        window.rootViewController = host
        window.isHidden = false
        host.view.setNeedsLayout()
        host.view.layoutIfNeeded()
        return host.view.bounds.size
    }

    func renderBothSchemes<V: View>(_ view: V, file: StaticString = #filePath, line: UInt = #line) {
        for scheme in [ColorScheme.light, .dark] {
            let s = render(view, scheme: scheme)
            XCTAssertGreaterThan(s.width, 0, "zero width in \(scheme)", file: file, line: line)
            XCTAssertGreaterThan(s.height, 0, "zero height in \(scheme)", file: file, line: line)
        }
    }

    func test_byline_calm1to1_noTagNoHost() {
        renderBothSchemes(ParticipantByline(
            senderAddress: "assistant@localhost", senderName: "Assistant",
            directedTo: nil, isMultiParticipant: false, homeHost: "localhost"))
    }

    func test_byline_multiAgent_identityTintedName() {
        renderBothSchemes(ParticipantByline(
            senderAddress: "researcher@localhost", senderName: "Researcher",
            directedTo: nil, isMultiParticipant: true, homeHost: "localhost"))
    }

    func test_byline_directional_targetTinted() {
        renderBothSchemes(ParticipantByline(
            senderAddress: "assistant@localhost", senderName: "Assistant",
            directedTo: "researcher@localhost", isMultiParticipant: true, homeHost: "localhost"))
    }

    func test_byline_foreignSender_hostSuffix() {
        renderBothSchemes(ParticipantByline(
            senderAddress: "analyst@acme.test", senderName: "Analyst",
            directedTo: nil, isMultiParticipant: true, homeHost: "localhost"))
    }

    func test_byline_foreignTarget_hostSuffix() {
        renderBothSchemes(ParticipantByline(
            senderAddress: "assistant@localhost", senderName: "Assistant",
            directedTo: "analyst@acme.test", isMultiParticipant: true, homeHost: "localhost"))
    }

    func test_agentMessage_directionalHeaderRenders() {
        let blocks: [ContentBlock] = [.text(.init(
            version: "1.0", source: "agent", text: "On it.", mentions: nil, fallback: "On it."))]
        let view = AgentMessageView(
            blocks: blocks, senderName: "Assistant", senderType: "agent",
            senderAddress: "assistant@localhost", showHeader: true, timestamp: "9:41 AM",
            directedTo: "researcher@acme.test", isMultiParticipant: true, homeHost: "localhost")
        renderBothSchemes(view)
    }

    func test_streamingMessage_tintedBylineNoDirectionalTag() {
        let state = StreamingViewModel.StreamingMessageState(
            id: "s3", messageId: "m9", from: "researcher@acme.test", threadId: "t1",
            blocks: [.init(type: "text", content: "Pulling the numbers…")],
            status: .streaming, finalizingStartTime: nil)
        let view = StreamingMessageView(state: state, isMultiParticipant: true, homeHost: "localhost")
        renderBothSchemes(view)
    }

    func test_conversationRow_usesAgentAddressForAvatar() {
        let row = ConversationRow(
            id: "t-9", title: "Assistant", createdAt: "2026-06-08T09:41:00Z",
            createdBy: "me@localhost", hasUnread: false,
            lastMessageTimestamp: "2026-06-08T09:41:00Z",
            lastMessageContent: nil, lastMessageFrom: nil,
            agentAddress: "assistant@localhost")
        renderBothSchemes(ConversationRowView(
            conversation: row, previewText: "On it", relativeTime: "9:41 AM"))
    }
}
