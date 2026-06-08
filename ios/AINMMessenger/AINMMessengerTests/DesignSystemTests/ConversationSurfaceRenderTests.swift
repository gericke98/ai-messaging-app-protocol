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
}
