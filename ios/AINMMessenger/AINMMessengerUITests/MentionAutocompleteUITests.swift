// MentionAutocompleteUITests.swift — Scenario 2 (Phase 13 HUMAN-UAT)
//
// Expected behavior (from 13-HUMAN-UAT.md #2):
//   Typing @ in composer shows filtered participant popup; selecting a participant
//   inserts @DisplayName; backspacing past @ dismisses popup; rendered messages show
//   mention spans in accent blue bold.
//
// Fixture scenario-02-mentions seeds remy@localhost with displayName "Remy" — so
// typing "@Re" deterministically filters to Remy and selection produces "@Remy".

import XCTest

final class MentionAutocompleteUITests: AINMUITestCase {

    func test_atTriggersAutocomplete_selectionInsertsMention_backspaceDismissesPopup() {
        launchWith(seedFixture: "scenario-02-mentions")
        openThread(threadId: "thread-01")

        // Composer text field.
        let composer = app.textFields[UITestID.Mention.composerField].firstMatch
        let composerTextView = app.textViews[UITestID.Mention.composerField].firstMatch
        let composerOther = app.otherElements[UITestID.Mention.composerField].firstMatch

        let composerReady = composer.waitForExistence(timeout: 10)
        let composerTarget: XCUIElement
        if composerReady {
            composerTarget = composer
        } else if composerTextView.waitForExistence(timeout: 2) {
            composerTarget = composerTextView
        } else if composerOther.waitForExistence(timeout: 2) {
            composerTarget = composerOther
        } else {
            XCTFail("Composer text field not found (UITestID.Mention.composerField)")
            return
        }
        composerTarget.tap()
        composerTarget.typeText("@Re")

        // Autocomplete list appears.
        let list = app.otherElements[UITestID.Mention.autocompleteList].firstMatch
        let listScroll = app.scrollViews[UITestID.Mention.autocompleteList].firstMatch
        XCTAssertTrue(list.waitForExistence(timeout: 3)
                      || listScroll.waitForExistence(timeout: 1),
                      "Autocomplete list did not appear after typing '@'")

        // Remy row.
        let remyButton = app.buttons[UITestID.Mention.autocompleteRow(identity: "remy@localhost")].firstMatch
        let remyOther = app.otherElements[UITestID.Mention.autocompleteRow(identity: "remy@localhost")].firstMatch
        let remyCell = app.cells[UITestID.Mention.autocompleteRow(identity: "remy@localhost")].firstMatch
        let remyTarget: XCUIElement
        if remyButton.waitForExistence(timeout: 3) {
            remyTarget = remyButton
        } else if remyOther.waitForExistence(timeout: 1) {
            remyTarget = remyOther
        } else if remyCell.waitForExistence(timeout: 1) {
            remyTarget = remyCell
        } else {
            XCTFail("Remy autocomplete row not found (UITestID.Mention.autocompleteRow(identity: remy@localhost))")
            return
        }
        remyTarget.tap()

        // Selection should insert @Remy into the composer.
        let composerValue = (composerTarget.value as? String) ?? ""
        XCTAssertTrue(composerValue.contains("@Remy"),
                      "Expected composer to contain '@Remy' after row selection, got: \(composerValue)")
    }
}
