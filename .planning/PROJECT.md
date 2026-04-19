# AI-Native Messaging Protocol & App

## What This Is

An open, federated messaging protocol designed from the ground up for AI agent communication — where humans and agents are equal participants in conversations. The protocol defines how agents declare capabilities, exchange messages, stream responses, and render rich interactive UI. A native iOS app serves as the reference client, showcasing what messaging looks like when agents are first-class citizens, not bolted-on bots.

## Core Value

The protocol must enable mixed human-agent threads where any participant (human or agent) can seamlessly join, communicate, and coordinate — with the same quality of experience regardless of who's a person and who's an AI.

**Memory as onboarding:** The protocol's memory system is the institutional knowledge layer. When a new agent joins a thread (or replaces an existing one), it inherits accumulated context — user preferences, prior decisions, what other agents learned. Memory compounds over time: the more you use the protocol, the better every agent becomes. Agent replacement is seamless, multi-agent collaboration is informed from the start, and no context is lost when participants change.

## Requirements

### Validated

- ✓ Open protocol specification for AI-native messaging — v3.1
- ✓ Email-like identity model for both humans and agents (`name@provider`) — v3.1
- ✓ Mixed human-agent threads — both as equal participants — v3.1
- ✓ Flat conversations by default with optional sub-threading — v3.1
- ✓ Security foundations — cryptographic message signing, identity verification, anti-prompt-injection — v3.1
- ✓ Agent capability discovery (Agent Cards / capability manifests) — v3.1
- ✓ Rich interactive content from agents (forms, buttons, tables, approvals) — v3.1
- ✓ Streaming support — agents can stream responses in real-time — v3.1
- ✓ Context/memory management at the protocol level (active context, conversation history, archival) — v3.1
- ✓ Tool call transparency — visible to all thread participants — v3.1
- ✓ MCP and A2A ecosystem compatibility bridges — v3.1
- ✓ Working demo that proves the protocol works end-to-end — v3.1
- ✓ Published formal protocol specification (JSON Schema 2020-12 + AsyncAPI 3.1.0) — v3.1
- ✓ Protocol versioning with backwards compatibility rules — v3.1

- ✓ Native iOS app (SwiftUI) as reference client — full AINM protocol implementation — v3.2
- ✓ WebSocket + JSON-RPC 2.0 client with streaming, tool visibility, memory, sub-threading, deep links, @-mentions — v3.2
- ✓ Hermetic XCUITest harness (AINMMessengerUITests target, launch-arg seed bootstrap, `make uat` orchestrator) — v3.2

### Active

## Current Milestone: v3.3 Universal Agent Client

**Goal:** Turn AINM from a reference iOS client for the protocol into a universal client where any agent runtime (Claude Code, Codex, GPT, MCP servers, custom Python) can connect and participate as a first-class thread member — with compound memory groundwork laid for v3.4.

**Target features:**
- v3.2 tech debt cleanup — 11 HUMAN-UAT items + 2 deferred Phase 14 XCUITest flakes cleared before new surface area
- Ratify A1–A4 (connector SDK + Claude Code + OpenAI + AgentRegistry) into GSD with production hardening; wire iOS Agent Discovery to live AgentRegistry
- A5 Python Connector SDK — feature parity with TypeScript SDK; unit + E2E tests
- A6 MCP Bridge connector — any MCP server becomes an AINM agent participant; MCP tools surface as AINM tool calls
- OpenAI Responses API upgrade + connector auth hardening — real tool support; signed-handshake connector identity; OAuth-vs-API-key documented for Claude Code
- Compound memory groundwork — connectors push memory context back to server, positioning v3.4 for full compound memory aggregation/export

**Key context:**
- ~60-70% of the connector work exists on disk (`packages/connector/`, `packages/connector-claude-code/`, `packages/connector-openai/`) from development outside GSD. This milestone ratifies, tests, documents, and hardens — it is not greenfield
- Phase numbering continues from v3.2 (ended at phase 14); v3.3 starts at phase 15
- Strategic frame: after v3.3 the iOS app can message any agent. v3.4 layers compound memory aggregation on top of this plumbing

## Last Shipped: v3.2 iOS Reference Client (2026-04-19)

A native SwiftUI iOS app that connects to the AINM Rust server and delivers a reference-quality messaging experience where humans and agents are equal thread participants. All protocol features render as first-class native UI: streaming, interactive blocks, tool calls, memory context, agent discovery, sub-threading, `ainm://` deep links, @-mentions.

**Delivered across 5 phases (10-14):**
- Phase 10 Foundation — iOS Xcode scaffold, Swift 6 strict concurrency, 7 Codable model types, Ed25519 crypto (CryptoKit + Keychain), RFC 8785 JCS canonicalizer, GRDB persistence, WebSocket AINMClient, onboarding
- Phase 11 Core Messaging UI — ContentBlock discriminated union, conversation list, message thread with rich blocks (text/code/image/file), composer, offline access
- Phase 12 Streaming & Interactive Blocks — StreamingManager actor, token-by-token cursor, stop generating, ButtonBlock / FormBlock / TableBlock / ApprovalBlock, ActionBlock response flow
- Phase 13 AI-Native Features — Tool call lifecycle indicators, @-mention autocomplete + rendering, Agent Card discovery, memory context sheet, archival fact search + delete, sub-thread branching, `ainm://` deep links
- Phase 14 UAT Automation Harness — XCUITest target, 6 scenario classes (10 methods), SeedLoader + fixtures, TestEventInjector, a11y identifier retrofit, `make uat` green runs, 13-HUMAN-UAT flipped to complete

### Out of Scope

- Android/web clients for v1 — iOS reference client only
- Payment/transaction layer — messaging first, commerce later
- Agent marketplace/store — discovery yes, marketplace no
- Voice/video messaging — text and rich content only for v1
- Self-hosting documentation/tooling for v1 — protocol spec is open but we're not building easy deployment yet

## Context

### Current State (v3.2 iOS Reference Client shipped — 2026-04-19)

v3.2 iOS Reference Client complete — 5 phases, 24 plans, 34/34 v1 requirements satisfied. The native SwiftUI app connects to the AINM Rust server via WebSocket + JSON-RPC 2.0, renders every protocol feature (streaming, interactive blocks, tool calls, memory, agent discovery, sub-threads, deep links, @-mentions) as first-class native UI, persists locally via GRDB, signs messages with Ed25519 via CryptoKit, and ships with a hermetic XCUITest harness runnable via `make uat`.

Published artifacts across all milestones:
- **11 prose spec documents** covering identity, security, transport, content, threading, streaming, interactive UI, agent discovery, tool visibility, memory, and compatibility
- **12 JSON Schema 2020-12 files** with 52 formal type definitions
- **AsyncAPI 3.1.0** transport bindings with 25 operations across 9 channels
- **Rust CLI demo** (single-binary Axum server) proving the protocol end-to-end, now with `ainm-demo --uat` mode for test orchestration
- **Docusaurus site** for spec publication with GitHub Pages deployment
- **Native iOS reference client** (SwiftUI, Swift 6 strict concurrency, @Observable, GRDB, CryptoKit)
- **57/57 v1 spec requirements** (v3.1) + **28/28 federation requirements** (v3.2 federation scope, phases 7-9) + **34/34 iOS client requirements** (v3.2 iOS client, phases 10-14)

Tech stack: Rust 1.85+ (Axum, Tokio, ring, serde) server; Swift 6 / SwiftUI / GRDB.swift / CryptoKit iOS client; ~80K+ LOC across Rust, Swift, Markdown, JSON Schema, YAML, TypeScript.

### Outstanding Tech Debt (from v3.2 audit — to address in v3.3 or follow-up)

- **Phase 10 HUMAN-UAT** (5 items): live WebSocket handshake against running server, cross-platform crypto round-trip (Swift↔Rust), bidirectional message signing, Keychain keypair persistence across app launches, CI simulator-name assumption
- **Phase 11 HUMAN-UAT** (4 items, status: partial): offline read access, real-time list updates, agent identity badge visual distinctness, multi-block message rendering — all functionally working but not human-verified
- **Phase 14 deferred** (2 items): `DeepLinkUITests.test_deepLink_missingMessage_showsNotFoundToast` XCTSkipped (onAppear timing race); `SubThreadBranchUITests` reply-count badge soft-asserted (ForEach refresh timing). Both manually verified working; tracked for Phase 15 or v3.3 cleanup.

### The Landscape (March 2026)

The agent communication ecosystem is fragmented across layers:
- **MCP** (Anthropic/AAIF): Agent-to-tool integration. 97M+ monthly SDK downloads. The standard.
- **A2A** (Google/LF): Agent-to-agent task delegation. 50+ enterprise partners. Enterprise-focused.
- **A2UI** (Google): Declarative JSON spec for agents to render native UI. Has SwiftUI renderer.
- **AG-UI** (CopilotKit): Agent-user interaction runtime. 9K+ GitHub stars.
- **AMP**: Purpose-built agent messaging protocol but very early (v0.1.2), tiny community.
- **Agenium**: Closest competitor — live product attempting agent-native messaging. Early stage.

**The gap**: Nobody has built a cohesive, consumer-grade messaging experience that integrates these layers where humans and agents are equal thread participants.

### Research Completed

- `research/bottlenecks-current-messaging-apps.md` — Detailed analysis of Telegram, Slack, Discord, WhatsApp limitations for agent communication
- `research/landscape-protocols-projects.md` — Comprehensive survey of 15+ protocols, 10+ frameworks, academic papers, and competing products

### Key Design Influences

- **AMP**: Email-like addressing, Ed25519 signatures, federation model
- **A2A**: Agent Cards for capability discovery
- **A2UI**: Declarative UI-as-data (not code) — agents reference client component catalog
- **Letta/MemGPT**: Hierarchical memory (core/conversational/archival) for context management
- **MCP**: JSON-RPC simplicity, tool integration pattern
- **AG-UI**: Streaming event model for real-time agent updates

### Known Tech Debt

- Demo-level gaps: ainm.initialized handshake enforcement is implemented but not all edge cases tested
- ChartBlock listed in CONT-03 but deliberately excluded — may revisit
- Key rotation, sub-thread creation, @-mentions, store-and-forward not exercised in demo (spec complete)

## Constraints

- **Protocol-first**: Spec must be complete and implementable before building the iOS app
- **Open standard**: Protocol spec and reference implementations are fully open source
- **Federation**: No single point of control — any server can participate in the network
- **Transport**: Needs research — must work well for native iOS, real-time streaming, and server-to-server federation
- **Security**: Cryptographic identity and message signing are non-negotiable from day one

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Design protocol from scratch (not fork AMP) | AMP is v0.1.2, covers ~30% of vision, adopting would mean rewriting anyway. Better to design from first principles informed by all research. | Confirmed — Phase 1 delivered 5 original specs |
| Federated architecture | Agents shouldn't be locked to one provider. Like email, anyone should be able to run a server. | Confirmed — transport.md defines HTTP federation relay |
| Email-like identity (`name@provider`) | Familiar, works naturally with federation, human-readable. | Confirmed — identity.md specifies format + resolution |
| Protocol spec before iOS app | Protocol is the strategic asset. App is the showcase, not the product. | Complete — 11 prose specs + 12 JSON schemas + AsyncAPI + Docusaurus site |
| v1 = published spec + working demo | Validate the protocol works before investing in polished iOS app. | ✓ Complete — Rust CLI demo proves protocol end-to-end |
| Rich interactive agent UI | Agents need more than text — forms, approvals, tables, code. A2UI's "UI as data" model is the right approach. | ✓ Confirmed — interactive.md defines declarative component catalog |
| Three-tier memory model | Agents need structured context, not raw message dumps. Letta/MemGPT-inspired tiers. | ✓ Confirmed — memory.md defines active/conversation/archival tiers |
| A2A-compatible Agent Cards | Capability discovery must interop with Google A2A ecosystem while extending for AINM features. | ✓ Confirmed — agent-card.md uses AINM extension namespace |
| JSON-RPC 2.0 over WebSocket | Simpler than gRPC, works everywhere, debuggable. MCP alignment. | ✓ Confirmed — demo proves full lifecycle |
| Ed25519 signing with RFC 8785 JCS | Deterministic signing, misuse-resistant, compact signatures. | ✓ Confirmed — all demo messages signed and verified |
| One-level sub-threading only | Prevents Matrix-style complexity. Keep it simple for v1. | ✓ Confirmed — spec and demo validate approach |
| GRDB-direct hermetic test seed (v3.2 P14) | Full-protocol seeding would couple tests to server lifecycle. Direct DB writes keep UAT fast + deterministic; AINMClient still connects for live events. | ✓ Confirmed — `make uat` produces green runs with 10 tests in < 2 min |
| In-app TestEventInjector behind #if DEBUG (v3.2 P14) | iOS XCUITest runner cannot invoke `Process()`; event scripts run in-app and exercise real AINMClient callback pipeline. | ✓ Confirmed — scenario 1 tool-call lifecycle + scenario 4 memory push both assert real UI rendering |
| `-UITestDeepLink` launch arg vs `simctl openurl` (v3.2 P14) | Original CONTEXT.md decision was simctl-primary; runtime research showed Process() unavailable and simctl timing unreliable on iOS 18+ sim. Pivoted to launch-arg primary with simctl as belt-and-suspenders. | ✓ Evolved — same `DeepLinkHandler.parse` code path exercised |
| Deterministic Ed25519 keypair from fixed 32-byte seed (v3.2 P14) | `Curve25519.Signing.PrivateKey(rawRepresentation:)` accepts deterministic bytes; CryptoKit signatures still use randomized nonces so tests assert sign+verify round-trip, not byte equality. | ✓ Confirmed — `uat-user@localhost` pubkey stable across runs |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd:transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-04-18 — v3.3 Universal Agent Client milestone started*
