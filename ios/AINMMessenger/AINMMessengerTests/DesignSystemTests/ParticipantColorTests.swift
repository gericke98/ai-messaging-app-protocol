import XCTest
@testable import AINMMessenger

final class ParticipantColorTests: XCTestCase {

    // Golden FNV-1a 32-bit indices (count = 10), computed offline. These lock the algorithm:
    // a change to the hash would change these and fail the test.
    func testPaletteIndexGoldenValues() {
        XCTAssertEqual(ParticipantColor.paletteIndex(for: "assistant@localhost", count: 10), 8)
        XCTAssertEqual(ParticipantColor.paletteIndex(for: "researcher@acme.test", count: 10), 5)
    }

    func testPaletteIndexIsDeterministic() {
        for _ in 0..<100 {
            XCTAssertEqual(
                ParticipantColor.paletteIndex(for: "sam@home.test", count: 10),
                ParticipantColor.paletteIndex(for: "sam@home.test", count: 10)
            )
        }
    }

    func testPaletteIndexInRange() {
        for i in 0..<200 {
            let idx = ParticipantColor.paletteIndex(for: "user\(i)@host.test", count: 10)
            XCTAssertTrue((0..<10).contains(idx))
        }
    }

    func testDistributionCoversManySlots() {
        var slots = Set<Int>()
        for i in 0..<50 {
            slots.insert(ParticipantColor.paletteIndex(for: "user\(i)@host\(i % 7).test", count: 10))
        }
        XCTAssertGreaterThanOrEqual(slots.count, 6, "Hash distribution is degenerate: \(slots)")
    }

    func testInitials() {
        XCTAssertEqual(ParticipantColor.initials(for: "Sam Rivera"), "SR")
        XCTAssertEqual(ParticipantColor.initials(for: "assistant"), "A")
        XCTAssertEqual(ParticipantColor.initials(for: "researcher@acme.test"), "R")
        XCTAssertEqual(ParticipantColor.initials(for: "Sam"), "S")
        XCTAssertEqual(ParticipantColor.initials(for: ""), "?")
        XCTAssertEqual(ParticipantColor.initials(for: "   "), "?")
        XCTAssertEqual(ParticipantColor.initials(for: "🙂"), "?")
    }
}
