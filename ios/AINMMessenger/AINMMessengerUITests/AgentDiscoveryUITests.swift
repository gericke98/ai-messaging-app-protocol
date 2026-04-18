// AgentDiscoveryUITests.swift — Scenario 3 (Phase 13 HUMAN-UAT)
//
// Expected behavior (from 13-HUMAN-UAT.md #3):
//   Discover Agents view shows cached agent cards with name, description, skill tags;
//   search filters list; tapping agent shows full profile with Skills, Tools, capability
//   badges; Start Conversation creates thread.
//
// Fixture scenario-03-agent-discovery seeds two agent cards: helper@localhost (with
// streaming + memory + sub-threads capabilities) and assistant@localhost (streaming-only).
// The test taps the Helper card, asserts Skills header + Streaming capability badge,
// taps Start Conversation, and asserts navigation into a thread (composer visible).

import XCTest

final class AgentDiscoveryUITests: AINMUITestCase {

    func test_agentDiscovery_listTapDetailStartConversation() {
        launchWith(seedFixture: "scenario-03-agent-discovery")

        // Entry point: the conversation list toolbar exposes a "Discover Agents" button.
        // We try a few possible query paths because the toolbar button may be backed by
        // either an accessibility label, an image+label, or an accessibility identifier.
        let discoverByLabel = app.buttons["Discover Agents"].firstMatch
        let discoverByImage = app.buttons["person.crop.circle.badge.plus"].firstMatch
        let discoverAny = app.buttons.matching(NSPredicate(format: "label CONTAINS[c] 'discover' OR identifier CONTAINS[c] 'discover'")).firstMatch

        let discoverTarget: XCUIElement
        if discoverByLabel.waitForExistence(timeout: 10) {
            discoverTarget = discoverByLabel
        } else if discoverByImage.waitForExistence(timeout: 2) {
            discoverTarget = discoverByImage
        } else if discoverAny.waitForExistence(timeout: 2) {
            discoverTarget = discoverAny
        } else {
            XCTFail("No Discover Agents entry point located in conversation list toolbar")
            return
        }
        discoverTarget.tap()

        // Helper card list cell (NavigationLink from 14-05).
        let helperId = UITestID.AgentDiscovery.listCell(agentId: "helper@localhost")
        let helperButton = app.buttons[helperId].firstMatch
        let helperOther = app.otherElements[helperId].firstMatch
        let helperCell = app.cells[helperId].firstMatch
        let helperTarget: XCUIElement
        if helperButton.waitForExistence(timeout: 5) {
            helperTarget = helperButton
        } else if helperCell.waitForExistence(timeout: 2) {
            helperTarget = helperCell
        } else if helperOther.waitForExistence(timeout: 2) {
            helperTarget = helperOther
        } else {
            XCTFail("Helper agent card row not found under \(helperId)")
            return
        }
        helperTarget.tap()

        // Skills header (static text).
        let skillsHeader = app.staticTexts[UITestID.AgentDiscovery.detailSkillsHeader].firstMatch
        let skillsHeaderOther = app.otherElements[UITestID.AgentDiscovery.detailSkillsHeader].firstMatch
        XCTAssertTrue(skillsHeader.waitForExistence(timeout: 5)
                      || skillsHeaderOther.waitForExistence(timeout: 1),
                      "Skills header not found in agent detail view")

        // Streaming capability badge. Helper has capability "streaming" per fixture.
        let streamingBadge = app.staticTexts[UITestID.AgentDiscovery.detailBadgeStreaming].firstMatch
        let streamingBadgeOther = app.otherElements[UITestID.AgentDiscovery.detailBadgeStreaming].firstMatch
        let streamingBadgeButton = app.buttons[UITestID.AgentDiscovery.detailBadgeStreaming].firstMatch
        XCTAssertTrue(streamingBadge.waitForExistence(timeout: 3)
                      || streamingBadgeOther.waitForExistence(timeout: 1)
                      || streamingBadgeButton.waitForExistence(timeout: 1),
                      "Streaming capability badge not found in agent detail view")

        // Start Conversation button.
        let startButton = app.buttons[UITestID.AgentDiscovery.detailStartConversation].firstMatch
        let startOther = app.otherElements[UITestID.AgentDiscovery.detailStartConversation].firstMatch
        let startTarget: XCUIElement
        if startButton.waitForExistence(timeout: 3) {
            startTarget = startButton
        } else if startOther.waitForExistence(timeout: 1) {
            startTarget = startOther
        } else {
            XCTFail("Start Conversation button not found in agent detail view")
            return
        }
        startTarget.tap()

        // After Start Conversation, the newly-created thread's composer should be visible.
        let composer = app.textFields[UITestID.Mention.composerField].firstMatch
        let composerTextView = app.textViews[UITestID.Mention.composerField].firstMatch
        let composerOther = app.otherElements[UITestID.Mention.composerField].firstMatch
        XCTAssertTrue(composer.waitForExistence(timeout: 10)
                      || composerTextView.waitForExistence(timeout: 2)
                      || composerOther.waitForExistence(timeout: 2),
                      "After Start Conversation, expected to land in a thread with the composer visible")
    }
}
