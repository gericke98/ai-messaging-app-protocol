# Roadmap: AI-Native Messaging Protocol

## Milestones

- ✅ **v3.1 AI-Native Messaging Protocol v1 Specification & Demo** — Phases 1-6 (shipped 2026-03-29) — [Archive](milestones/v3.1-ROADMAP.md)

## Current Milestone: v3.2 iOS Reference Client

### Phase 12: Streaming & Interactive Blocks
**Goal**: Agent responses stream token-by-token in real-time and users can interact with rich UI blocks (buttons, forms, tables, approvals) that send responses back through the protocol
**Depends on**: Phase 11
**Requirements**: STR-01, STR-02, STR-03, STR-04, INT-01, INT-02, INT-03, INT-04, INT-05
**Success Criteria** (what must be TRUE):
  1. When an agent streams a response, tokens appear one-by-one with a cursor animation at 30-60fps — no main thread starvation even at 100+ deltas/second
  2. User can tap "Stop Generating" during an active stream and the agent stops — the partial response is preserved in the thread
  3. User taps a ButtonBlock and the app sends a signed ActionBlock response as a new thread message; FormBlocks validate input client-side before submission; ApprovalBlocks cycle through pending/approved/rejected states
  4. TableBlocks render with horizontal scroll for wide data and row-level action buttons; unrecognized block types display their fallback text instead of crashing
  5. Typing/processing indicators show for both humans and agents, and parallel block streaming (two blocks receiving deltas simultaneously) renders correctly
**Plans**: 4 plans
Plans:
- [x] 12-01-PLAN.md -- Streaming data layer: StreamEvent models, StreamingManager actor, AINMClient stream handling
- [x] 12-02-PLAN.md -- Interactive block models and renderers: ContentBlock extension, 5 new SwiftUI views
- [x] 12-03-PLAN.md -- Streaming UI: StreamingViewModel, StreamingMessageView, CursorView, StopGeneratingButton
- [x] 12-04-PLAN.md -- ActionBlock response flow, TypingIndicatorView, presence handling
**UI hint**: yes

### Phase 13: AI-Native Features & Platform
**Goal**: The app showcases AINM's differentiating features — tool call transparency, memory context, agent discovery, sub-threading, and platform integration — delivering a reference-quality iOS client
**Depends on**: Phase 12
**Requirements**: AI-01, AI-02, AI-03, AI-04, AI-05, PLT-04, PLT-05
**Success Criteria** (what must be TRUE):
  1. When an agent executes tool calls, the user sees collapsible indicators grouped by call_id showing the started/progress/completed lifecycle — tool execution is transparent, not hidden
  2. User can @-mention agents and humans in the composer with autocomplete, and mentions are highlighted in rendered messages
  3. User can browse an agent discovery view showing Agent Cards with skills, tools, and capabilities — and start a conversation with a discovered agent
  4. User can view active memory context frames for an agent in a thread, search archival memory facts, and delete specific facts — memory is transparent and user-controlled
  5. User can branch a sub-thread from any message (one level per spec), navigate between parent and sub-thread, and open a specific thread/message via deep link URL
**Plans**: TBD
**UI hint**: yes

## Progress

**Execution Order:**
Phases execute in numeric order: 10 -> 11 -> 12 -> 13

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 10. Foundation | v3.3 | 5/5 | Complete    | 2026-03-30 |
| 11. Core Messaging UI | v3.3 | 4/4 | Complete    | 2026-03-30 |
| 12. Streaming & Interactive Blocks | v3.3 | 4/4 | Complete    | 2026-03-30 |
| 13. AI-Native Features & Platform | v3.3 | 0/0 | Not started | - |
