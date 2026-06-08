import XCTest
import SwiftUI
@testable import AINMMessenger

final class ThemeTokenTests: XCTestCase {
    func testSpacingScale() {
        XCTAssertEqual(Theme.Spacing.xs, 4)
        XCTAssertEqual(Theme.Spacing.sm, 8)
        XCTAssertEqual(Theme.Spacing.md, 12)
        XCTAssertEqual(Theme.Spacing.lg, 16)
        XCTAssertEqual(Theme.Spacing.xl, 20)
        XCTAssertEqual(Theme.Spacing.xxl, 24)
    }

    func testRadiusScale() {
        XCTAssertEqual(Theme.Radius.chip, 8)
        XCTAssertEqual(Theme.Radius.card, 12)
        XCTAssertEqual(Theme.Radius.bubble, 16)
    }

    func testIdentityPaletteHasTenColors() {
        XCTAssertEqual(Theme.Color.identityPalette.count, 10)
    }
}
