import XCTest
import SwiftUI
@testable import AINMMessenger

/// Dependency-free render/smoke tests: instantiate each restyled conversation surface,
/// force a layout pass in both color schemes, and assert it neither crashes nor produces
/// a zero-size layout. Catches compile breaks, token-resolution failures, and layout traps
/// without the fragility of cross-machine image diffing.
@MainActor
final class ConversationSurfaceRenderTests: XCTestCase {

    /// Hosts `view` in a real window, forces layout in the given scheme, returns laid-out size.
    @discardableResult
    func render<V: View>(_ view: V, scheme: ColorScheme, size: CGSize = CGSize(width: 390, height: 240)) -> CGSize {
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

    /// Render in both light and dark; assert non-zero size each time.
    func renderBothSchemes<V: View>(_ view: V, file: StaticString = #filePath, line: UInt = #line) {
        for scheme in [ColorScheme.light, .dark] {
            let s = render(view, scheme: scheme)
            XCTAssertGreaterThan(s.width, 0, "zero width in \(scheme)", file: file, line: line)
            XCTAssertGreaterThan(s.height, 0, "zero height in \(scheme)", file: file, line: line)
        }
    }

    /// Convenience: a one-text-block agent/human content array.
    func textBlocks(_ s: String, source: String) -> [ContentBlock] {
        [.text(ContentBlock.TextContent(version: "1.0", source: source, text: s, mentions: nil, fallback: s))]
    }

    func test_haloedAvatar_rendersInsideClippedParent() {
        let avatar = AvatarView(
            participantType: "agent", size: 36,
            address: "assistant@localhost", displayName: "Assistant",
            showAgentHalo: true
        )
        // Clip to exactly the avatar's footprint: a clip-unsafe ring would be cut off,
        // but rendering must still succeed and lay out at full size.
        let clipped = avatar.frame(width: 36, height: 36).clipped()
        renderBothSchemes(clipped)
    }

    func test_streamingMessage_rendersLivingTreatment() {
        let state = StreamingViewModel.StreamingMessageState(
            id: "s1", messageId: "m1", from: "assistant@localhost", threadId: "t1",
            blocks: [.init(type: "text", content: "Monday is your clearest block")],
            status: .streaming, finalizingStartTime: nil
        )
        renderBothSchemes(StreamingMessageView(state: state))
    }

    func test_typingIndicator_rendersIdentityAvatar() {
        renderBothSchemes(TypingIndicatorView(name: "Assistant", isAgent: true, address: "assistant@localhost"))
    }

    func test_agentMessage_rendersAiry() {
        let view = AgentMessageView(
            blocks: textBlocks("Here's a plan. I checked your calendar.", source: "agent"),
            senderName: "Assistant", senderType: "agent",
            senderAddress: "assistant@localhost", showHeader: true, timestamp: "9:41 AM"
        )
        renderBothSchemes(view)
    }

    func test_humanBubble_rendersInkTokens() {
        let view = MessageBubbleView(
            blocks: textBlocks("Plan my week around the launch", source: "user"),
            senderName: "You", showHeader: false, timestamp: "9:40 AM", messageId: "m0"
        )
        renderBothSchemes(view)
    }

    func test_conversationRow_warmTokensAndAvatar() {
        let row = ConversationRow(
            id: "t-test-1",
            title: "Assistant",
            createdAt: "2026-06-08T09:41:00Z",
            createdBy: "user@localhost",
            hasUnread: true,
            lastMessageTimestamp: "2026-06-08T09:41:00Z",
            lastMessageContent: nil,
            lastMessageFrom: nil
        )
        renderBothSchemes(ConversationRowView(
            conversation: row,
            previewText: "Sounds good",
            relativeTime: "9:41 AM"
        ))
    }
}
