import XCTest
import SwiftUI
@testable import AINMMessenger

@MainActor
final class A2ACapsuleRenderTests: XCTestCase {

    @discardableResult
    func render<V: View>(_ view: V, scheme: ColorScheme,
                         size: CGSize = CGSize(width: 390, height: 300)) -> CGSize {
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

    private func msg(_ id: String, _ from: String) -> MessageRecord {
        MessageRecord(id: id, threadId: "t", fromAddress: from, timestamp: "2026-06-08T10:00:00.000Z",
                      type: "message", contentJson: "[]", signature: "s", serverTimestamp: nil)
    }
    private func grp(_ id: String, _ who: String) -> MessageGroup {
        MessageGroup(id: id, senderAddress: who, senderType: "agent",
                     senderName: who.components(separatedBy: "@").first ?? who,
                     messages: [msg("\(id)-m", who)], isFromCurrentUser: false, directedTo: nil)
    }
    private func capsule(_ agents: [String]) -> A2ACapsule {
        let groups = agents.enumerated().map { grp("g\($0.offset)", $0.element) }
        return A2ACapsule(id: "g0", groups: groups, agents: agents, messageCount: max(3, agents.count))
    }

    func test_collapsed_twoAgents() {
        renderBothSchemes(A2ACapsuleView(
            capsule: capsule(["assistant@localhost", "researcher@localhost"]),
            isExpanded: false, homeHost: "localhost", onToggle: {},
            groupContent: { Text($0.senderName) }))
    }

    func test_collapsed_threeAgentsShowsOverflow() {
        renderBothSchemes(A2ACapsuleView(
            capsule: capsule(["assistant@localhost", "researcher@localhost", "analyst@acme.test", "scribe@localhost"]),
            isExpanded: false, homeHost: "localhost", onToggle: {},
            groupContent: { Text($0.senderName) }))
    }

    func test_expanded_rendersGroupContent() {
        renderBothSchemes(A2ACapsuleView(
            capsule: capsule(["assistant@localhost", "researcher@localhost"]),
            isExpanded: true, homeHost: "localhost", onToggle: {},
            groupContent: { Text($0.senderName).font(.body) }))
    }

    func test_capsule_rendersUnderReduceMotion() {
        UIView.setAnimationsEnabled(false)
        defer { UIView.setAnimationsEnabled(true) }
        renderBothSchemes(A2ACapsuleView(
            capsule: capsule(["assistant@localhost", "researcher@localhost"]),
            isExpanded: false, homeHost: "localhost", onToggle: {},
            groupContent: { Text($0.senderName) }))
    }
}
