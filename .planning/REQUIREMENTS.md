# Requirements: AI-Native Messaging Protocol — iOS Reference Client

**Defined:** 2026-03-29
**Core Value:** Enable mixed human-agent threads where any participant (human or agent) can seamlessly join, communicate, and coordinate — with the same quality of experience regardless of who's a person and who's an AI.

## v1 Requirements

Requirements for the native iOS reference client. Each maps to roadmap phases.

### Networking & Identity

- [ ] **NET-01**: App connects to AINM server via WebSocket with JSON-RPC 2.0 framing
- [ ] **NET-02**: App completes `ainm.initialize` handshake with capability negotiation
- [ ] **NET-03**: User can set server URL and display name during onboarding (simple identity flow)
- [ ] **NET-04**: App generates Ed25519 keypair and stores private key in iOS Keychain
- [ ] **NET-05**: App signs all outgoing messages with Ed25519 via Apple CryptoKit
- [ ] **NET-06**: App implements RFC 8785 JCS canonicalization matching Rust server output

### Core Messaging

- [ ] **MSG-01**: User can view conversation list with thread title, last message preview, and timestamps
- [ ] **MSG-02**: User can view message thread with chronological messages and participant avatars
- [ ] **MSG-03**: User can send text messages with Markdown rendering
- [ ] **MSG-04**: App renders multi-block messages (text + code + image in sequence)
- [ ] **MSG-05**: App renders CodeBlocks with syntax highlighting, language label, and copy button
- [ ] **MSG-06**: App renders ImageBlocks with tap-to-fullscreen and size pre-layout
- [ ] **MSG-07**: App renders FileBlocks with filename, size, and download capability
- [ ] **MSG-08**: User can create threads with mixed human and agent participants
- [ ] **MSG-09**: App displays agent identity distinctly (type badge, profile shows Agent Card data)

### Streaming & Real-Time

- [ ] **STR-01**: App renders streaming responses token-by-token with cursor animation
- [ ] **STR-02**: App shows typing/processing indicators for humans and agents
- [ ] **STR-03**: User can cancel an active streaming response
- [ ] **STR-04**: App handles parallel block streaming (multiple blocks receiving deltas simultaneously)

### Interactive Blocks

- [ ] **INT-01**: App renders ButtonBlocks with semantic styles and sends ActionBlock on tap
- [ ] **INT-02**: App renders FormBlocks with all field types and client-side validation
- [ ] **INT-03**: App renders TableBlocks with horizontal scroll and row actions
- [ ] **INT-04**: App renders ApprovalBlocks with state machine (pending/approved/rejected/changes_requested)
- [ ] **INT-05**: App renders fallback text for unrecognized block types

### AI-Native Features

- [ ] **AI-01**: App shows tool call visibility (collapsible indicators with lifecycle grouping)
- [ ] **AI-02**: App supports @-mentions with highlighting and composer autocomplete
- [ ] **AI-03**: User can browse agent discovery (Agent Cards with skills, tools, capabilities)
- [ ] **AI-04**: App displays memory context (active context frames, conversation history)
- [ ] **AI-05**: User can view, search, and delete archival memory facts per agent

### Persistence & Platform

- [ ] **PLT-01**: App caches messages, threads, and participants locally via GRDB.swift
- [ ] **PLT-02**: App provides offline access to cached messages
- [ ] **PLT-03**: App supports light and dark mode via SwiftUI semantic colors
- [ ] **PLT-04**: User can branch sub-threads from any message (one level per spec)
- [ ] **PLT-05**: App supports deep linking to specific threads and messages

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Push & Background

- **PUSH-01**: App receives push notifications for new messages via APNs
- **PUSH-02**: Notification tap opens correct thread
- **PUSH-03**: Server-side APNs device token registration protocol extension

### Platform Expansion

- **PLAT-01**: iPad layout with split view navigation
- **PLAT-02**: Mac Catalyst support
- **PLAT-03**: Accessibility audit (VoiceOver, Dynamic Type, reduced motion)

### Advanced Features

- **ADV-01**: Extension block plugin architecture for custom renderers
- **ADV-02**: Verified message badges (cryptographic verification indicators)
- **ADV-03**: Connection status indicator (connecting/connected/reconnecting/offline)
- **ADV-04**: Tool call timeline (aggregate view of all tool calls in thread)
- **ADV-05**: Haptic feedback on interactive block actions

### Federation UI

- **FED-01**: Federated identity display (show participant's server domain)
- **FED-02**: Cross-server conversation support in UI

## Out of Scope

| Feature | Reason |
|---------|--------|
| Voice/video messaging | Text and rich content only for v1 — massive WebRTC complexity |
| Agent marketplace/store | Discovery yes, marketplace no — premature commerce layer |
| End-to-end encryption | Server-visible messages needed for agent processing, search, memory |
| Custom themes/skins | Light/dark mode only — focus on one polished visual identity |
| Slash commands for bots | Old paradigm — AINM agents are @-mentioned participants, not command responders |
| Read receipts | Not in AINM v1 protocol — presence indicators serve the purpose |
| Message reactions (emoji) | Not in AINM v1 protocol — would create protocol divergence |
| Offline message composition queue | Over-engineering for v1 — disable send when offline |
| ChartBlock rendering | Deliberately excluded from protocol spec |
| Self-hosting tools | App connects to servers, doesn't bundle one |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| NET-01 | — | Pending |
| NET-02 | — | Pending |
| NET-03 | — | Pending |
| NET-04 | — | Pending |
| NET-05 | — | Pending |
| NET-06 | — | Pending |
| MSG-01 | — | Pending |
| MSG-02 | — | Pending |
| MSG-03 | — | Pending |
| MSG-04 | — | Pending |
| MSG-05 | — | Pending |
| MSG-06 | — | Pending |
| MSG-07 | — | Pending |
| MSG-08 | — | Pending |
| MSG-09 | — | Pending |
| STR-01 | — | Pending |
| STR-02 | — | Pending |
| STR-03 | — | Pending |
| STR-04 | — | Pending |
| INT-01 | — | Pending |
| INT-02 | — | Pending |
| INT-03 | — | Pending |
| INT-04 | — | Pending |
| INT-05 | — | Pending |
| AI-01 | — | Pending |
| AI-02 | — | Pending |
| AI-03 | — | Pending |
| AI-04 | — | Pending |
| AI-05 | — | Pending |
| PLT-01 | — | Pending |
| PLT-02 | — | Pending |
| PLT-03 | — | Pending |
| PLT-04 | — | Pending |
| PLT-05 | — | Pending |

**Coverage:**
- v1 requirements: 34 total
- Mapped to phases: 0
- Unmapped: 34

---
*Requirements defined: 2026-03-29*
*Last updated: 2026-03-29 after initial definition*
