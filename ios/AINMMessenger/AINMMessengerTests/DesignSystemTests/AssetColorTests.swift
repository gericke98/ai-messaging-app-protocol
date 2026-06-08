import XCTest
import UIKit
@testable import AINMMessenger

final class AssetColorTests: XCTestCase {
    private let semantic = [
        "Paper", "Surface", "SurfaceBorder", "InkPrimary", "InkSecondary",
        "HumanBubble", "HumanBubbleText", "Accent", "AccentSoft", "Separator",
    ]

    func testAllSemanticColorsResolve() {
        for name in semantic {
            XCTAssertNotNil(
                UIColor(named: name, in: .designSystem, compatibleWith: nil),
                "Missing semantic color asset: \(name)"
            )
        }
    }

    func testAllIdentityColorsResolve() {
        for i in 0..<10 {
            XCTAssertNotNil(
                UIColor(named: "Identity\(i)", in: .designSystem, compatibleWith: nil),
                "Missing identity color asset: Identity\(i)"
            )
        }
    }
}
