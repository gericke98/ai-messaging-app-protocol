---
phase: 13-ai-native-features-platform
verified: 2026-03-30T16:00:00Z
status: human_needed
score: 13/13 must-haves verified
human_verification:
  - test: "Build and run in Xcode Simulator — tool call visibility"
    expected: "When agent executes tool calls, ToolCallIndicatorViews appear below message list, collapsed by default; tap expands to show input JSON, progress, result, or error; spinner for running, green checkmark for completed, red X for error, clock for timed out"
    why_human: "Visual state machine cannot be verified programmatically; requires live WebSocket interaction or mocked events"
  - test: "Build and run in Xcode Simulator — @-mention autocomplete"
    expected: "Typing @ in composer shows filtered participant popup; selecting a participant inserts @DisplayName; backspacing past @ dismisses popup; rendered messages show mention spans in accent blue bold (agent bubbles) or white bold (human bubbles)"
    why_human: "TextField onChange detection, popup positioning, and attributed string rendering require visual confirmation"
  - test: "Build and run in Xcode Simulator — agent discovery"
    expected: "Discover Agents view shows cached agent cards with name, description, skill tags; search filters list; tapping agent shows full profile with Skills, Tools, capability badges; Start Conversation creates thread; Add Agent resolves AINM address"
    why_human: "Requires running app, HTTP fetch mock or real agent, and visual confirmation of all sections"
  - test: "Build and run in Xcode Simulator — memory context"
    expected: "Context pill (brain icon) appears in thread toolbar when agent participant exists; tapping shows half-sheet with Summary, Active Context, Archival Facts preview, Participants; View All navigates to full archival view; search and swipe-to-delete work"
    why_human: "Requires live context frame push from server or mock; sheet presentation, search behavior, and delete confirmation need visual verification"
  - test: "Build and run in Xcode Simulator — sub-threading"
    expected: "Long-pressing a message shows Branch Thread option; branching creates a new sub-thread and navigates to it; sub-thread shows banner 'Thread from {sender}' with back navigation; parent message shows reply count badge; Branch Thread hidden inside sub-threads"
    why_human: "Real-time thread creation, NavigationStack push, and banner state require simulator interaction"
  - test: "Test ainm:// deep linking in Simulator"
    expected: "Opening ainm://thread/{id} navigates to the thread; ainm://thread/{id}/message/{mid} also scrolls to the message; nonexistent thread shows 'Thread Not Found' alert; nonexistent message shows 'Message not found' toast"
    why_human: "URL scheme registration and .onOpenURL handling require xcrun simctl openurl or Safari invocation in simulator"
---

# Phase 13: AI-Native Features & Platform Verification Report

**Phase Goal:** The app showcases AINM's differentiating features — tool call transparency, memory context, agent discovery, sub-threading, and platform integration — delivering a reference-quality iOS client
**Verified:** 2026-03-30T16:00:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #  | Truth                                                                                     | Status     | Evidence                                                                                                                      |
|----|-------------------------------------------------------------------------------------------|------------|-------------------------------------------------------------------------------------------------------------------------------|
| 1  | When an agent executes tool calls, ToolCallIndicatorViews appear below the triggering message in collapsed state | ? VERIFIED* | ToolCallGroupView in MessageThreadView feeds `appState.toolCalls(forThread:)`; wired end-to-end through AINMClient -> AppState -> UI |
| 2  | Tapping a collapsed indicator expands to show input JSON, progress, result, or error      | ? VERIFIED* | ToolCallDetailView has prettyPrinted JSON, progress, result, error sections; ToolCallIndicatorView toggles `isExpanded`        |
| 3  | Tool calls group by call_id — each call_id gets one indicator that updates in place       | ✓ VERIFIED | AppState.handleToolEvent upserts by callId; ToolCallState is keyed by callId in per-thread array                             |
| 4  | Tool calls that run for 5+ minutes without completion show 'Timed out' status            | ✓ VERIFIED | AppState.handleToolEvent schedules `Task.sleep(300s)` timeout that transitions `.running` to `.timedOut`                      |
| 5  | Mentions in rendered messages are highlighted in accent blue bold (white bold in human bubbles) | ? VERIFIED* | MentionTextRenderer.highlightMentions applies accent/white bold; TextBlockView calls it with content.mentions — visual confirmation needed |
| 6  | Typing @ in the composer shows an autocomplete popup with thread participants            | ? VERIFIED* | ComposerView has showMentionPopup state, MentionAutocompleteView overlay, onChange detection — needs simulator to verify UX    |
| 7  | Selecting a participant from autocomplete inserts @DisplayName and stores mention metadata | ✓ VERIFIED | insertMention function in ComposerView replaces @filter with @DisplayName and appends to mentions array                       |
| 8  | User can browse an agent discovery view showing Agent Cards with name, description, and skill tags | ? VERIFIED* | AgentDiscoveryView loads from AgentCardService.loadCachedCards() -> GRDB; accessible via ConversationListView toolbar — visual confirm needed |
| 9  | User can tap 'Start Conversation' to create a new thread with that agent                 | ✓ VERIFIED | AgentCardDetailView calls appState.createThread and sets selectedThreadId; accessibilityLabel confirmed                       |
| 10 | Agent Cards are cached in GRDB with ETag for offline access and conditional refresh      | ✓ VERIFIED | AgentCardService uses If-None-Match header; 304 returns cached; successful fetches save to AgentCardRecord GRDB               |
| 11 | User can tap 'Context' pill in the thread toolbar to view active memory context          | ? VERIFIED* | MessageThreadView has brain/Context toolbar button showing MemoryContextView sheet — visual confirm needed                    |
| 12 | Searching archival facts sends ainm.memory.query; deleting sends ainm.memory.evict       | ✓ VERIFIED | ArchivalMemoryView calls appState.queryMemory (routes to client.memoryQuery) and appState.evictMemoryFact (routes to client.memoryEvict); both use CheckedContinuation pattern |
| 13 | User can branch a sub-thread from any message; sub-thread banner shows back navigation   | ? VERIFIED* | MessageGroupView has Branch Thread context menu; SubThreadBannerView shows "Thread from {sender}" with arrow.uturn.backward — simulator needed |
| 14 | Opening ainm://thread/{id} navigates to the correct thread; message variant scrolls      | ✓ VERIFIED | DeepLinkHandler.parse handles both URL patterns; AINMMessengerApp.onOpenURL sets selectedThreadId and scrollToMessageId; Thread Not Found alert present |

**Score:** 13/13 truths structurally verified (6 need human visual confirmation)

### Required Artifacts

| Artifact                                          | Expected                                          | Status      | Details                                              |
|---------------------------------------------------|---------------------------------------------------|-------------|------------------------------------------------------|
| `ios/.../Models/ToolEvent.swift`                  | ToolCallEvent + ToolCallState + formattedDuration | ✓ VERIFIED  | All 3 constructs confirmed present and substantive   |
| `ios/.../Persistence/ToolCallRecord.swift`        | GRDB record for tool call state                   | ✓ VERIFIED  | struct + databaseTableName confirmed                 |
| `ios/.../UI/ToolVisibility/ToolCallIndicatorView.swift` | Collapsible indicator with 4 lifecycle states | ✓ VERIFIED  | All 4 status icons + accessibilityLabel present      |
| `ios/.../UI/ToolVisibility/ToolCallDetailView.swift`    | Expanded detail sections                      | ✓ VERIFIED  | prettyPrinted JSON, progress, result, error sections |
| `ios/.../UI/Mentions/MentionTextRenderer.swift`   | AttributedString mention highlighter              | ✓ VERIFIED  | highlightMentions, foregroundColor, body.bold        |
| `ios/.../UI/Mentions/MentionAutocompleteView.swift` | Overlay popup for participant picker            | ✓ VERIFIED  | MentionCandidate, filtered list, accessibility       |
| `ios/.../Models/AgentCard.swift`                  | AgentCard with A2A + AINM extensions              | ✓ VERIFIED  | AgentCard, AINMExtension, Skill, Tool structs        |
| `ios/.../Networking/AgentCardService.swift`       | HTTP fetch with ETag caching                      | ✓ VERIFIED  | actor, fetchCard, fetchCardByAddress, loadCachedCards, If-None-Match |
| `ios/.../Persistence/AgentCardRecord.swift`       | GRDB record for cached agent cards                | ✓ VERIFIED  | struct + agentCard() decode helper                   |
| `ios/.../UI/AgentDiscovery/AgentDiscoveryView.swift` | Browsable list with search + add-by-address    | ✓ VERIFIED  | All required strings and search present              |
| `ios/.../UI/AgentDiscovery/AgentCardDetailView.swift` | Full profile with Start Conversation          | ✓ VERIFIED  | Skills, Tools, Start Conversation, capability badges |
| `ios/.../Models/MemoryModels.swift`               | ContextFrame + MemoryQueryResult                  | ✓ VERIFIED  | All 4 structs present                                |
| `ios/.../Persistence/MemoryFactRecord.swift`      | GRDB record for archival facts                    | ✓ VERIFIED  | struct + databaseTableName + decodedTags             |
| `ios/.../UI/Memory/MemoryContextView.swift`       | Half-sheet with 4 sections                        | ✓ VERIFIED  | Summary, Active Context, Archival Facts, Participants, presentationDetents |
| `ios/.../UI/Memory/ArchivalMemoryView.swift`      | Full-screen searchable facts with delete          | ✓ VERIFIED  | search, delete confirmation, evictMemoryFact, queryMemory |
| `ios/.../UI/SubThreading/SubThreadIndicatorView.swift` | Badge with reply count                       | ✓ VERIFIED  | bubble.left.and.bubble.right, replies, accessibilityLabel |
| `ios/.../UI/SubThreading/SubThreadBannerView.swift` | Parent navigation banner                       | ✓ VERIFIED  | "Thread from", "View Parent", arrow.uturn.backward   |
| `ios/.../App/DeepLinkHandler.swift`               | URL scheme parser                                 | ✓ VERIFIED  | enum Route, parse(), ainm://thread/ cases             |
| `ios/AINMMessenger/Info.plist`                    | ainm:// URL scheme registration                   | ✓ VERIFIED  | CFBundleURLSchemes contains "ainm", CFBundleURLName set |

### Key Link Verification

| From                        | To                          | Via                          | Status      | Details                                              |
|-----------------------------|-----------------------------|------------------------------|-------------|------------------------------------------------------|
| `AINMClient.swift`          | `AppState.swift`            | setToolEventCallback         | ✓ WIRED     | AppState line 113 wires callback; handleToolEvent updates toolCallStates |
| `ToolCallIndicatorView.swift` | `ToolCallState`           | @Bindable var state          | ✓ WIRED     | Takes ToolCallState directly; ToolCallGroupView passes from appState |
| `TextBlockView.swift`       | `MentionTextRenderer.swift` | highlightMentions call       | ✓ WIRED     | TextBlockView calls MentionTextRenderer.highlightMentions(in:mentions:isHumanBubble:) |
| `ComposerView.swift`        | `MentionAutocompleteView.swift` | overlay on @ detection   | ✓ WIRED     | showMentionPopup state drives MentionAutocompleteView overlay |
| `AgentDiscoveryView.swift`  | `AgentCardService.swift`    | loadCachedCards / fetchByAddress | ✓ WIRED | loadAgents() creates AgentCardService and calls both methods |
| `AgentCardDetailView.swift` | `AppState.swift`            | appState.createThread        | ✓ WIRED     | Start Conversation calls appState.createThread, sets selectedThreadId |
| `AgentCardService.swift`    | `AgentCardRecord.swift`     | GRDB persist after fetch     | ✓ WIRED     | record.save(db) after successful HTTP 200 response   |
| `AINMClient.swift`          | `AppState.swift`            | setContextFrameCallback      | ✓ WIRED     | AppState line 120 wires callback; contextFrames[frame.threadId] updated |
| `ArchivalMemoryView.swift`  | `AINMClient.swift`          | queryMemory / evictMemoryFact | ✓ WIRED    | Routes through AppState -> client.memoryQuery / memoryEvict with CheckedContinuation |
| `AINMMessengerApp.swift`    | `DeepLinkHandler.swift`     | .onOpenURL handler           | ✓ WIRED     | onOpenURL at line 24 calls DeepLinkHandler.parse and routes to AppState |
| `DeepLinkHandler.swift`     | `AppState.swift`            | selectedThreadId + scrollToMessageId | ✓ WIRED | AINMMessengerApp sets both AppState properties on deep link match |
| `MessageGroupView.swift`    | `SubThreadIndicatorView.swift` | subThreadInfo closure     | ✓ WIRED     | MessageGroupView line 49 passes subThreadInfo?.replyCount to SubThreadIndicatorView |
| `MessageThreadView.swift`   | `SubThreadBannerView.swift` | parentThreadId condition     | ✓ WIRED     | MessageThreadView line 19 renders SubThreadBannerView when parentThread present |

### Data-Flow Trace (Level 4)

| Artifact                  | Data Variable         | Source                                      | Produces Real Data | Status       |
|---------------------------|-----------------------|---------------------------------------------|--------------------|--------------|
| `ToolCallGroupView`       | toolCalls             | appState.toolCalls(forThread:) from toolCallStates dict | Yes — populated by WebSocket ainm.tool.call.* events via AINMClient | ✓ FLOWING |
| `AgentDiscoveryView`      | agents                | AgentCardService.loadCachedCards() -> GRDB AgentCardRecord | Yes — GRDB query returns all cached records | ✓ FLOWING |
| `MemoryContextView`       | frame                 | appState.contextFrame(forThread:) from contextFrames dict | Yes — populated by WebSocket ainm.context.frame push | ✓ FLOWING |
| `ArchivalMemoryView`      | facts                 | appState.contextFrames.values.flatMap { $0.frame.archivalFacts } | Yes — derives from live contextFrames, searchable via memoryQuery | ✓ FLOWING |
| `SubThreadIndicatorView`  | replyCount            | MessageThreadViewModel.subThreadCache from GRDB ThreadRecord query | Yes — GRDB SELECT by parentMessageId during loadInitial | ✓ FLOWING |

### Behavioral Spot-Checks

Step 7b: SKIPPED — this is an iOS app with no runnable entry points without Xcode. Build compilation was verified by task-level xcodebuild calls during execution, and all 10 commits are present in git log.

### Requirements Coverage

| Requirement | Source Plan | Description                                                                  | Status      | Evidence                                                   |
|-------------|------------|------------------------------------------------------------------------------|-------------|-------------------------------------------------------------|
| AI-01       | 13-01       | App shows tool call visibility (collapsible indicators with lifecycle grouping) | ✓ SATISFIED | ToolCallIndicatorView, ToolCallGroupView in MessageThreadView, full event pipeline wired |
| AI-02       | 13-02       | App supports @-mentions with highlighting and composer autocomplete          | ✓ SATISFIED | MentionTextRenderer + TextBlockView + MentionAutocompleteView + ComposerView all wired |
| AI-03       | 13-03       | User can browse agent discovery (Agent Cards with skills, tools, capabilities) | ✓ SATISFIED | AgentDiscoveryView, AgentCardDetailView, AgentCardService with ETag caching   |
| AI-04       | 13-04       | App displays memory context (active context frames, conversation history)    | ✓ SATISFIED | MemoryContextView sheet with context pill in MessageThreadView toolbar       |
| AI-05       | 13-04       | User can view, search, and delete archival memory facts per agent            | ✓ SATISFIED | ArchivalMemoryView with search (ainm.memory.query) and delete (ainm.memory.evict) |
| PLT-04      | 13-05       | User can branch sub-threads from any message (one level per spec)            | ✓ SATISFIED | Branch Thread context menu, SubThreadIndicatorView, SubThreadBannerView, createSubThread |
| PLT-05      | 13-05       | App supports deep linking to specific threads and messages                   | ✓ SATISFIED | DeepLinkHandler, ainm:// Info.plist registration, onOpenURL wiring, scroll-to-message |

**Orphaned requirements:** None — all 7 requirement IDs (AI-01 through AI-05, PLT-04, PLT-05) are claimed in plan frontmatter and verified in code.

### Anti-Patterns Found

No TODO, FIXME, HACK, PLACEHOLDER, or empty implementation patterns found in any of the 21 phase 13 files scanned.

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| — | — | None found | — | — |

### Human Verification Required

All automated structural, substantive, wiring, and data-flow checks passed. The following items require simulator testing because they involve visual presentation, real-time behavior, or interactive UX:

#### 1. Tool Call Visibility — Visual States

**Test:** Connect to a server (or mock) that emits `ainm.tool.call.started`, `ainm.tool.call.progress`, `ainm.tool.call.completed` events. Send a message in a thread with an agent.
**Expected:** Collapsed indicator appears below message list with spinner (running), updates to green checkmark (completed) or red X (error). Tapping expands to show input JSON panel, progress bar, result summary. After 5 minutes without completion, shows clock icon and "Timed out".
**Why human:** Cannot verify animated state transitions, SF Symbol rendering, or 5-minute timeout without live runtime.

#### 2. @-Mention Autocomplete UX

**Test:** Open a thread, type `@` in the composer. Then type a character to filter. Select a participant.
**Expected:** Popup appears above composer with participant list filtered by typed text. Selecting inserts `@DisplayName `. Backspacing over the `@` dismisses popup. Rendered messages from the server that contain mention spans show them in accent blue bold (agent bubbles) or white bold (human bubbles).
**Why human:** TextField onChange detection timing, popup overlay positioning (offset -216pt), and AttributedString rendering quality need visual confirmation.

#### 3. Agent Discovery — Full Flow

**Test:** Navigate via the toolbar button in ConversationListView. Search for an agent by name. Tap an agent card. Verify all sections render. Tap Start Conversation.
**Expected:** Discovery list shows name, description, skill tag pills. Detail view shows capability badges (Streaming, Memory, Sub-threads), Skills section, Tools section. Start Conversation creates thread and navigates to it.
**Why human:** HTTP fetch from real server or mock needed; visual layout of nested sections requires inspection; navigation to new thread needs confirmation.

#### 4. Memory Context Sheet

**Test:** Open a thread with an agent participant. Tap the "Context" brain pill in the toolbar. View the sheet. Navigate to Archival Memory, search for a term, swipe-delete a fact.
**Expected:** Half-sheet presents with Summary, Active Context, Archival Facts preview, Participants sections. "View All" navigates to full view. Search triggers ainm.memory.query. Delete shows confirmation alert "This will permanently remove this fact from the agent's memory." Confirmed delete removes the row with animation.
**Why human:** Requires server to push `ainm.context.frame` notification; sheet detent behavior, search submission timing, and delete animation need visual confirmation.

#### 5. Sub-Threading — Branch and Navigate

**Test:** Long-press a message in a thread. Tap "Branch Thread". Navigate back using banner.
**Expected:** Context menu shows "Branch Thread" with arrow.branch icon. Tapping creates new sub-thread and navigates to it. Sub-thread shows "Thread from {senderName}" banner with "View Parent" button. Parent message shows "N replies" badge. "Branch Thread" does not appear in sub-threads.
**Why human:** Context menu rendering, NavigationStack push animation, GRDB sub-thread creation, and banner state all require live execution.

#### 6. Deep Linking via ainm:// URL Scheme

**Test:** In Simulator terminal: `xcrun simctl openurl booted "ainm://thread/some-thread-id"`. Then try with a message ID. Then try with a nonexistent thread ID.
**Expected:** App navigates to matching thread. With message ID, also scrolls to that message. With nonexistent thread, shows "Thread Not Found" alert with "This thread is not available on your device." message.
**Why human:** URL scheme registration requires OS-level activation; scroll position and alert presentation need simulator observation.

### Gaps Summary

No functional gaps found. All 18 artifact files exist with substantive implementations. All 13 key links are wired. All 5 data flows produce real data. All 7 requirement IDs (AI-01 through AI-05, PLT-04, PLT-05) are satisfied in code with verified evidence. All 10 task commits (f69bd8f, 216dc26, 11df337, ebae602, 16adad4, 79b293b, 0a69391, b671c7a, 99bec61, 6f7fd2e) are present in the repository.

Status is `human_needed` rather than `passed` because 6 of 14 truths involve visual rendering, animated state transitions, or real-time WebSocket interaction that cannot be verified without running the app in Xcode Simulator. These are all quality/UX confirmation items — the underlying code is structurally complete and connected.

---

_Verified: 2026-03-30T16:00:00Z_
_Verifier: Claude (gsd-verifier)_
