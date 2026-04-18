#if DEBUG
import Foundation

// MARK: - Event Script Schema
//
// Phase 14 UAT automation harness — runtime event injector.
//
// Scripts are small JSON files bundled in the APP target under
// `AINMMessenger/Testing/EventScripts/`. At launch (when `-UITestEvents <name>`
// is present), the app loads the matching script and schedules each entry at its
// `atMs` offset, dispatching into the SAME `@MainActor` handlers that
// `AINMClient` wires as callbacks in `AppState.setup()` (lines 124-128, 131-135).
//
// This guarantees tests exercise the identical UI rendering path as production
// server pushes — the injector does NOT stub AppState; it calls the real
// `handleToolEvent(threadId:event:)` method and mutates `contextFrames` directly.
//
// See:
//   - .planning/phases/14-uat-automation-harness/14-CONTEXT.md D-13..D-16
//   - .planning/phases/14-uat-automation-harness/14-RESEARCH.md §"Dynamic event provisioning"

/// Top-level script loaded from scenario-NN-events.json.
struct EventScript: Decodable, Sendable {
    let version: String
    let scenario: String
    let events: [ScriptEntry]
}

/// One scripted event. Field set is a union across all supported event types;
/// only the fields relevant to `type` are populated.
struct ScriptEntry: Decodable, Sendable {
    /// Offset in milliseconds from injector schedule() start.
    let atMs: Int

    /// Event type: `tool.call.started` | `tool.call.progress` | `tool.call.completed`
    /// | `tool.call.error` | `context.frame`. Unknown types are logged + skipped at
    /// dispatch time (the script still decodes) to keep forward-compat smooth.
    let type: String

    /// Target thread id for this event.
    let threadId: String

    // --- Tool-call variants ---
    let callId: String?
    let toolName: String?
    let toolDescription: String?
    let input: [String: AnyCodableValue]?
    let progress: ScriptProgress?
    let status: String?           // "success" | "error"
    let resultSummary: String?
    let durationMs: Int?
    let errorMessage: String?

    // --- Context-frame variant ---
    let frame: ScriptFramePayload?
}

/// Tool-call progress shape (mirrors `ToolCallEvent.ToolProgress`).
struct ScriptProgress: Decodable, Sendable {
    let current: Int?
    let total: Int?
    let detail: String?
}

/// Flat context-frame payload. The injector translates this into the real
/// `ContextFrame` model (which has a nested `Frame` / `ActiveContext` shape).
///
/// References by id (activeMessages, archivalFactIds) must match rows already
/// seeded by the 14-03 SeedLoader for the corresponding scenario.
struct ScriptFramePayload: Decodable, Sendable {
    let summary: String?
    let activeMessages: [String]
    let archivalFactIds: [String]
    let participantIdentities: [String]
}

// MARK: - Injector

/// Schedules and dispatches scripted events into AppState during UAT runs.
///
/// Lifecycle:
///   1. `scheduleIfRequested(script:for:)` called from `AINMMessengerApp.task { }`
///      after `appState.setup()` + seed load.
///   2. Decodes the script from `Bundle.main`.
///   3. Creates one `Task` per entry that sleeps for `atMs` then calls `fire(_:)`.
///   4. `fire` builds the concrete `ToolCallEvent` or `ContextFrame` and hands it to
///      `appState.handleToolEvent(...)` or assigns into `appState.contextFrames[...]`.
///
/// Swift 6 strict concurrency:
///   - Class is `@MainActor`-isolated; all dispatch runs on main actor.
///   - Uses `Task.sleep(for: .milliseconds(...))` (Sendable-clean) rather than
///     legacy GCD delay primitives that require bypassing Sendable checks.
@MainActor
final class TestEventInjector {
    private let appState: AppState
    private var runningTasks: [Task<Void, Never>] = []

    /// Live injectors pinned for the lifetime of a UAT run. The launch-time
    /// bootstrap fires events across a 2+ second window, so the injector must
    /// outlive the synchronous `scheduleIfRequested` return — we keep a strong
    /// reference here and drop it when all tasks finish (or on app exit).
    ///
    /// Using @MainActor ensures no synchronization is needed.
    private static var activeInjectors: [TestEventInjector] = []

    private init(appState: AppState) {
        self.appState = appState
    }

    /// Test-visible entry point: schedule an already-decoded script against an
    /// AppState. Useful for unit tests that build scripts inline rather than
    /// relying on a bundle resource.
    static func schedule(script: EventScript, for appState: AppState) {
        let injector = TestEventInjector(appState: appState)
        activeInjectors.append(injector)
        injector.schedule(script: script)
    }

    /// Launch-time entry point from `AINMMessengerApp.task`. Loads the named
    /// script (e.g. `scenario-01-events`) from the app bundle and schedules all
    /// entries. Silently no-ops when the script cannot be found — tests that
    /// don't pass `-UITestEvents` must not crash.
    static func scheduleIfRequested(script name: String, for appState: AppState) async {
        guard let url = Bundle.main.url(forResource: name, withExtension: "json") else {
            print("[TestEventInjector] script not found in bundle: \(name).json — skipping")
            return
        }
        do {
            let data = try Data(contentsOf: url)
            let script = try JSONDecoder().decode(EventScript.self, from: data)
            let injector = TestEventInjector(appState: appState)
            activeInjectors.append(injector)
            injector.schedule(script: script)
        } catch {
            print("[TestEventInjector] failed to decode \(name).json: \(error)")
        }
    }

    // MARK: - Scheduling

    private func schedule(script: EventScript) {
        for entry in script.events {
            // Strong capture of self inside the Task is intentional: the
            // launch-time caller does not hold a reference, so relying on
            // `weak self` would drop the injector before any events fire. The
            // static `activeInjectors` pin (above) plus this strong capture
            // keeps the injector alive until every task completes.
            let task = Task { @MainActor in
                if entry.atMs > 0 {
                    try? await Task.sleep(for: .milliseconds(entry.atMs))
                }
                guard !Task.isCancelled else { return }
                self.fire(entry)
            }
            runningTasks.append(task)
        }
    }

    // MARK: - Dispatch

    private func fire(_ entry: ScriptEntry) {
        switch entry.type {
        case "tool.call.started":
            guard let callId = entry.callId else {
                print("[TestEventInjector] tool.call.started missing callId; skipping")
                return
            }
            let event = ToolCallEvent(
                callId: callId,
                name: entry.toolName,
                description: entry.toolDescription,
                input: entry.input,
                progress: nil,
                resultSummary: nil,
                durationMs: nil,
                status: nil,
                errorMessage: nil
            )
            appState.handleToolEvent(threadId: entry.threadId, event: event)

        case "tool.call.progress":
            guard let callId = entry.callId else {
                print("[TestEventInjector] tool.call.progress missing callId; skipping")
                return
            }
            let event = ToolCallEvent(
                callId: callId,
                name: entry.toolName,
                description: entry.toolDescription,
                input: entry.input,
                progress: ToolCallEvent.ToolProgress(
                    current: entry.progress?.current,
                    total: entry.progress?.total,
                    detail: entry.progress?.detail
                ),
                resultSummary: nil,
                durationMs: nil,
                status: nil,
                errorMessage: nil
            )
            appState.handleToolEvent(threadId: entry.threadId, event: event)

        case "tool.call.completed":
            guard let callId = entry.callId else {
                print("[TestEventInjector] tool.call.completed missing callId; skipping")
                return
            }
            let event = ToolCallEvent(
                callId: callId,
                name: entry.toolName,
                description: entry.toolDescription,
                input: entry.input,
                progress: nil,
                resultSummary: entry.resultSummary,
                durationMs: entry.durationMs,
                status: entry.status ?? "success",
                errorMessage: nil
            )
            appState.handleToolEvent(threadId: entry.threadId, event: event)

        case "tool.call.error":
            guard let callId = entry.callId else {
                print("[TestEventInjector] tool.call.error missing callId; skipping")
                return
            }
            let event = ToolCallEvent(
                callId: callId,
                name: entry.toolName,
                description: entry.toolDescription,
                input: entry.input,
                progress: nil,
                resultSummary: entry.resultSummary,
                durationMs: entry.durationMs,
                status: "error",
                errorMessage: entry.errorMessage ?? entry.resultSummary ?? "error"
            )
            appState.handleToolEvent(threadId: entry.threadId, event: event)

        case "context.frame":
            guard let payload = entry.frame else {
                print("[TestEventInjector] context.frame missing frame payload; skipping")
                return
            }
            let frame = buildContextFrame(threadId: entry.threadId, payload: payload)
            appState.contextFrames[entry.threadId] = frame

        default:
            print("[TestEventInjector] unknown event type: \(entry.type); skipping")
        }
    }

    // MARK: - Context-frame assembly

    /// Translate the flat script payload into the real `ContextFrame` model.
    /// Uses `ISO8601DateFormatter` timestamps so downstream code that parses them
    /// (e.g. MemoryContextView date labels) works unchanged.
    private func buildContextFrame(threadId: String, payload: ScriptFramePayload) -> ContextFrame {
        let now = ISO8601DateFormatter().string(from: Date())

        let recentMessages = payload.activeMessages.map { id in
            ContextFrame.MessageSummary(
                id: id,
                from: "uat-user@localhost",
                timestamp: now,
                summary: "Seeded active message \(id)"
            )
        }
        let archivalFacts = payload.archivalFactIds.map { factId in
            ContextFrame.ArchivalFact(
                factId: factId,
                key: factId,
                value: .string("seeded archival fact \(factId)"),
                tags: nil,
                storedBy: "uat-user@localhost",
                storedAt: now
            )
        }
        let participants = payload.participantIdentities.map { identity in
            ContextFrame.FrameParticipant(
                identity: identity,
                type: identity.hasPrefix("helper") || identity.contains("agent") ? "agent" : "human",
                presence: "active"
            )
        }

        let activeContext = ContextFrame.ActiveContext(
            recentMessages: recentMessages,
            windowStart: now,
            messageCount: recentMessages.count
        )
        let frame = ContextFrame.Frame(
            activeContext: activeContext,
            conversationSummary: payload.summary,
            archivalFacts: archivalFacts,
            participants: participants
        )
        return ContextFrame(threadId: threadId, frame: frame)
    }
}
#endif
