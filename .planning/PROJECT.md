# AI-Native Messaging Protocol & App

## What This Is

An open, federated messaging protocol designed from the ground up for AI agent communication — where humans and agents are equal participants in conversations. The protocol defines how agents declare capabilities, exchange messages, stream responses, and render rich interactive UI. A native iOS app serves as the reference client, showcasing what messaging looks like when agents are first-class citizens, not bolted-on bots.

## Core Value

The protocol must enable mixed human-agent threads where any participant (human or agent) can seamlessly join, communicate, and coordinate — with the same quality of experience regardless of who's a person and who's an AI.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Open protocol specification for AI-native messaging
- [ ] Federated architecture — anyone can run a server, messages route across servers
- [ ] Email-like identity model for both humans and agents (`name@provider`)
- [ ] Agent capability discovery (Agent Cards / capability manifests)
- [ ] Mixed human-agent threads — both as equal participants
- [ ] Flat conversations by default with optional sub-threading
- [ ] Rich interactive content from agents (forms, buttons, charts, code blocks, approvals)
- [ ] Streaming support — agents can stream responses in real-time
- [ ] Context/memory management at the protocol level (active context, conversation history, archival)
- [ ] Security foundations — cryptographic message signing, identity verification, anti-prompt-injection
- [ ] Working demo that proves the protocol works end-to-end
- [ ] Native iOS app (SwiftUI) as reference client

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
| Design protocol from scratch (not fork AMP) | AMP is v0.1.2, covers ~30% of vision, adopting would mean rewriting anyway. Better to design from first principles informed by all research. | — Pending |
| Federated architecture | Agents shouldn't be locked to one provider. Like email, anyone should be able to run a server. | — Pending |
| Email-like identity (`name@provider`) | Familiar, works naturally with federation, human-readable. | — Pending |
| Protocol spec before iOS app | Protocol is the strategic asset. App is the showcase, not the product. | — Pending |
| v1 = published spec + working demo | Validate the protocol works before investing in polished iOS app. | — Pending |
| Rich interactive agent UI | Agents need more than text — forms, approvals, charts, code. A2UI's "UI as data" model is the right approach. | — Pending |

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
*Last updated: 2026-03-27 after initialization*
