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

### Active

- [ ] Native iOS app (SwiftUI) as reference client — full AINM protocol implementation

## Current Milestone: v3.3 iOS Reference Client

**Goal:** A native SwiftUI iOS app that connects to the AINM Rust server and delivers a messaging experience built for agent interaction — rendering all protocol features (streaming, interactive blocks, tool calls, memory) as first-class native UI.

**Target features:**
- Full AINM protocol client — WebSocket connection to Rust server with JSON-RPC 2.0
- Conversation list and message thread views with mixed human-agent participation
- Native rendering of all content block types (text, code, image, file)
- Interactive block rendering — forms, buttons, tables, approvals, action blocks
- Real-time streaming with token-by-token rendering
- Tool call transparency — visible started/progress/completed events
- Memory context display (active context frames, conversation history)
- Agent discovery — browse agent capabilities via Agent Cards
- Simplified identity flow architected for full name@provider + Ed25519 Keychain storage
- Ed25519 message signing via Apple CryptoKit
- Local persistence via GRDB.swift for offline message access
- iOS 18+ / SwiftUI / Swift Concurrency / MVVM + Repository

### Out of Scope

- Android/web clients for v1 — iOS reference client only
- Payment/transaction layer — messaging first, commerce later
- Agent marketplace/store — discovery yes, marketplace no
- Voice/video messaging — text and rich content only for v1
- Self-hosting documentation/tooling for v1 — protocol spec is open but we're not building easy deployment yet

## Context

### Current State (Phase 10 complete — 2026-03-30)

Phase 10 (Foundation) complete — iOS Xcode project scaffold with Swift 6 strict concurrency, all protocol Codable models, Ed25519 crypto (CryptoKit + Keychain), GRDB persistence layer, WebSocket networking with AINM protocol client, onboarding UI, and GitHub Actions CI. Build verified on iOS 26.2 simulator.

The v1 protocol specification is complete and published:
- **11 prose spec documents** covering identity, security, transport, content, threading, streaming, interactive UI, agent discovery, tool visibility, memory, and compatibility
- **12 JSON Schema 2020-12 files** with 52 formal type definitions
- **AsyncAPI 3.1.0** transport bindings with 25 operations across 9 channels
- **Rust CLI demo** (single-binary Axum server) proving the protocol end-to-end
- **Docusaurus site** for spec publication with GitHub Pages deployment
- **57/57 v1 spec requirements** satisfied across 6 phases (v3.1)
- **28/28 federation requirements** satisfied across 3 phases (v3.2) — mTLS relay, replay protection, persistent offline queues, backfill, AI-native features across federation, comprehensive Docker Compose demo

Tech stack: Rust 1.85+ (Axum, Tokio, ring, serde), ~73K LOC across Rust, Markdown, JSON Schema, YAML, TypeScript.

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
*Last updated: 2026-03-30 after Phase 10 Foundation completion*
