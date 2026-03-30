---
phase: 13-ai-native-features-platform
plan: 05
subsystem: ui
tags: [swiftui, sub-threading, deep-linking, url-scheme, navigation, ios]

requires:
  - phase: 13-01
    provides: Tool call visibility and ToolCallState model
  - phase: 13-02
    provides: Mention support and ComposerView enhancements
  - phase: 13-03
    provides: AgentDiscoveryView and AgentCard models
  - phase: 13-04
    provides: MemoryContextView and context frame handling
provides:
  - SubThreadIndicatorView for sub-thread reply count badges
  - SubThreadBannerView for parent navigation in sub-threads
  - Branch Thread context menu action on messages
  - DeepLinkHandler for ainm:// URL scheme parsing
  - URL scheme registration in Info.plist
  - AgentDiscovery toolbar button in ConversationListView
affects: [ios-app, navigation, threading]

tech-stack:
  added: []
  patterns:
    - Sub-thread cache pattern in MessageThreadViewModel for O(1) indicator lookups
    - Deep link handling via .onOpenURL with async thread existence verification

key-files:
  created:
    - ios/AINMMessenger/AINMMessenger/UI/SubThreading/SubThreadIndicatorView.swift
    - ios/AINMMessenger/AINMMessenger/UI/SubThreading/SubThreadBannerView.swift
    - ios/AINMMessenger/AINMMessenger/App/DeepLinkHandler.swift
    - ios/AINMMessenger/Info.plist
  modified:
    - ios/AINMMessenger/AINMMessenger/App/AppState.swift
    - ios/AINMMessenger/AINMMessenger/Networking/AINMClient.swift
    - ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageGroupView.swift
    - ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift
    - ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadViewModel.swift
    - ios/AINMMessenger/AINMMessenger/App/AINMMessengerApp.swift
    - ios/AINMMessenger/AINMMessenger/UI/ConversationList/ConversationListView.swift
    - ios/AINMMessenger/AINMMessenger.xcodeproj/project.pbxproj

key-decisions:
  - "Info.plist placed at project root (not source dir) to avoid Xcode auto-resource copy conflict"
  - "Sub-thread cache loaded eagerly during loadInitial for O(1) indicator lookups"

patterns-established:
  - "Sub-thread indicator pattern: query ThreadRecord by parentMessageId, cache in view model"
  - "Deep link pattern: DeepLinkHandler.parse -> async thread check -> set AppState navigation properties"

requirements-completed: [PLT-04, PLT-05]

duration: 8min
completed: 2026-03-30
---

# Phase 13 Plan 05: Sub-Threading and Deep Linking Summary

**Sub-thread branching with context menu, parent/child navigation, ainm:// deep links, and AgentDiscovery toolbar access**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-30T15:07:54Z
- **Completed:** 2026-03-30T15:16:16Z
- **Tasks:** 3 (2 auto + 1 checkpoint auto-approved)
- **Files modified:** 12

## Accomplishments
- Sub-thread branching from any message via long-press context menu (hidden in sub-threads for one-level limit)
- Sub-thread indicator badge showing reply count on parent messages with navigation tap
- Sub-thread banner with "Thread from {sender}" and back-to-parent navigation
- Deep link handler parsing ainm://thread/{id} and ainm://thread/{id}/message/{mid}
- URL scheme registration (ainm://) in Info.plist for OS-level deep link routing
- AgentDiscovery toolbar button in conversation list for agent browsing
- Scroll-to-message support with "Message not found" toast fallback

## Task Commits

Each task was committed atomically:

1. **Task 1: Sub-threading views, branch action, and sub-thread creation** - `99bec61` (feat)
2. **Task 2: Deep linking, URL scheme registration, and AgentDiscovery access** - `6f7fd2e` (feat)
3. **Task 3: Visual verification** - auto-approved (checkpoint)

## Files Created/Modified
- `UI/SubThreading/SubThreadIndicatorView.swift` - Badge view for sub-thread reply counts on parent messages
- `UI/SubThreading/SubThreadBannerView.swift` - Banner at top of sub-threads with back navigation
- `App/DeepLinkHandler.swift` - URL scheme parser for ainm:// deep links
- `Info.plist` - URL scheme registration for ainm://
- `App/AppState.swift` - Added scrollToMessageId and createSubThread method
- `Networking/AINMClient.swift` - Added createSubThread protocol method
- `UI/MessageThread/MessageGroupView.swift` - Sub-thread indicator, context menu, branch action
- `UI/MessageThread/MessageThreadView.swift` - Sub-thread banner, scroll-to-message, message-not-found toast
- `UI/MessageThread/MessageThreadViewModel.swift` - Sub-thread cache for indicator lookups
- `App/AINMMessengerApp.swift` - Deep link handler with thread existence verification
- `UI/ConversationList/ConversationListView.swift` - AgentDiscovery toolbar navigation
- `project.pbxproj` - INFOPLIST_FILE build setting

## Decisions Made
- Placed Info.plist at project root (ios/AINMMessenger/) instead of source directory to avoid Xcode auto-resource-copy conflict with GENERATE_INFOPLIST_FILE=YES
- Sub-thread cache loaded eagerly during MessageThreadViewModel.loadInitial() for instant indicator rendering without per-message queries

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Info.plist placement conflict with Xcode auto-resource copy**
- **Found during:** Task 2 (Deep linking)
- **Issue:** Placing Info.plist in AINMMessenger/ source directory caused "Multiple commands produce Info.plist" build error because Xcode's auto-include feature copies it as a resource AND processes it as the Info.plist
- **Fix:** Moved Info.plist to project root (ios/AINMMessenger/Info.plist) and updated INFOPLIST_FILE build setting to "Info.plist"
- **Files modified:** ios/AINMMessenger/Info.plist, project.pbxproj
- **Verification:** Build succeeds with no errors
- **Committed in:** 6f7fd2e (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Build config fix necessary for compilation. No scope creep.

## Issues Encountered
None beyond the Info.plist placement issue documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All Phase 13 features complete: tool visibility, @-mentions, agent discovery, memory context, sub-threading, deep linking
- Ready for visual verification in Xcode Simulator
- Ready for next milestone planning

---
*Phase: 13-ai-native-features-platform*
*Completed: 2026-03-30*
