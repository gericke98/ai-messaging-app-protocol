// AINMUITestCase.swift — Phase 14 UAT automation harness
//
// Base class for all AINM UI tests. Handles:
//   - XCUIApplication launch with `-UITestSeed <fixture>` and optional
//     `-UITestEvents <script>` arguments consumed by AINMMessengerApp's
//     DEBUG-only bootstrap path (see 14-01/14-03/14-04 wiring).
//   - Optional `-UITestDeepLink <url>` launch argument used by scenario 6
//     (DeepLinkUITests) — replayed by the app via DeepLinkHandler.parse,
//     exercising the exact same code path as onOpenURL (per 14-06 executor
//     guidance — launch-arg delivery is more reliable than simctl openurl
//     on iOS 18+/26 simulators).
//   - Blocks until `UITestID.App.ready` appears (rendered by AINMMessengerApp
//     once SeedLoader + AppState.setup + optional event injector complete).
//   - Terminates the app on tearDown.

import XCTest

class AINMUITestCase: XCTestCase {

    var app: XCUIApplication!

    override func setUpWithError() throws {
        try super.setUpWithError()
        continueAfterFailure = false
        app = XCUIApplication()
    }

    override func tearDownWithError() throws {
        app?.terminate()
        try super.tearDownWithError()
    }

    /// Launch the app with `-UITestSeed <fixture>` and optional `-UITestEvents <script>`.
    /// Blocks until the `app.ready` marker appears (bootstrap complete).
    /// Fails the test with a clear message if bootstrap doesn't complete within `timeout` seconds.
    func launchWith(seedFixture: String,
                    eventScript: String? = nil,
                    deepLink: String? = nil,
                    timeout: TimeInterval = 30,
                    file: StaticString = #file,
                    line: UInt = #line) {
        var args = ["-UITestSeed", seedFixture]
        if let eventScript {
            args += ["-UITestEvents", eventScript]
        }
        if let deepLink {
            args += ["-UITestDeepLink", deepLink]
        }
        app.launchArguments = args
        app.launch()

        let ready = app.otherElements[UITestID.App.ready]
        XCTAssertTrue(ready.waitForExistence(timeout: timeout),
                      "App never signalled ready (UITestID.App.ready) after seed load within \(timeout)s. " +
                      "Check tmp/uat-server.log for server issues or the test runner console for bootstrap errors.",
                      file: file, line: line)
    }

    /// Convenience: navigate into a seeded thread via its conversation list cell.
    /// Precondition: launchWith has already been called and returned.
    @discardableResult
    func openThread(threadId: String,
                    timeout: TimeInterval = 10,
                    file: StaticString = #file,
                    line: UInt = #line) -> Bool {
        let cellId = UITestID.ConversationList.cell(threadId: threadId)
        // Conversation list cells can resolve as buttons or other elements depending
        // on SwiftUI version. Try buttons first (the primary attachment site), fall
        // back to otherElements if needed.
        let button = app.buttons[cellId]
        if button.waitForExistence(timeout: timeout) {
            button.tap()
            return true
        }
        let other = app.otherElements[cellId]
        if other.waitForExistence(timeout: 2) {
            other.tap()
            return true
        }
        XCTFail("Conversation list cell for threadId=\(threadId) not found within \(timeout)s",
                file: file, line: line)
        return false
    }
}
