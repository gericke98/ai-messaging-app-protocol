---
phase: 13-ai-native-features-platform
plan: 01
subsystem: ui
tags: [swiftui, grdb, websocket, tool-visibility, json-rpc, observable]

requires:
  - phase: 12-streaming-interactive-blocks
    provides: "Streaming callbacks, ContentBlockRenderer, MessageThreadView, AppState patterns"
provides:
  - "ToolCallEvent model for decoding ainm.tool.call.* notifications"
  - "ToolCallState @Observable lifecycle model for SwiftUI"
  - "ToolCallRecord GRDB persistence for tool call history"
  - "AINMClient tool event handling with GRDB upsert"
  - "ToolCallIndicatorView with 4-state lifecycle (running/completed/error/timedOut)"
  - "ToolCallDetailView with input JSON, progress, result, error sections"
  - "AppState.toolCallStates with 5-minute timeout detection"
affects: [13-02, 13-03, 13-04, 13-05]

tech-stack:
  added: []
  patterns: ["Tool event callback pattern (AINMClient -> AppState -> UI)", "5-minute timeout scheduling for running tool calls", "GRDB upsert pattern for tool call records"]

key-files:
  created:
    - "ios/AINMMessenger/AINMMessenger/Models/ToolEvent.swift"
    - "ios/AINMMessenger/AINMMessenger/Persistence/ToolCallRecord.swift"
    - "ios/AINMMessenger/AINMMessenger/UI/ToolVisibility/ToolCallIndicatorView.swift"
    - "ios/AINMMessenger/AINMMessenger/UI/ToolVisibility/ToolCallDetailView.swift"
  modified:
    - "ios/AINMMessenger/AINMMessenger/Models/MessageEnvelope.swift"
    - "ios/AINMMessenger/AINMMessenger/Persistence/AppDatabase.swift"
    - "ios/AINMMessenger/AINMMessenger/Networking/AINMClient.swift"
    - "ios/AINMMessenger/AINMMessenger/App/AppState.swift"
    - "ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift"

key-decisions:
  - "@MainActor @Observable on ToolCallState since it only drives UI views"
  - "Tool call indicators shown below message list above streaming messages, not attached to individual message bubbles"
  - "GRDB upsert pattern: insert on started, update on progress/completed"

patterns-established:
  - "Tool event callback: AINMClient.onToolEvent -> AppState.handleToolEvent -> ToolCallState mutation -> SwiftUI observation"
  - "Timeout scheduling: Task.sleep(300s) with cancellation on completion event"
  - "UnevenRoundedRectangle for collapsible card UI (0pt top radius when expanded)"

requirements-completed: [AI-01]

duration: 8min
completed: 2026-03-30
---

# Phase 13 Plan 01: Tool Call Visibility Summary

**Tool call lifecycle indicators with ToolCallEvent model, GRDB persistence, AINMClient notification handling, and collapsible ToolCallIndicatorView showing running/completed/error/timedOut states**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-30T07:44:25Z
- **Completed:** 2026-03-30T07:52:08Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- Full tool call data pipeline: WebSocket notification -> ToolCallEvent decode -> GRDB persist -> AppState observable -> ToolCallIndicatorView
- Collapsible indicator with 4 lifecycle states (running spinner, green checkmark, red X, clock timeout) per UI-SPEC Component 1
- Expanded detail view with pretty-printed input JSON, progress bar, result summary, and error display
- 5-minute timeout detection transitions running tools to timedOut status automatically

## Task Commits

Each task was committed atomically:

1. **Task 1: Tool event models, GRDB record, and DB migration** - `f69bd8f` (feat)
2. **Task 2: AINMClient tool event handling, AppState wiring, and ToolCallIndicatorView + ToolCallDetailView** - `216dc26` (feat)

## Files Created/Modified
- `ios/AINMMessenger/AINMMessenger/Models/ToolEvent.swift` - ToolCallEvent Codable struct, ToolCallState @Observable class, Int.formattedDuration extension
- `ios/AINMMessenger/AINMMessenger/Persistence/ToolCallRecord.swift` - GRDB record with thread foreign key and query helpers
- `ios/AINMMessenger/AINMMessenger/Persistence/AppDatabase.swift` - v3_create_tool_calls migration
- `ios/AINMMessenger/AINMMessenger/Models/MessageEnvelope.swift` - 3 new MessageType cases for tool events
- `ios/AINMMessenger/AINMMessenger/Networking/AINMClient.swift` - onToolEvent callback, handleToolEvent GRDB upsert, 3 notification cases
- `ios/AINMMessenger/AINMMessenger/App/AppState.swift` - toolCallStates dictionary, handleToolEvent with timeout, toolCalls(forThread:)
- `ios/AINMMessenger/AINMMessenger/UI/ToolVisibility/ToolCallIndicatorView.swift` - Collapsible indicator with status icons, accessibility, ToolCallGroupView
- `ios/AINMMessenger/AINMMessenger/UI/ToolVisibility/ToolCallDetailView.swift` - Input JSON, progress, result, error sections
- `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift` - ToolCallGroupView integration

## Decisions Made
- Used @MainActor @Observable on ToolCallState rather than nonisolated(unsafe) since it exclusively drives SwiftUI views
- Placed tool call indicators in MessageThreadView between permanent messages and streaming messages (not attached to individual message bubbles) for simplicity and because tool events arrive as separate notifications without direct message-to-tool mapping
- Used GRDB upsert pattern (insert on started, update on progress/completed) to handle out-of-order events gracefully

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Tool call visibility fully functional, ready for Phase 13 plans 02-05
- ToolCallState is observable and can be extended for future features
- ToolCallGroupView pattern ready for integration with mention autocomplete and other AI-native features

---
*Phase: 13-ai-native-features-platform*
*Completed: 2026-03-30*
