import XCTest
@testable import AINMMessenger

final class HapticsTests: XCTestCase {
    @MainActor
    func testHapticCallsDoNotCrash() {
        Haptics.send()
        Haptics.receive()
        Haptics.select()
        Haptics.success()
    }
}
