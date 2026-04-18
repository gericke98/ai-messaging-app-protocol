// ToolCallIndicatorUITests.swift — Scenario 1 (Phase 13 HUMAN-UAT)
//
// Expected behavior (from 13-HUMAN-UAT.md #1):
//   When agent executes tool calls, ToolCallIndicatorViews appear below message list,
//   collapsed by default; tap expands to show input JSON, progress, result, or error;
//   spinner for running, green checkmark for completed, red X for error, clock for timed out.
//
// This test asserts the running → completed transition (the positive lifecycle path),
// expands the indicator, and verifies the input JSON + result strings are present.
// Per spec/tool-visibility.md (D-12), only one lifecycle state is visible at a time —
// we also assert the running spinner is gone after the completed transition.
//
// Error/timedOut states are verified to exist in the UITestID namespace (compile-time),
// but a full lifecycle drill is deferred to dedicated future scripts.

import XCTest

final class ToolCallIndicatorUITests: AINMUITestCase {

    func test_toolCallLifecycle_runningTransitionsToCompleted_withExpandableDetail() {
        launchWith(seedFixture: "scenario-01-tool-call",
                   eventScript: "scenario-01-events")

        openThread(threadId: "thread-01")

        // First wait for the indicator ROW to appear — guarantees the tool call state
        // was handed off to the view. Only THEN check the specific lifecycle icon.
        let indicatorRow = app.buttons[UITestID.ToolCall.indicatorRow(callId: "call-01")].firstMatch
        let indicatorRowOther = app.otherElements[UITestID.ToolCall.indicatorRow(callId: "call-01")].firstMatch
        let rowAppeared = indicatorRow.waitForExistence(timeout: 10)
            || indicatorRowOther.waitForExistence(timeout: 2)
        XCTAssertTrue(rowAppeared,
                      "Tool call indicator row did not appear (UITestID.ToolCall.indicatorRow.call-01). " +
                      "Check scenario-01-events.json timing and ToolCallGroupView render path.")

        // t=100ms from bootstrap: tool.call.started → running spinner (ProgressView) appears.
        // ProgressView resolves as `otherElements` or `progressIndicators` in XCUITest.
        let runningOther = app.otherElements[UITestID.ToolCall.statusRunning].firstMatch
        let runningImage = app.images[UITestID.ToolCall.statusRunning].firstMatch
        let runningProgress = app.progressIndicators[UITestID.ToolCall.statusRunning].firstMatch
        let runningAppeared = runningOther.waitForExistence(timeout: 5)
            || runningImage.waitForExistence(timeout: 1)
            || runningProgress.waitForExistence(timeout: 1)
        XCTAssertTrue(runningAppeared,
                      "Running spinner did not appear after tool.call.started (UITestID.ToolCall.statusRunning)")
        // Use the element that found it for subsequent assertions.
        let running = runningOther.exists ? runningOther : (runningImage.exists ? runningImage : runningProgress)

        // t=20s: tool.call.completed → green checkmark replaces spinner.
        let completed = app.images[UITestID.ToolCall.statusCompleted].firstMatch
        let completedOther = app.otherElements[UITestID.ToolCall.statusCompleted].firstMatch
        let completedAppeared = completed.waitForExistence(timeout: 25)
            || completedOther.waitForExistence(timeout: 2)
        XCTAssertTrue(completedAppeared,
                      "Completed checkmark did not appear after tool.call.completed (20s+margin)")

        // After the transition, the running spinner MUST be gone — per spec/tool-visibility.md
        // only one lifecycle state is visible at a time (D-12 invariant).
        XCTAssertFalse(running.exists,
                       "Running spinner should be gone after tool.call.completed lifecycle event")

        // Expand the indicator → input JSON + result visible.
        let expandButton = app.buttons[UITestID.ToolCall.expandButton].firstMatch
        let expandImage = app.images[UITestID.ToolCall.expandButton].firstMatch
        let expandOther = app.otherElements[UITestID.ToolCall.expandButton].firstMatch
        if expandButton.waitForExistence(timeout: 2) {
            expandButton.tap()
        } else if expandImage.waitForExistence(timeout: 1) {
            expandImage.tap()
        } else if expandOther.waitForExistence(timeout: 1) {
            expandOther.tap()
        } else {
            XCTFail("Expand control for tool call indicator not queryable via buttons/images/otherElements")
            return
        }

        let inputJson = app.staticTexts[UITestID.ToolCall.detailInputJson].firstMatch
        let inputJsonOther = app.otherElements[UITestID.ToolCall.detailInputJson].firstMatch
        let inputJsonScroll = app.scrollViews[UITestID.ToolCall.detailInputJson].firstMatch
        XCTAssertTrue(inputJson.waitForExistence(timeout: 3)
                      || inputJsonOther.waitForExistence(timeout: 1)
                      || inputJsonScroll.waitForExistence(timeout: 1),
                      "Expanded detail did not show input JSON (UITestID.ToolCall.detailInputJson)")

        let result = app.staticTexts[UITestID.ToolCall.detailResult].firstMatch
        let resultOther = app.otherElements[UITestID.ToolCall.detailResult].firstMatch
        XCTAssertTrue(result.waitForExistence(timeout: 3)
                      || resultOther.waitForExistence(timeout: 1),
                      "Expanded detail did not show result summary (UITestID.ToolCall.detailResult)")
    }

    func test_identifiers_forAllFourStates_exist_in_UITestID_namespace() {
        // Compile-time guard: ensures UITestID exposes all 4 state ids. If a future fixture
        // scripts an error or timedOut event, the query-side ids are already in place.
        XCTAssertEqual(UITestID.ToolCall.statusRunning, "toolCall.indicator.status.running")
        XCTAssertEqual(UITestID.ToolCall.statusCompleted, "toolCall.indicator.status.completed")
        XCTAssertEqual(UITestID.ToolCall.statusError, "toolCall.indicator.status.error")
        XCTAssertEqual(UITestID.ToolCall.statusTimedOut, "toolCall.indicator.status.timedOut")
    }
}
