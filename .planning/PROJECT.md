# AI-Native Messaging Protocol & App

## What This Is

An open, federated messaging protocol designed from the ground up for AI agent communication — where humans and agents are equal participants in conversations. The protocol defines how agents declare capabilities, exchange messages, stream responses, and render rich interactive UI. A native iOS app serves as the reference client, showcasing what messaging looks like when agents are first-class citizens, not bolted-on bots.

## Core Value

The protocol must enable mixed human-agent threads where any participant (human or agent) can seamlessly join, communicate, and coordinate — with the same quality of experience regardless of who's a person and who's an AI.

## Requirements

### Validated

- [x] Open protocol specification for AI-native messaging — *Validated in Phase 1: protocol-kernel (identity, security, content, transport, threading specs)*
- [x] Email-like identity model for both humans and agents (`name@provider`) — *Validated in Phase 1: identity.md*
- [x] Mixed human-agent threads — both as equal participants — *Validated in Phase 1: threading.md*
- [x] Flat conversations by default with optional sub-threading — *Validated in Phase 1: threading.md*
- [x] Security foundations — cryptographic message signing, identity verification, anti-prompt-injection — *Validated in Phase 1: security.md*

### Active

- [ ] Federated architecture — anyone can run a server, messages route across servers
- [ ] Native iOS app (SwiftUI) as reference client

### Validated in Phase 3

- [x] Working demo that proves the protocol works end-to-end — *Validated in Phase 3: Rust CLI demo (Axum WebSocket server, Ed25519 signing, streaming, rich content, agent discovery)*
- [x] Published formal protocol specification (JSON Schema 2020-12 + AsyncAPI 3.1.0) — *Validated in Phase 3: 12 schema files, asyncapi.yaml, Docusaurus site*
- [x] Protocol versioning with backwards compatibility rules — *Validated in Phase 3: CHANGELOG.md, VERSION, SemVer policy*

### Validated in Phase 2

- [x] Agent capability discovery (Agent Cards / capability manifests) — *Validated in Phase 2: agent-card.md (A2A-compatible with AINM extensions)*
- [x] Rich interactive content from agents (forms, buttons, tables, approvals) — *Validated in Phase 2: interactive.md (declarative component model)*
- [x] Streaming support — agents can stream responses in real-time — *Validated in Phase 2: streaming.md (start/delta/end lifecycle, parallel multi-block)*
- [x] Context/memory management at the protocol level (active context, conversation history, archival) — *Validated in Phase 2: memory.md (three-tier model, context frames)*
- [x] Tool call transparency — visible to all thread participants — *Validated in Phase 2: tool-visibility.md*
- [x] MCP and A2A ecosystem compatibility bridges — *Validated in Phase 2: compatibility.md*

### Out of Scope

- Android/web clients for v1 — iOS reference client only
- Payment/transaction layer — messaging first, commerce later
- Agent marketplace/store — discovery yes, marketplace no
- Voice/video messaging — text and rich content only for v1
- Self-hosting documentation/tooling for v1 — protocol spec is open but we're not building easy deployment yet

## Context

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

- `research/bottlenecks-current-messaging-apps.md` — Detailed analysis of Telegram, Slack, Discord, WhatsApp limitations for agent communication (rate limits, no streaming, no capability discovery, no rich content)
- `research/landscape-protocols-projects.md` — Comprehensive survey of 15+ protocols, 10+ frameworks, academic papers, and competing products

### Key Design Influences

- **AMP**: Email-like addressing, Ed25519 signatures, federation model
- **A2A**: Agent Cards for capability discovery
- **A2UI**: Declarative UI-as-data (not code) — agents reference client component catalog
- **Letta/MemGPT**: Hierarchical memory (core/conversational/archival) for context management
- **MCP**: JSON-RPC simplicity, tool integration pattern
- **AG-UI**: Streaming event model for real-time agent updates

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
| v1 = published spec + working demo | Validate the protocol works before investing in polished iOS app. | Complete — Rust CLI demo proves protocol end-to-end |
| Rich interactive agent UI | Agents need more than text — forms, approvals, tables, code. A2UI's "UI as data" model is the right approach. | Confirmed — interactive.md defines declarative component catalog |
| Three-tier memory model | Agents need structured context, not raw message dumps. Letta/MemGPT-inspired tiers. | Confirmed — memory.md defines active/conversation/archival tiers |
| A2A-compatible Agent Cards | Capability discovery must interop with Google A2A ecosystem while extending for AINM features. | Confirmed — agent-card.md uses AINM extension namespace |

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
*Last updated: 2026-03-28 after Phase 4 (protocol-conformance-polish) completion — all 12 audit findings resolved, demo exercises full protocol surface*
