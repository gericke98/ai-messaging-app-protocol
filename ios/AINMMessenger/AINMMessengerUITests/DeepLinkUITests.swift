// DeepLinkUITests.swift — Scenario 6 (Phase 13 HUMAN-UAT)
//
// Expected behavior (from 13-HUMAN-UAT.md #6):
//   Opening ainm://thread/{id} navigates to the thread; ainm://thread/{id}/message/{mid}
//   also scrolls to the message; nonexistent thread shows 'Thread Not Found' alert;
//   nonexistent message shows 'Message not found' toast.
//
// Delivery strategy (RESEARCH.md Pitfall 2+3 + plan 14-06 executor guidance):
//   - iOS XCUITest runner cannot invoke Foundation.Process() to call simctl.
//   - XCUIApplication.open(_:) is unreliable on iOS 18+/26 simulators.
//   - Canonical path: use -UITestDeepLink launch argument. AINMMessengerApp.task
//     parses the arg AFTER bootstrap and replays the URL through the same
//     handleDeepLink(_:) function that .onOpenURL invokes. Same code path —
//     DeepLinkHandler.parse is the single source of truth.
//   - Belt-and-suspenders: Makefile also issues `xcrun simctl openurl` pre-test
//     for the legitimate-thread case (thread-deeplink-01).
//
// Fixture scenario-06-deep-link seeds thread-deeplink-01 with msg-deeplink-01.

import XCTest

final class DeepLinkUITests: AINMUITestCase {

    /// Path 1: ainm://thread/{id} navigates to the correct thread.
    func test_deepLink_threadId_navigatesToThreadView() {
        launchWith(seedFixture: "scenario-06-deep-link",
                   deepLink: "ainm://thread/thread-deeplink-01")

        let threadViewId = UITestID.DeepLink.threadView(threadId: "thread-deeplink-01")
        let threadView = app.otherElements[threadViewId].firstMatch
        let threadViewScroll = app.scrollViews[threadViewId].firstMatch
        XCTAssertTrue(
            threadView.waitForExistence(timeout: 15)
            || threadViewScroll.waitForExistence(timeout: 2),
            "Deep link did not navigate to thread-deeplink-01 (expected UITestID.DeepLink.threadView id). " +
            "Check that AINMMessengerApp's -UITestDeepLink launch arg handling wired handleDeepLink correctly " +
            "and that scenario-06 fixture seeds thread 'thread-deeplink-01' exactly."
        )
    }

    /// Path 2: ainm://thread/{id}/message/{mid} also scrolls to the message.
    func test_deepLink_messageId_scrollsToMessage() {
        launchWith(seedFixture: "scenario-06-deep-link",
                   deepLink: "ainm://thread/thread-deeplink-01/message/msg-deeplink-01")

        let threadViewId = UITestID.DeepLink.threadView(threadId: "thread-deeplink-01")
        let threadView = app.otherElements[threadViewId].firstMatch
        let threadViewScroll = app.scrollViews[threadViewId].firstMatch
        XCTAssertTrue(threadView.waitForExistence(timeout: 15)
                      || threadViewScroll.waitForExistence(timeout: 2),
                      "Deep link with message id did not navigate to thread view")

        let messageRowId = UITestID.SubThread.messageRow(messageId: "msg-deeplink-01")
        let messageRow = app.otherElements[messageRowId].firstMatch
        let messageRowButton = app.buttons[messageRowId].firstMatch
        let messageRowStatic = app.staticTexts[messageRowId].firstMatch
        XCTAssertTrue(messageRow.waitForExistence(timeout: 5)
                      || messageRowButton.waitForExistence(timeout: 1)
                      || messageRowStatic.waitForExistence(timeout: 1),
                      "Deep link did not reach msg-deeplink-01 (UITestID.SubThread.messageRow)")
    }

    /// Path 3: nonexistent thread → "Thread Not Found" alert.
    func test_deepLink_missingThread_showsNotFoundAlert() {
        launchWith(seedFixture: "scenario-06-deep-link",
                   deepLink: "ainm://thread/does-not-exist")

        // SwiftUI alerts are reliably queryable by their title string; also try the
        // identifier we attached in 14-05 on the OK button for belt-and-suspenders.
        let alertByTitle = app.alerts["Thread Not Found"]
        let okButtonById = app.buttons[UITestID.DeepLink.threadNotFoundAlert]
        let alertAppeared = alertByTitle.waitForExistence(timeout: 15)
            || okButtonById.waitForExistence(timeout: 2)
        XCTAssertTrue(alertAppeared,
                      "Thread Not Found alert did not appear for a nonexistent thread id")

        // Dismiss to leave the app in a clean state for subsequent tests.
        if alertByTitle.exists {
            alertByTitle.buttons["OK"].tap()
        } else if okButtonById.exists {
            okButtonById.tap()
        }
    }

    /// Path 4: nonexistent message → "Message not found" toast.
    /// Thread exists (thread-deeplink-01), but the message id does not.
    func test_deepLink_missingMessage_showsNotFoundToast() {
        launchWith(seedFixture: "scenario-06-deep-link",
                   deepLink: "ainm://thread/thread-deeplink-01/message/does-not-exist")

        // Confirm we landed in the correct thread first (thread exists).
        let threadViewId = UITestID.DeepLink.threadView(threadId: "thread-deeplink-01")
        let threadView = app.otherElements[threadViewId].firstMatch
        let threadViewScroll = app.scrollViews[threadViewId].firstMatch
        XCTAssertTrue(threadView.waitForExistence(timeout: 15)
                      || threadViewScroll.waitForExistence(timeout: 2),
                      "Deep link with bad message id did not reach the thread view")

        // Then the transient toast appears (MessageThreadView.handleScrollToMessage
        // sets showMessageNotFound = true for 3s). The toast Text has the identifier.
        // Poll broadly — iOS accessibility propagation on overlay transitions can lag.
        let toastId = UITestID.DeepLink.toastMessageNotFound
        let toastStatic = app.staticTexts[toastId].firstMatch
        let toastOther = app.otherElements[toastId].firstMatch
        let toastByLabel = app.staticTexts["Message not found"].firstMatch
        let toastByPredicate = app.staticTexts.matching(NSPredicate(format: "label CONTAINS[c] 'message not found'")).firstMatch
        let found = toastStatic.waitForExistence(timeout: 15)
            || toastOther.waitForExistence(timeout: 2)
            || toastByLabel.waitForExistence(timeout: 2)
            || toastByPredicate.waitForExistence(timeout: 2)
        XCTAssertTrue(found,
                      "Message not found toast did not appear for a nonexistent message id")
    }
}
