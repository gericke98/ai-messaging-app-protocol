// UITestIdentifiers.swift — Phase 14 UAT automation harness
//
// Single source of truth for accessibility identifiers used by AINMMessengerUITests.
// Naming convention (per 14-CONTEXT D-10): screen.component.action (dot-path).
// Stateful indicators get a distinct identifier per state, not a single ID with
// a changing accessibilityValue (14-CONTEXT D-12). Tests assert existence.
//
// File membership: BOTH the AINMMessenger app target (via the synchronized root
// group) AND the AINMMessengerUITests target (via membershipExceptions).
//
// Swift 6 strict concurrency: String constants and static funcs are trivially
// Sendable; no annotations required.

import Foundation

enum UITestID {

    // MARK: - App-level readiness marker
    enum App {
        /// Rendered when SeedLoader + AppState.setup() + (optional) event injector
        /// have completed. XCUITest waits for this before issuing assertions.
        static let ready = "app.ready"
    }

    // MARK: - Scenario 1: Tool call visibility
    enum ToolCall {
        static func indicatorRow(callId: String) -> String { "toolCall.indicator.row.\(callId)" }
        static let statusRunning   = "toolCall.indicator.status.running"
        static let statusCompleted = "toolCall.indicator.status.completed"
        static let statusError     = "toolCall.indicator.status.error"
        static let statusTimedOut  = "toolCall.indicator.status.timedOut"
        static let expandButton    = "toolCall.indicator.expandButton"
        static let detailInputJson = "toolCall.detail.inputJson"
        static let detailResult    = "toolCall.detail.result"
    }

    // MARK: - Scenario 2: @-mentions
    enum Mention {
        static let composerField    = "composer.textField"
        static let sendButton       = "composer.sendButton"
        static let autocompleteList = "composer.mention.autocomplete.list"
        static func autocompleteRow(identity: String) -> String { "composer.mention.autocomplete.row.\(identity)" }
        static let autocompleteEmpty = "composer.mention.autocomplete.empty"
    }

    // MARK: - Scenario 3: Agent discovery
    enum AgentDiscovery {
        static let search                  = "agentDiscovery.search"
        static func listCell(agentId: String) -> String { "agentDiscovery.list.cell.\(agentId)" }
        static let detail                  = "agentDiscovery.detail"
        static let detailSkillsHeader      = "agentDiscovery.detail.skillsHeader"
        static let detailBadgeStreaming    = "agentDiscovery.detail.badge.streaming"
        static let detailStartConversation = "agentDiscovery.detail.startConversation"
        static let addByAddressField       = "agentDiscovery.addByAddress.textField"
        static let addByAddressButton      = "agentDiscovery.addByAddress.button"
    }

    // MARK: - Scenario 4: Memory context
    enum Memory {
        static let toolbarPill       = "thread.toolbar.memoryPill"
        static let sheetSummary      = "memory.sheet.summary"
        static let sheetActiveContext = "memory.sheet.activeContext"
        static let sheetArchivalPreview = "memory.sheet.archivalPreview"
        static let sheetViewAllButton = "memory.sheet.viewAllButton"
        static let archivalSearch    = "memory.archival.search"
        static func archivalRow(factId: String) -> String { "memory.archival.row.\(factId)" }
        static let archivalDeleteConfirm = "memory.archival.deleteConfirm"
    }

    // MARK: - Scenario 5: Sub-thread branch
    enum SubThread {
        static func messageRow(messageId: String) -> String { "thread.message.row.\(messageId)" }
        static func replyBadge(messageId: String) -> String { "thread.message.row.\(messageId).replyBadge" }
        static let banner           = "subThread.banner"
        static let bannerBackButton = "subThread.banner.backButton"
        static let bannerViewParent = "subThread.banner.viewParent"
    }

    // MARK: - Scenario 6: Deep link
    enum DeepLink {
        static func threadView(threadId: String) -> String { "thread.view.\(threadId)" }
        static let toastMessageNotFound = "thread.toast.messageNotFound"
        static let threadNotFoundAlert = "thread.alert.threadNotFound"
    }

    // MARK: - Shared (conversation list, used for navigation into seeded threads)
    enum ConversationList {
        static func cell(threadId: String) -> String { "conversationList.cell.\(threadId)" }
    }
}
