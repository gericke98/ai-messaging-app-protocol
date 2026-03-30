---
phase: 12-streaming-interactive-blocks
plan: 04
subsystem: ios-client
tags: [typing-indicator, action-block, presence, interactive-blocks, swiftui]
dependency_graph:
  requires: [12-01, 12-02]
  provides: [action-block-flow, typing-indicators, presence-handling]
  affects: [AppState, MessageThreadView, MessageGroupView, AgentMessageView, AINMClient]
tech_stack:
  added: []
  patterns: [closure-chain-wiring, presence-cleanup-timer, AnyCodableValue-fromAny]
key_files:
  created:
    - ios/AINMMessenger/AINMMessenger/UI/MessageThread/TypingIndicatorView.swift
  modified:
    - ios/AINMMessenger/AINMMessenger/App/AppState.swift
    - ios/AINMMessenger/AINMMessenger/Networking/AINMClient.swift
    - ios/AINMMessenger/AINMMessenger/Models/AnyCodableValue.swift
    - ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift
    - ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageGroupView.swift
    - ios/AINMMessenger/AINMMessenger/UI/MessageThread/AgentMessageView.swift
decisions:
  - Used AvatarView with participantType parameter instead of name+isAgent since AvatarView API takes participantType string
  - Added AnyCodableValue.fromAny static method to bridge [String: Any] from onAction closures to Codable ActionContent
  - Wired presence and message-delivered callbacks in AppState.setup() via Task to bridge MainActor to actor isolation
metrics:
  duration: 220s
  completed: "2026-03-30T05:01:00Z"
  tasks_completed: 2
  tasks_total: 2
  files_changed: 7
---

# Phase 12 Plan 04: ActionBlock Response Flow & Typing Indicators Summary

Typing indicators with bouncing dots animation and 30-second stale guard, plus full ActionBlock response wiring from interactive block taps through the view hierarchy to signed protocol messages.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | e0761fe | TypingIndicatorView, presence handling, ActionBlock sending, AnyCodableValue.fromAny |
| 2 | 21a9311 | Wire onAction closure through MessageGroupView and AgentMessageView |

## What Was Built

### Task 1: TypingIndicatorView + Presence Handling + ActionBlock Sending

- **TypingIndicatorView.swift**: Bouncing dots animation (3 circles, 6pt, 0.15s stagger) with `accessibilityReduceMotion` gate. Shows avatar, dots, and "{name} is typing/processing..." label.
- **AppState.swift**: Added `PresenceEntry` model, `activePresence` dictionary, `handlePresence()` method, `schedulePresenceCleanup()` with 30-second Task-based timer, `presenceEntries(forThread:)` filtered query, and `sendActionBlock()` method.
- **AINMClient.swift**: Added `setPresenceCallback()` and `setMessageDeliveredCallback()` methods. Updated `handleMessageDelivery()` to call `onMessageDelivered` to clear typing indicators when a message arrives.
- **AnyCodableValue.swift**: Added `fromAny(_ value: Any)` static method for converting untyped `[String: Any]` dictionaries to `AnyCodableValue`.
- **MessageThreadView.swift**: Added `handleAction()` method and typing indicator rendering via `TypingIndicatorView` in the message list.

### Task 2: ActionBlock Closure Chain Wiring

- **MessageGroupView.swift**: Added `onAction` parameter that curries messageId from the message record and passes blockIndex+actionId+data downstream.
- **AgentMessageView.swift**: Added `onAction` parameter that binds block index and passes actionId+data to `ContentBlockRenderer.onAction`.

Complete chain: `MessageThreadView.handleAction` -> `MessageGroupView.onAction` -> `AgentMessageView.onAction` -> `ContentBlockRenderer.onAction` -> `ButtonBlockView/FormBlockView/TableBlockView/ApprovalBlockView.onAction`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added AnyCodableValue.fromAny helper**
- **Found during:** Task 1
- **Issue:** `AnyCodableValue(data)` where data is `[String: Any]` does not compile because `[String: Any]` is not `Encodable`. The existing `init(_ encodable:)` requires `Encodable` conformance.
- **Fix:** Added `static func fromAny(_ value: Any) -> AnyCodableValue` that recursively converts untyped values.
- **Files modified:** `AnyCodableValue.swift`
- **Commit:** e0761fe

**2. [Rule 2 - Critical] AvatarView API adaptation**
- **Found during:** Task 1
- **Issue:** Plan called for `AvatarView(name: name, isAgent: isAgent)` but actual AvatarView API is `AvatarView(participantType: String, size: CGFloat)`.
- **Fix:** Used `AvatarView(participantType: isAgent ? "agent" : "human", size: 24)` in TypingIndicatorView.
- **Files modified:** `TypingIndicatorView.swift`
- **Commit:** e0761fe

## Known Stubs

None. All data paths are fully wired.

## Self-Check: PASSED
