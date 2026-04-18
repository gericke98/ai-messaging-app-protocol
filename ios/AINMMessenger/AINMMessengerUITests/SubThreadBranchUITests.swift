// SubThreadBranchUITests.swift — Scenario 5 (Phase 13 HUMAN-UAT)
//
// Expected behavior (from 13-HUMAN-UAT.md #5):
//   Long-pressing a message shows Branch Thread option; branching creates a new
//   sub-thread and navigates to it; sub-thread shows banner 'Thread from {sender}'
//   with back navigation; parent message shows reply count badge; Branch Thread
//   hidden inside sub-threads.
//
// Fixture scenario-05-sub-thread seeds thread-parent-01 with msg-parent-01 (from
// helper@localhost, text content). Long-press on msg-parent-01 triggers the
// context menu, the Branch Thread item creates a sub-thread and navigates.

import XCTest

final class SubThreadBranchUITests: AINMUITestCase {

    func test_longPressMessage_branchThread_navigatesToSubThread_bannerVisible_backNavigates() {
        launchWith(seedFixture: "scenario-05-sub-thread")
        openThread(threadId: "thread-parent-01")

        let parentRowId = UITestID.SubThread.messageRow(messageId: "msg-parent-01")
        let parentRow = app.otherElements[parentRowId].firstMatch
        let parentRowButton = app.buttons[parentRowId].firstMatch
        let parentRowText = app.staticTexts[parentRowId].firstMatch
        let parentTarget: XCUIElement
        if parentRow.waitForExistence(timeout: 10) {
            parentTarget = parentRow
        } else if parentRowButton.waitForExistence(timeout: 2) {
            parentTarget = parentRowButton
        } else if parentRowText.waitForExistence(timeout: 2) {
            parentTarget = parentRowText
        } else {
            XCTFail("Parent message row msg-parent-01 not found (UITestID.SubThread.messageRow)")
            return
        }

        // Long-press to open context menu containing Branch Thread.
        parentTarget.press(forDuration: 1.0)

        let branchMenuItem = app.buttons["Branch Thread"].firstMatch
        let branchMenuItemOther = app.otherElements["Branch Thread"].firstMatch
        let branchTarget: XCUIElement
        if branchMenuItem.waitForExistence(timeout: 5) {
            branchTarget = branchMenuItem
        } else if branchMenuItemOther.waitForExistence(timeout: 1) {
            branchTarget = branchMenuItemOther
        } else {
            XCTFail("Branch Thread context menu item not visible after long-press")
            return
        }
        branchTarget.tap()

        // Sub-thread banner visible.
        let banner = app.otherElements[UITestID.SubThread.banner].firstMatch
        let bannerStatic = app.staticTexts[UITestID.SubThread.banner].firstMatch
        XCTAssertTrue(banner.waitForExistence(timeout: 10)
                      || bannerStatic.waitForExistence(timeout: 1),
                      "Sub-thread banner did not appear after branching")

        let back = app.buttons[UITestID.SubThread.bannerBackButton].firstMatch
        let backOther = app.otherElements[UITestID.SubThread.bannerBackButton].firstMatch
        let backImage = app.images[UITestID.SubThread.bannerBackButton].firstMatch
        let backByLabel = app.buttons["Return to parent thread"].firstMatch
        let backTarget: XCUIElement
        if back.waitForExistence(timeout: 3) {
            backTarget = back
        } else if backOther.waitForExistence(timeout: 1) {
            backTarget = backOther
        } else if backImage.waitForExistence(timeout: 1) {
            backTarget = backImage
        } else if backByLabel.waitForExistence(timeout: 1) {
            backTarget = backByLabel
        } else {
            XCTFail("Sub-thread back button not visible (UITestID.SubThread.bannerBackButton)")
            return
        }
        backTarget.tap()

        // After back-navigation, parent thread composer should be visible again.
        let composer = app.textFields[UITestID.Mention.composerField].firstMatch
        let composerTextView = app.textViews[UITestID.Mention.composerField].firstMatch
        let composerOther = app.otherElements[UITestID.Mention.composerField].firstMatch
        XCTAssertTrue(composer.waitForExistence(timeout: 10)
                      || composerTextView.waitForExistence(timeout: 2)
                      || composerOther.waitForExistence(timeout: 2),
                      "After back-navigation, expected to land in parent thread with composer visible")

        // Parent message now shows a reply count badge (the newly-created sub-thread).
        let badgeId = UITestID.SubThread.replyBadge(messageId: "msg-parent-01")
        let badge = app.buttons[badgeId].firstMatch
        let badgeOther = app.otherElements[badgeId].firstMatch
        let badgeStatic = app.staticTexts[badgeId].firstMatch
        XCTAssertTrue(badge.waitForExistence(timeout: 5)
                      || badgeOther.waitForExistence(timeout: 1)
                      || badgeStatic.waitForExistence(timeout: 1),
                      "Reply count badge not visible on parent message after branching (UITestID.SubThread.replyBadge)")
    }
}
