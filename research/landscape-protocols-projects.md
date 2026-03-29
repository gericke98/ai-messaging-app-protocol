# AI-Native Messaging: Landscape Research Report

## The Emerging Protocol Stack

The landscape is converging toward a layered stack:

```
+--------------------------------------------------+
|  A2UI / MCP Apps / Open-JSON-UI                  |  <- Agent-to-User Interface
|  (How agents present UI to humans)               |
+--------------------------------------------------+
|  AG-UI                                           |  <- Agent-User Interaction Runtime
|  (Bi-directional state/event streaming)          |
+--------------------------------------------------+
|  A2A (+ merged ACP)                              |  <- Agent-to-Agent Communication
|  (Task delegation, capability discovery)         |
+--------------------------------------------------+
|  AMP / ANP                                       |  <- Agent Messaging / Identity
|  (Federated messaging, decentralized identity)   |
+--------------------------------------------------+
|  MCP                                             |  <- Agent-to-Tool Integration
|  (Tool invocation, data access)                  |
+--------------------------------------------------+
|  ANS / AID / Agent Cards                         |  <- Agent Discovery
|  (Finding agents, verifying identity)            |
+--------------------------------------------------+
|  AAIF (Linux Foundation)                         |  <- Governance
+--------------------------------------------------+
```

---

## 1. OPEN PROTOCOLS & STANDARDS

### 1.1 Model Context Protocol (MCP) — Agent-to-Tool Layer

- **What**: Open protocol (Anthropic, Nov 2024) for connecting LLMs to external tools. JSON-RPC 2.0 over HTTP.
- **Status**: Donated to AAIF under Linux Foundation (Dec 2025). 97M+ monthly SDK downloads, 10,000+ servers. Supported by Claude, ChatGPT, Cursor, Gemini, VS Code, Copilot.
- **MCP Apps Extension (SEP-1865)**: Released Jan 2026 — jointly by Anthropic and OpenAI. Interactive UI via sandboxed HTML iframes within chat windows.
- **Relevance**: Universal standard for agent tool access. Must support. MCP Apps extension proves interactive UI in messaging is a validated need.
- **Gaps**: Not designed for agent-to-agent communication. No peer-to-peer messaging. No agent identity/discovery.
- **Links**: [Spec](https://modelcontextprotocol.io/specification/2025-11-25), [GitHub](https://github.com/modelcontextprotocol/modelcontextprotocol)

### 1.2 Agent-to-Agent Protocol (A2A) — Google

- **What**: Open protocol for agent interoperability. HTTP, SSE, JSON-RPC. Enterprise-grade auth. Under Linux Foundation.
- **Core concepts**: Agent Cards (JSON capability descriptors), Task lifecycle management, Context sharing, UX negotiation.
- **Version**: v0.3 with gRPC support, signed security cards, extended Python SDK.
- **Backed by**: 50+ partners (Atlassian, Salesforce, SAP, ServiceNow, PayPal, LangChain).
- **Relevance**: Leading standard for agent-to-agent communication. Critical for multi-agent messaging.
- **Gaps**: Designed for enterprise task delegation, not conversational messaging. No message threading or conversation persistence.
- **Links**: [Blog](https://developers.googleblog.com/en/a2a-a-new-era-of-agent-interoperability/), [GitHub](https://github.com/a2aproject/A2A)

### 1.3 Agent Communication Protocol (ACP) — IBM (merged into A2A)

- **What**: IBM's protocol via BeeAI. REST-native, multi-part MIME, async streaming, multimodal.
- **Status**: Merged with A2A under Linux Foundation (Aug 2025).
- **Relevance**: Validates A2A as convergence point. ACP's multimodal streaming concepts may influence A2A evolution.

### 1.4 Agent Network Protocol (ANP) — Decentralized

- **What**: "HTTP for the Agentic Web." W3C DID for authentication. Three layers: Identity, Meta-Protocol, Application.
- **W3C involvement**: Founded W3C AI Agent Protocol Community Group.
- **Relevance**: Addresses decentralized identity that centralized protocols miss.
- **Gaps**: Less adoption than MCP/A2A, complex, early.
- **Links**: [Website](https://agent-network-protocol.com/), [GitHub](https://github.com/agent-network-protocol/AgentNetworkProtocol)

### 1.5 Agent Messaging Protocol (AMP) — 23blocks

- **What**: Open-source protocol (Apache 2.0) specifically for AI agent messaging. Federated, cryptographically secure, local-first.
- **Address format**: `agent-name@tenant.provider` (email-like).
- **Security**: Ed25519 signatures, canonical signing, trust annotations against prompt injection, replay protection, TLS 1.2+.
- **Delivery**: REST + WebSocket + Relay Queue (for offline agents).
- **Spec**: 11 formal specification documents.
- **MOST DIRECTLY RELEVANT PROTOCOL**: Purpose-built for agent messaging (not task delegation like A2A, not tool access like MCP). Addresses identity, federation, security, message routing.
- **Gaps**: Very early (v0.1.2), small community, limited implementations.
- **Links**: [GitHub](https://github.com/agentmessaging/protocol), [Website](https://agentmessaging.org/)

### 1.6 Agora Protocol — Oxford Research

- **What**: Meta-protocol where agents negotiate communication using plain-text Protocol Documents.
- **Innovation**: Standardized routines for frequent comms, natural language for rare comms, LLM-written routines for middle ground.
- **Relevance**: Dynamic protocol negotiation is interesting for adaptive agent communication.
- **Links**: [Paper](https://arxiv.org/html/2410.11905v1), [GitHub](https://github.com/agora-protocol/paper-demo)

### 1.7 AITP (Agent Interaction & Transaction Protocol) — NEAR Foundation

- **What**: Agent interactions across trust boundaries with blockchain-backed identity.
- **Relevance**: Interesting for economic transactions but likely too blockchain-heavy for general messaging.
- **Links**: [aitp.dev](https://aitp.dev)

---

## 2. UI/INTERFACE PROTOCOLS — Agent-to-User Layer

### 2.1 AG-UI (Agent-User Interaction Protocol) — CopilotKit

- **What**: Bi-directional protocol between user-facing apps and agentic backends. Event/state protocol for UI updates, agent state, user interactions.
- **Positioning**: "While MCP handles context and A2A handles agent coordination, AG-UI defines the interaction layer between user, application, and agent."
- **Status**: 9,000+ GitHub stars, 2M+ agent-user interactions/week. Integrated with Oracle, Google.
- **HIGHLY RELEVANT**: Fills the gap between backend agent protocols and frontend UX.
- **Gaps**: Focused on embedding agents in existing apps, not building native messaging from scratch.
- **Links**: [Docs](https://docs.ag-ui.com/), [GitHub](https://github.com/ag-ui-protocol/ag-ui)

### 2.2 A2UI (Agent-to-User Interface) — Google

- **What**: Declarative JSON spec allowing agents to describe rich native interfaces. Agents send JSONL streams; clients render with own components.
- **Security**: UI as data, not code — agents reference client-controlled component catalog, preventing UI injection.
- **Implementations**: Renderers for Angular, Flutter, Lit, React, **SwiftUI**, Markdown.
- **Version**: v0.8 stable, v0.9 draft.
- **HIGHLY RELEVANT**: Solves how agents "speak UI" safely and cross-platform. SwiftUI renderer exists.
- **Links**: [Website](https://a2ui.org/), [GitHub](https://github.com/google/A2UI)

---

## 3. GOVERNANCE & DISCOVERY

### 3.1 Agentic AI Foundation (AAIF)

- **Formed**: Dec 9, 2025 under Linux Foundation.
- **Platinum members**: AWS, Anthropic, Block, Bloomberg, Cloudflare, Google, Microsoft, OpenAI.
- **Projects**: MCP, goose, AGENTS.md, A2A.
- **Relevance**: Where standards converge. Any serious protocol should align with AAIF.

### 3.2 Agent Discovery Standards

- **Agent Name Service (ANS)**: OWASP. DNS-inspired, PKI-based. Format: `a2a://textProcessor.DocumentTranslation.AcmeCorp.v2.1.hipaa`.
- **Agent Identity & Discovery (AID)**: DNS TXT record-based. IETF draft.
- **Agent Cards (A2A)**: JSON capability descriptors.

---

## 4. MULTI-AGENT FRAMEWORKS

### 4.1 Letta (formerly MemGPT)

- **What**: Stateful agents with memory hierarchy: core (in-context), conversational (recent), archival (long-term).
- **Innovation**: Agents actively manage own memory. "LLM as Operating System."
- **HIGHLY RELEVANT**: Memory architecture maps directly to messaging needs (current thread / recent convos / full history).
- **Links**: [Letta](https://letta.com/), [GitHub](https://github.com/letta-ai/letta)

### 4.2 Microsoft Agent Framework (AutoGen successor)

- **Pattern**: GroupChat — multiple agents in shared conversation, selector determines who speaks.
- **Supports**: MCP, A2A, AG-UI. MIT licensed.
- **Relevance**: GroupChat pattern is closest to "messaging app" model.

### 4.3 ElizaOS

- **What**: TypeScript framework with character/persona system. One event pipeline for Discord, Telegram, X, HTTP.
- **Relevance**: Character system (JSON persona, behavior, knowledge) relevant for defining agent personalities.

---

## 5. PRODUCTS & STARTUPS

### 5.1 Agenium — CLOSEST COMPETITOR

- **What**: Live product at chat.agenium.net. "DNS for the agent web" — agents get real addresses and communication channels.
- **Status**: Early access, open source, building in public.
- **Relevance**: Attempting to build exactly an "AI-native messaging app" where agents are first-class citizens.
- **Links**: [Agenium](https://agenium.net/)

### 5.2 Manus AI (Meta-owned)

- **What**: Personal AI agents deployed into existing messaging apps. Launched on Telegram (Feb 2026).
- **Approach**: Parasitic on existing platforms rather than building native.

### 5.3 NanoClaw

- **What**: Lightweight open-source personal AI agent connecting to WhatsApp, Telegram, Slack, Discord, Signal.
- **Links**: [nanoclaw.dev](https://nanoclaw.dev/)

---

## 6. UX/DESIGN PATTERNS

### Key Agentic Design Patterns (agentic-design.ai)

| Pattern | Description |
|---------|-------------|
| **Human-in-the-Loop** | Human judgment at critical decisions |
| **Progressive Disclosure** | Gradually revealing agent capabilities/reasoning |
| **Confidence Visualization** | Showing AI uncertainty levels |
| **Mixed-Initiative Interface** | Seamless control switching human <-> agent |
| **Agent Status & Activity** | Real-time visibility of agent thinking/working |
| **Generative UI** | Agent dynamically generates UI components |
| **Context Window Management UI** | Visual patterns for managing LLM context limits |

### Structured vs. Unstructured Messaging (Academic Consensus)

Hybrid approach:
- Structured routines for frequent/predictable communication
- Natural language for rare/novel communication
- LLM-generated routines for middle ground

---

## 7. GAPS & OPPORTUNITIES

1. **No unified messaging experience**: Nobody has built a cohesive messaging app integrating MCP + A2A + AG-UI where humans and agents are equal participants.

2. **Agent identity in messaging is unsolved**: AMP is closest but early (v0.1.2).

3. **Conversation memory across agents is fragmented**: Letta has best architecture but per-agent, not per-conversation.

4. **Mixed human-agent threads don't exist**: Every protocol handles either human-to-agent OR agent-to-agent. None handles both elegantly.

5. **Rich messaging primitives for agents missing**: Agents need structured data, approvals, forms, code execution results — not just text.

6. **No "iMessage for the agent era"**: Consumer experience doesn't exist in polished form.

---

## 8. KEY ACADEMIC REFERENCES

| Paper | Link | Key Contribution |
|-------|------|-----------------|
| Survey of Agent Interoperability Protocols | [arXiv:2505.02279](https://arxiv.org/abs/2505.02279) | Phased adoption roadmap |
| A Survey of AI Agent Protocols | [arXiv:2504.16736](https://arxiv.org/pdf/2504.16736) | Broad protocol landscape |
| Security Threat Modeling for AI-Agent Protocols | [arXiv:2602.11327](https://arxiv.org/html/2602.11327) | Security analysis |
| Agora: Scalable Communication for LLM Networks | [arXiv:2410.11905](https://arxiv.org/html/2410.11905v1) | Meta-protocol |
| Agent Name Service (ANS) | [arXiv:2505.10609](https://arxiv.org/abs/2505.10609) | DNS-inspired agent discovery |
