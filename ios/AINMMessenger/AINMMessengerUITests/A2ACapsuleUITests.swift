// A2ACapsuleUITests.swift — LV-3 live-verify: A2A digest capsule fold/expand/collapse
//
// Scenario: scenario-07-a2a-burst
//   msg-01 (human uat-user@localhost) "Are we ready to launch?"
//   msg-a2a-01 (agent agent-alpha@localhost)  ─┐
//   msg-a2a-02 (agent analyst@acme.test)       ├─ folds into one capsule (id == "msg-a2a-01")
//   msg-a2a-03 (agent agent-alpha@localhost)  ─┘
//   msg-04 (human uat-user@localhost) "Great — launching now."
//
// Expected rendered thread: human msg-01 → [A2A capsule] → human msg-04.
// Capsule folds the 3-message / 2-agent burst; both human messages render outside.
//
// Assertions:
//   1. Capsule (id == "msg-a2a-01") is present in the collapsed state.
//   2. Closing human message msg-04 is visible outside the capsule.
//   3. Inner A2A message msg-a2a-02 is hidden while the capsule is collapsed.
//   4. Tapping the capsule expands it; msg-a2a-02 becomes visible.
//   5. Tapping the Collapse button inside the expanded capsule collapses it;
//      msg-a2a-02 disappears from the accessibility tree.
//
// Screenshots captured at collapsed, expanded, and re-collapsed states.
//
// Matcher strategy:
//   Collapsed capsule: .accessibilityElement(children: .ignore) → surfaces as otherElements.
//   Expanded capsule: .accessibilityElement(children: .contain) — children are traversable.
//   The expand/collapse toggle tap targets the capsule when collapsed (whole card is tappable)
//   and the "Collapse" Button rendered inside the expanded body when already open. Using the
//   "Collapse" button label is the reliable target since when expanded the outer capsule element
//   becomes a container (not independently tappable via accessibility identifier).

import XCTest

/// Live render/behavior verification of the A2A arc against the seeded multi-agent thread
/// (scenario-07-a2a-burst): the bounded 3-message / 2-agent burst folds into one capsule,
/// which expands in place to reveal the inner agent messages and collapses again. Both human
/// messages (msg-01, msg-04) render outside the capsule.
final class A2ACapsuleUITests: AINMUITestCase {

    // MARK: - Helpers

    private func captureScreenshot(_ name: String) {
        let shot = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: shot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    // MARK: - Tests

    func test_a2aBurst_collapsesToCapsule_expandsAndCollapses() {
        launchWith(seedFixture: "scenario-07-a2a-burst")
        _ = openThread(threadId: "thread-01")

        // ── Step 1: Collapsed capsule present ────────────────────────────────
        // Collapsed: .accessibilityElement(children: .ignore) → appears as otherElements.
        let capsuleId = UITestID.A2ACapsule.capsule(capsuleId: "msg-a2a-01")
        let capsuleOther   = app.otherElements[capsuleId].firstMatch
        let capsuleButton  = app.buttons[capsuleId].firstMatch
        let capsuleAny     = app.descendants(matching: .any)[capsuleId].firstMatch

        let capsule: XCUIElement
        if capsuleOther.waitForExistence(timeout: 15) {
            capsule = capsuleOther
        } else if capsuleButton.waitForExistence(timeout: 3) {
            capsule = capsuleButton
        } else if capsuleAny.waitForExistence(timeout: 3) {
            capsule = capsuleAny
        } else {
            XCTFail("A2A burst did not fold into a capsule (\(capsuleId)). " +
                    "Accessibility tree:\n\(app.debugDescription)")
            return
        }
        captureScreenshot("a2a-01-capsule-collapsed")

        // ── Step 2: Closing human message (msg-04) is OUTSIDE the capsule ────
        // msg-04 is the last message in the thread; the ScrollView uses defaultScrollAnchor(.bottom)
        // so it should already be at the bottom of the screen. If it's off-screen due to the capsule
        // taking up space, scroll down to bring it into the lazy-render window.
        let closingId = UITestID.SubThread.messageRow(messageId: "msg-04")
        let closingOther = app.otherElements[closingId].firstMatch
        let closingAny   = app.descendants(matching: .any)[closingId].firstMatch
        if !closingOther.waitForExistence(timeout: 5) && !closingAny.waitForExistence(timeout: 1) {
            // Scroll to bring the bottom of the thread into view.
            app.swipeUp(velocity: .slow)
        }
        let closingVisible = closingOther.waitForExistence(timeout: 5)
                             || closingAny.waitForExistence(timeout: 2)
        XCTAssertTrue(closingVisible,
                      "Closing human message msg-04 should render outside the capsule (\(closingId))")

        // ── Step 3: Inner A2A message hidden while collapsed ──────────────────
        // XCTest queries the semantic accessibility tree; with children: .ignore the inner
        // rows are excluded. Check immediately (no wait) — if they slip through, the capsule
        // fold is broken.
        let innerCollapsedId = UITestID.SubThread.messageRow(messageId: "msg-a2a-02")
        let innerCollapsed = app.descendants(matching: .any)[innerCollapsedId].firstMatch
        XCTAssertFalse(innerCollapsed.exists,
                       "Inner A2A message msg-a2a-02 should be hidden while capsule is collapsed")

        // ── Step 4: Expand ────────────────────────────────────────────────────
        // Tap the collapsed capsule to expand it.
        capsule.tap()

        // After expansion the inner MessageGroupView rows become accessible (children: .contain).
        let innerExpandedId = UITestID.SubThread.messageRow(messageId: "msg-a2a-02")
        let innerExpanded = app.descendants(matching: .any)[innerExpandedId].firstMatch
        XCTAssertTrue(innerExpanded.waitForExistence(timeout: 8),
                      "Inner A2A message msg-a2a-02 not visible after expanding the capsule")
        captureScreenshot("a2a-02-capsule-expanded")

        // ── Step 5: Collapse again ────────────────────────────────────────────
        // When expanded (children: .contain), the outer capsule element becomes a container
        // and is not independently tappable via its accessibility identifier. Use the
        // "Collapse" button that A2ACapsuleView renders inside the expanded body instead.
        let collapseBtn = app.buttons["Collapse"].firstMatch
        if collapseBtn.waitForExistence(timeout: 5) {
            collapseBtn.tap()
        } else {
            // Fallback: the header Button inside the capsule body (chevron area). When expanded,
            // the header is a plain button within the contain-tree — try to find it by the capsule
            // id (may resolve to the header button when the container exposes children).
            let headerOther = app.otherElements[capsuleId].firstMatch
            let headerButton = app.buttons[capsuleId].firstMatch
            if headerButton.waitForExistence(timeout: 3) {
                headerButton.tap()
            } else if headerOther.exists {
                headerOther.tap()
            } else {
                XCTFail("Could not find a collapse control (neither 'Collapse' button nor capsule header)")
                return
            }
        }

        // After collapse the inner rows should disappear from the semantic accessibility tree.
        let innerAfterCollapse = app.descendants(matching: .any)[innerExpandedId].firstMatch
        let goneExpectation = expectation(
            for: NSPredicate(format: "exists == false"),
            evaluatedWith: innerAfterCollapse)
        wait(for: [goneExpectation], timeout: 10)
        captureScreenshot("a2a-03-capsule-recollapsed")
    }
}
