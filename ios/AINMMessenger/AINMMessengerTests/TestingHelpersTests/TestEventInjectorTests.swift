#if DEBUG
import XCTest
@testable import AINMMessenger

/// Unit tests for TestEventInjector — the runtime event harness that feeds synthetic
/// tool-call and context-frame events into AppState via the same callback contract
/// AINMClient uses for real server pushes.
///
/// See `.planning/phases/14-uat-automation-harness/14-04-PLAN.md` Task 1 for the
/// behavior specification these tests enforce.
final class TestEventInjectorTests: XCTestCase {

    // MARK: - Schema decoding

    /// Well-formed event script JSON must decode cleanly into EventScript with the
    /// expected number of entries and preserved per-event fields.
    func test_eventScript_decodesWellFormedJson() throws {
        let json = #"""
        {
          "version": "1.0",
          "scenario": "scenario-01-tool-call",
          "events": [
            { "atMs": 0, "type": "tool.call.started", "threadId": "thread-01", "callId": "call-01", "toolName": "weather_lookup" },
            { "atMs": 2000, "type": "tool.call.completed", "threadId": "thread-01", "callId": "call-01", "status": "success", "resultSummary": "SF: 18C", "durationMs": 2000 }
          ]
        }
        """#
        let script = try JSONDecoder().decode(EventScript.self, from: Data(json.utf8))
        XCTAssertEqual(script.version, "1.0")
        XCTAssertEqual(script.scenario, "scenario-01-tool-call")
        XCTAssertEqual(script.events.count, 2)
        XCTAssertEqual(script.events[0].type, "tool.call.started")
        XCTAssertEqual(script.events[0].callId, "call-01")
        XCTAssertEqual(script.events[0].toolName, "weather_lookup")
        XCTAssertEqual(script.events[1].type, "tool.call.completed")
        XCTAssertEqual(script.events[1].status, "success")
        XCTAssertEqual(script.events[1].durationMs, 2000)
    }

    /// Event scripts must tolerate unknown event types at decode time — the injector
    /// logs and skips at dispatch time rather than failing the entire script.
    /// This guards against forward-compat additions breaking existing test runs.
    func test_eventScript_decodesUnknownEventTypesWithoutThrowing() throws {
        let json = #"""
        {"version":"1.0","scenario":"test","events":[{"atMs":0,"type":"unknown.event","threadId":"t1"}]}
        """#
        XCTAssertNoThrow(try JSONDecoder().decode(EventScript.self, from: Data(json.utf8)))
    }

    // MARK: - Real dispatch against AppState

    /// End-to-end integration test: schedule a 2-event tool-call script against a
    /// real AppState instance, await completion, assert state actually mutated.
    /// Uses the same `appState.handleToolEvent(...)` call path `AINMClient` wires
    /// up in `AppState.setup()` lines 124-128 — guarantees the injector exercises
    /// the identical UI rendering graph as production server events.
    @MainActor
    func test_injector_dispatchesToolEvent_updatesAppState() async throws {
        let appState = AppState()

        let json = #"""
        {
          "version": "1.0",
          "scenario": "scenario-01-tool-call",
          "events": [
            { "atMs": 0, "type": "tool.call.started", "threadId": "thread-x", "callId": "call-x", "toolName": "weather_lookup" },
            { "atMs": 100, "type": "tool.call.completed", "threadId": "thread-x", "callId": "call-x", "status": "success", "resultSummary": "OK", "durationMs": 100 }
          ]
        }
        """#
        let script = try JSONDecoder().decode(EventScript.self, from: Data(json.utf8))

        TestEventInjector.schedule(script: script, for: appState)

        // Wait 200ms — covers the 100ms completed-event offset plus a generous jitter budget.
        try await Task.sleep(for: .milliseconds(200))

        let states = appState.toolCallStates["thread-x"] ?? []
        XCTAssertTrue(
            states.contains(where: { $0.id == "call-x" && $0.status == .completed }),
            "Expected call-x to reach .completed status after 100ms dispatch; got \(states.map { "\($0.id) \($0.status)" })"
        )
    }

    /// Context-frame dispatch integration test: a single `context.frame` script entry
    /// must assign into `appState.contextFrames[threadId]` — the same mutation
    /// `AINMClient.onContextFrame` performs via `AppState.setup()` lines 131-135.
    @MainActor
    func test_injector_dispatchesContextFrame_updatesAppState() async throws {
        let appState = AppState()

        let json = #"""
        {
          "version": "1.0",
          "scenario": "scenario-04-memory",
          "events": [
            {
              "atMs": 0,
              "type": "context.frame",
              "threadId": "thread-y",
              "frame": {
                "summary": "test summary",
                "activeMessages": ["msg-1"],
                "archivalFactIds": ["fact-a"],
                "participantIdentities": ["user@localhost"]
              }
            }
          ]
        }
        """#
        let script = try JSONDecoder().decode(EventScript.self, from: Data(json.utf8))

        TestEventInjector.schedule(script: script, for: appState)

        // Wait 100ms — covers the 0ms offset plus scheduling jitter.
        try await Task.sleep(for: .milliseconds(100))

        let frame = appState.contextFrames["thread-y"]
        XCTAssertNotNil(frame, "Expected context frame to land under thread-y after dispatch")
        XCTAssertEqual(frame?.threadId, "thread-y")
        XCTAssertEqual(frame?.frame.conversationSummary, "test summary")
        XCTAssertEqual(frame?.frame.activeContext.recentMessages.first?.id, "msg-1")
    }
}
#endif
