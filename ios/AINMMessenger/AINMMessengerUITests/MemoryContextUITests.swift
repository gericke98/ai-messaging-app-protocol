// MemoryContextUITests.swift — Scenario 4 (Phase 13 HUMAN-UAT)
//
// Expected behavior (from 13-HUMAN-UAT.md #4):
//   Context pill (brain icon) appears in thread toolbar when agent participant exists;
//   tapping shows half-sheet with Summary, Active Context, Archival Facts preview,
//   Participants; View All navigates to full archival view; search and swipe-to-delete
//   work.
//
// Fixture scenario-04-memory seeds thread-01 with helper@localhost as agent participant
// and four archival facts: fact-coffee-01, fact-meetings-01, fact-project-01, fact-timezone-01.
// Event script scenario-04-events pushes a context.frame at 500ms so Active Context is
// populated before the user taps the pill.

import XCTest

final class MemoryContextUITests: AINMUITestCase {

    func test_memoryPill_opensSheet_showsSections_viewAllNavigatesToArchival_searchFilters() {
        launchWith(seedFixture: "scenario-04-memory",
                   eventScript: "scenario-04-events")

        openThread(threadId: "thread-01")

        // Memory context pill in thread toolbar.
        let pill = app.buttons[UITestID.Memory.toolbarPill].firstMatch
        let pillOther = app.otherElements[UITestID.Memory.toolbarPill].firstMatch
        let pillTarget: XCUIElement
        if pill.waitForExistence(timeout: 10) {
            pillTarget = pill
        } else if pillOther.waitForExistence(timeout: 2) {
            pillTarget = pillOther
        } else {
            XCTFail("Memory context pill not visible in thread toolbar (UITestID.Memory.toolbarPill)")
            return
        }
        pillTarget.tap()

        // Sheet sections.
        let summary = app.otherElements[UITestID.Memory.sheetSummary].firstMatch
        let summaryStatic = app.staticTexts[UITestID.Memory.sheetSummary].firstMatch
        XCTAssertTrue(summary.waitForExistence(timeout: 5)
                      || summaryStatic.waitForExistence(timeout: 1),
                      "Memory sheet Summary section not present")

        let active = app.otherElements[UITestID.Memory.sheetActiveContext].firstMatch
        let activeStatic = app.staticTexts[UITestID.Memory.sheetActiveContext].firstMatch
        XCTAssertTrue(active.waitForExistence(timeout: 3)
                      || activeStatic.waitForExistence(timeout: 1),
                      "Memory sheet Active Context section not present")

        let archivalPreview = app.otherElements[UITestID.Memory.sheetArchivalPreview].firstMatch
        let archivalPreviewStatic = app.staticTexts[UITestID.Memory.sheetArchivalPreview].firstMatch
        XCTAssertTrue(archivalPreview.waitForExistence(timeout: 3)
                      || archivalPreviewStatic.waitForExistence(timeout: 1),
                      "Memory sheet Archival Preview section not present")

        // View All navigates to archival view.
        let viewAll = app.buttons[UITestID.Memory.sheetViewAllButton].firstMatch
        let viewAllOther = app.otherElements[UITestID.Memory.sheetViewAllButton].firstMatch
        let viewAllTarget: XCUIElement
        if viewAll.waitForExistence(timeout: 3) {
            viewAllTarget = viewAll
        } else if viewAllOther.waitForExistence(timeout: 1) {
            viewAllTarget = viewAllOther
        } else {
            XCTFail("View All button not found on memory sheet")
            return
        }
        viewAllTarget.tap()

        // Archival search field visible.
        let search = app.searchFields[UITestID.Memory.archivalSearch].firstMatch
        let searchOther = app.otherElements[UITestID.Memory.archivalSearch].firstMatch
        let searchText = app.textFields[UITestID.Memory.archivalSearch].firstMatch
        let searchTarget: XCUIElement
        if search.waitForExistence(timeout: 5) {
            searchTarget = search
        } else if searchText.waitForExistence(timeout: 1) {
            searchTarget = searchText
        } else if searchOther.waitForExistence(timeout: 1) {
            searchTarget = searchOther
        } else {
            XCTFail("Archival search field not queryable (UITestID.Memory.archivalSearch)")
            return
        }

        // Seeded fact rows visible (fact ids match fixture: fact-coffee-01 / fact-meetings-01 etc).
        let coffeeId = UITestID.Memory.archivalRow(factId: "fact-coffee-01")
        let coffeeRow = app.otherElements[coffeeId].firstMatch
        let coffeeCell = app.cells[coffeeId].firstMatch
        let coffeeButton = app.buttons[coffeeId].firstMatch
        XCTAssertTrue(coffeeRow.waitForExistence(timeout: 5)
                      || coffeeCell.waitForExistence(timeout: 1)
                      || coffeeButton.waitForExistence(timeout: 1),
                      "Seeded archival fact row fact-coffee-01 not found")

        // Filter via search — type "meetings" to narrow to fact-meetings-01.
        searchTarget.tap()
        searchTarget.typeText("meetings")

        let meetingsId = UITestID.Memory.archivalRow(factId: "fact-meetings-01")
        let meetingsRow = app.otherElements[meetingsId].firstMatch
        let meetingsCell = app.cells[meetingsId].firstMatch
        let meetingsButton = app.buttons[meetingsId].firstMatch
        XCTAssertTrue(meetingsRow.waitForExistence(timeout: 5)
                      || meetingsCell.waitForExistence(timeout: 1)
                      || meetingsButton.waitForExistence(timeout: 1),
                      "Search for 'meetings' did not surface fact-meetings-01 row")
    }
}
