#if DEBUG
import XCTest
import GRDB
@testable import AINMMessenger

/// Verifies that the seeded multi-agent A2A burst in scenario-07-a2a-burst.json folds
/// into exactly one digest capsule when loaded through the real SeedLoader → GRDB →
/// MessageThreadViewModel → A2ACapsuleFolder pipeline.
///
/// Thread layout:
///   msg-01       human (uat-user@localhost) — opens thread → not A2A, leading boundary
///   msg-a2a-01   agent-alpha@localhost      — A2A (no mention)
///   msg-a2a-02   analyst@acme.test          — A2A (mention of agent-alpha → directedTo = alpha, != user)
///   msg-a2a-03   agent-alpha@localhost      — A2A (no mention)
///   msg-04       human (uat-user@localhost) — "launching now" → isFromCurrentUser = true
///                                             → not A2A → trailing boundary
///
/// Expected fold: msgs a2a-01..a2a-03 are a bounded 3-message / 2-agent run → 1 capsule.
/// msg-01 and msg-04 are .group items (human messages, not in the capsule).
@MainActor
final class A2AFixtureFoldTests: XCTestCase {

    func test_scenario07_foldsIntoOneCapsule_withInlineReportBack() async throws {
        let db = try AppDatabase.makeInMemory()
        // SeedLoader.loadFixture resolves the fixture from the app bundle (Bundle(for:) fallback)
        // so it works in the unit-test host without AppState.
        try SeedLoader.loadFixture(named: "scenario-07-a2a-burst", into: db)

        let vm = MessageThreadViewModel(threadId: "thread-01",
                                        currentUserIdentity: "uat-user@localhost",
                                        database: db)
        await vm.loadInitial()

        XCTAssertTrue(vm.isMultiParticipant,
                      "thread has 3 participants (human + 2 agents) → isMultiParticipant must be true")

        let capsules: [A2ACapsule] = vm.threadItems.compactMap {
            if case .capsule(let c) = $0 { return c } else { return nil }
        }
        XCTAssertEqual(capsules.count, 1,
                       "the bounded 3-message / 2-agent A2A burst should fold into exactly 1 capsule")
        XCTAssertEqual(capsules.first?.messageCount, 3,
                       "capsule must contain the 3 A2A messages")
        XCTAssertEqual(Set(capsules.first?.agents ?? []),
                       ["agent-alpha@localhost", "analyst@acme.test"],
                       "capsule must list both A2A senders")

        let groupItems = vm.threadItems.filter {
            if case .group = $0 { return true } else { return false }
        }
        // msg-01 (human open) + msg-04 (human close) = 2 inline .group items
        XCTAssertGreaterThanOrEqual(groupItems.count, 2,
                                    "user opener and human follow-up must render as inline groups")
    }
}
#endif
