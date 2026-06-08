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
}
