# Bottlenecks and Limitations of Current Messaging Apps for AI Agent Communication

> Research compiled March 2026. Covers Telegram, Slack, Discord, WhatsApp, and iMessage.

---

## 1. Context Limitations

### Current State

Every mainstream messaging platform treats messages as isolated, flat units of text with minimal metadata. None of them have a native concept of "conversation context" the way an LLM consumes it.

| Platform | Message Length Limit | Threading Model | Metadata Support |
|----------|---------------------|-----------------|------------------|
| Telegram | 4,096 UTF-8 characters | Reply-to chains (no true threads) | None on messages |
| Slack | ~4,000 chars (section blocks: 3,000; total payload: ~40KB undocumented) | Native threads (parent + replies) | Message metadata (event_type + event_payload, invisible to users) |
| Discord | 2,000 chars (embeds: 6,000 chars across all fields) | Threads (auto-archive after 3 days of inactivity) | None on messages |
| WhatsApp | 4,096 characters (Business API) | Reply-to references only | Template metadata (header/body/footer/buttons ENUM) |
| iMessage | ~20,000 characters (no official limit published) | No threading | None; no developer API exists |

### Why This Is a Problem for AI Agents

**Message length limits force chunking.** An LLM generating a detailed analysis, code review, or structured response regularly exceeds 4,096 characters. Telegram and WhatsApp force the agent to split output into multiple messages, which:
- Breaks the atomicity of a response (users see partial output, may act on incomplete information)
- Creates race conditions in group chats where other messages interleave
- Makes it impossible to reference "the response" as a single unit

**No structured metadata means no memory layer.** When an agent operates through Telegram, every conversation turn is just a text string. There is no place to attach:
- Conversation state (what step of a workflow the user is on)
- Session identifiers (linking messages across a multi-turn interaction)
- Tool-call results (structured JSON from API calls the agent made)
- Confidence scores, source citations, or reasoning traces

Slack's message metadata feature is the closest thing to a solution, allowing apps to attach invisible structured JSON (event_type + event_payload) to messages. But it's designed for app-to-app signaling, not for persisting agent context. There are no documented size limits on the payload, no schema validation enforcement, and no way for the receiving agent to query historical metadata without paginating through `conversations.history` with `include_all_metadata=true`.

**No state persistence across sessions.** All platforms are stateless from the bot's perspective:
- Telegram: Updates are stored for max 24 hours, then discarded. The Bot API exposes no message history or search endpoint. If your bot process restarts, you lose all context.
- Discord: Archived threads are not synced via the gateway. Threads auto-archive after inactivity. The bot must maintain its own external state store (e.g., Redis) for conversation memory.
- WhatsApp: The 24-hour conversation window is a hard session boundary. After 24 hours of user inactivity, the agent cannot initiate contact without a pre-approved template message. Webhook events are retried for 7 days on failure, then permanently discarded -- no event log, replay API, or dead-letter queue exists.
- Slack: Thread history is retrievable via API, but there's no built-in mechanism for a bot to "resume" a conversation with context. The bot must re-fetch and re-parse the entire thread on every interaction.

### What an Ideal Solution Would Look Like

- **First-class conversation context objects** -- a persistent, queryable data structure attached to each conversation that agents can read/write structured state into (current workflow step, accumulated tool results, user preferences, session variables)
- **Unlimited or very large message payloads** with structured sections (text body, metadata JSON, artifacts, tool-call logs) rather than a single flat text field with a tight character limit
- **Server-side context persistence** so agents don't need external databases just to remember what happened 5 messages ago
- **Context windows as a protocol concept** -- the messaging platform understands that an agent may need the last N messages plus associated metadata, and can serve that as a single structured payload rather than requiring pagination through a chat history API

---

## 2. Interaction Model Mismatch

### Current State

Every messaging platform was designed around a human-to-human interaction model: one person types, the other reads, thinks, and types back. The entire UX -- typing indicators, read receipts, rate limits, response timeouts -- assumes human-speed participants.

**Rate limits assume human throughput:**

| Platform | Rate Limit | Designed For |
|----------|-----------|--------------|
| Telegram | ~30 messages/second globally; ~20 messages/minute per chat | Human conversation pace |
| Slack | 1 message/second per channel; burst tolerance | Team communication |
| Discord | 50 requests/second global; 3-second interaction response timeout | Gaming community chat |
| WhatsApp | Tiered: 250-100K unique users/24h; message ordering not guaranteed | Business-customer 1:1 |

**No streaming support for response generation:**
- Telegram: `sendMessage` is fire-and-forget. To simulate streaming, an agent must call `editMessageText` repeatedly, which counts against rate limits and produces a janky visual experience with no typing cursor.
- Slack: Similar pattern -- post a message, then call `chat.update` repeatedly. Rate-limited at Tier 3 (50+/minute), but in practice the 1/second channel rate limit applies. Developers have built streaming by updating a message every ~500ms, but it feels sluggish and creates API overhead.
- Discord: The 3-second interaction response deadline means an agent must `defer` immediately on any slash command, then follow up within 15 minutes. No token-by-token streaming. Editing a deferred response to simulate streaming counts against per-route rate limits.
- WhatsApp: No message editing at all. Once sent, a message cannot be updated. Streaming is fundamentally impossible.

**No tool-use visibility:**
When an agent invokes tools (API calls, database queries, code execution), the user sees nothing. There is no standard way to show:
- "Agent is searching your order history..."
- "Agent is running code..."
- "Agent is waiting for external API response..."

The best approximation is Telegram's `sendChatAction` (typing indicator), Slack's typing-like message updates, or Discord's deferred response. None of these convey *what* the agent is doing -- just that it's "thinking."

**No structured input beyond basic UI primitives:**

| Platform | Input Mechanisms Available |
|----------|--------------------------|
| Telegram | Inline keyboards (callback_data limited to 64 bytes!), reply keyboards, force-reply |
| Slack | Block Kit (50 blocks/message, 100 in modals), modals with form inputs, shortcuts |
| Discord | Slash commands (25 options per autocomplete), buttons (25/message), select menus, modals (no autocomplete in modals) |
| WhatsApp | Quick reply buttons (max 3), list messages (max 10 options), CTA buttons (max 2) |
| iMessage | No developer input mechanisms |

Telegram's 64-byte callback_data limit is particularly crippling. An inline keyboard button can only carry 64 bytes of payload -- not even enough for a UUID. Developers must maintain server-side lookup tables mapping short hashes to actual data, adding complexity and state management overhead for what should be a simple button press.

### Why This Is a Problem for AI Agents

**Agents operate at machine speed, platforms throttle to human speed.** An agent that needs to:
1. Receive a user message
2. Make 3 tool calls
3. Synthesize results
4. Respond with a structured answer

...might complete this in 2-3 seconds. But if each tool call produces a status update, the agent hits rate limits immediately. On Discord, the agent has exactly 3 seconds to respond or must defer -- forcing every non-trivial agent interaction into a deferred flow.

**The "thinking" gap destroys user experience.** When ChatGPT streams tokens, users feel engaged. When a Telegram bot goes silent for 8 seconds and then dumps a wall of text, users wonder if it crashed. The platform provides no affordance for "agent is working on a complex task."

**Structured input/output is a nightmare.** An agent that needs the user to provide 5 parameters for an API call (date range, filter criteria, output format, etc.) must either:
- Parse free-text natural language (error-prone)
- Send a sequence of individual questions (slow, tedious)
- Use platform-specific UI (Slack modals are decent; Telegram inline keyboards are crippled by the 64-byte limit; WhatsApp maxes out at 3 buttons)

### What an Ideal Solution Would Look Like

- **Native streaming protocol** -- the platform renders tokens as they arrive, with a proper streaming cursor, not repeated message edits
- **Tool-use status indicators** -- a structured "agent activity" feed showing what the agent is doing in real-time (tool calls, intermediate results, progress bars)
- **Dynamic form generation** -- agents can programmatically construct input forms with validation, conditional fields, and rich data types, rendered natively by the client
- **Agent-appropriate rate limits** -- separate rate tiers for agent-class participants that reflect machine throughput, not human typing speed
- **Structured response containers** -- messages that contain multiple typed sections (text, code, data tables, interactive elements) in a single atomic response

---

## 3. Multi-Agent Coordination

### Current State

No mainstream messaging platform has any concept of "multiple AI agents collaborating in a conversation." The platform sees bots as individual participants identical to users, with no affordances for agent-to-agent interaction.

**No agent identity system:**
- Telegram: A bot is identified by a token. There's a "verified" badge for organizations, but no way to advertise capabilities, supported tools, or interaction protocols. Users and other bots see only the bot's name and profile picture.
- Slack: Apps have scopes and permissions, but no capability advertisement. One Slack app cannot discover what another Slack app can do.
- Discord: Bots are OAuth2 applications. No capability registry. No way for Bot A to know Bot B exists in a server, let alone what it can do.
- WhatsApp: Business accounts have a profile, but AI chatbots were explicitly banned on WhatsApp as of January 15, 2026 ("General Purpose AI Chatbots" prohibition). The platform actively fights against multi-agent patterns.

**No handoff protocols:**
When a specialized agent (e.g., a coding assistant) needs to hand off to another agent (e.g., a deployment bot), there is no standard mechanism for:
- Signaling the handoff ("I'm done, Agent B should take over")
- Transferring conversation context to the next agent
- Defining the scope of the handoff (what Agent B is authorized to do)
- Returning control back to Agent A when Agent B finishes

Developers build this with custom routing layers, tagging conventions, or dedicated orchestrator bots -- all brittle, platform-specific hacks.

**No capability discovery:**
The emerging A2A protocol addresses this with "Agent Cards" -- JSON documents at `/.well-known/agent-card.json` that describe an agent's identity, capabilities (skills), authentication requirements, and supported interaction modalities. But this is an internet protocol, not integrated into any messaging platform. Inside Telegram or Discord, there is no equivalent.

### Why This Is a Problem for AI Agents

**The real-world pattern is multi-agent, not single-agent.** Production agent architectures increasingly involve:
- A router/orchestrator agent that triages requests
- Specialized agents for different domains (billing, technical support, coding, scheduling)
- Quality assurance agents that review other agents' outputs

Current messaging platforms force all of this into a single bot account, or require elaborate workarounds (multiple bot accounts in the same chat, custom @-mention routing, middleware orchestration layers).

**The N-squared connectivity problem emerges immediately.** If you have 5 agents in a Slack workspace, each needing to coordinate with others, you need custom integration code for every pair. There's no discovery mechanism, no shared task model, no way to observe which agent is working on what.

**Context transfer between agents is lossy.** When Agent A @-mentions Agent B in a Slack thread, Agent B receives a text notification. It doesn't receive Agent A's internal state, tool-call history, confidence assessments, or reasoning trace. The handoff is limited to whatever fits in a Slack message (4,000 characters of flat text).

### What an Ideal Solution Would Look Like

- **Agent registry with capability cards** -- every agent in a conversation advertises its skills, supported input/output types, and interaction protocols (similar to A2A Agent Cards, but native to the messaging platform)
- **Structured handoff protocol** -- a first-class "transfer" message type that carries conversation context, accumulated state, and scope definition from one agent to another
- **Task lifecycle management** -- the platform tracks multi-agent tasks (pending, in-progress, blocked, completed) and provides visibility to all participants
- **Role-based agent permissions** -- agents can be granted specific capabilities (read-only, can-execute-tools, can-delegate-to-other-agents) at the conversation level
- **Shared workspace** -- a persistent data space that multiple agents can read/write, with conflict resolution and access control

---

## 4. Rich Content and Tool Use

### Current State

AI agents increasingly produce structured outputs: code with syntax highlighting, data tables, interactive charts, forms, file artifacts, and multi-step workflows. Current messaging platforms support only a fraction of this.

**Code rendering:**
- Telegram: Monospace text via backtick markdown. No syntax highlighting. No line numbers. No copy button.
- Slack: Code blocks with basic syntax highlighting. Limited to the message length constraints (~4,000 chars). No collapsible sections.
- Discord: Code blocks with syntax highlighting via highlight.js. Limited to 2,000 characters per message. No line numbers (frequently requested feature, never implemented).
- WhatsApp: No code formatting at all. Backticks render as inline monospace only.

**Structured data / tables:**
- Telegram: No table support. Must use monospace text and manual alignment.
- Slack: Block Kit has no table block. Must use section blocks with fields (2 columns max) or mrkdwn-formatted pseudo-tables.
- Discord: No table support in messages. Embeds provide structured fields but are limited to 25 fields of 256-char names and 1024-char values.
- WhatsApp: No structured data rendering.

**Interactive elements:**
- Telegram: Inline keyboards are the primary interactive element, but callback_data is capped at 64 bytes per button. Web Apps (mini-apps) exist but require external hosting and a separate development stack.
- Slack: The richest ecosystem -- Block Kit supports buttons, selects, date pickers, multi-select, overflow menus, radio buttons, checkboxes. But limited to 50 blocks per message, and the undocumented payload size limit (~13KB in practice) means complex interactive layouts fail silently.
- Discord: Buttons (5 action rows, 25 buttons total), select menus, modals. But modals lack autocomplete, and components can only be sent in response to interactions. No way to send persistent interactive dashboards.
- WhatsApp: 3 quick reply buttons or 10-item list menus. No custom UI elements. Interactive "Flows" exist but require separate approval and are designed for forms, not dynamic agent output.

**Artifacts and file generation:**
- All platforms support file uploads, but with no semantic context. An agent that generates a CSV, a PDF, and an image has no way to group these as a single "artifact" with metadata explaining what each file is and how they relate.

### Why This Is a Problem for AI Agents

**Agent output exceeds platform rendering capabilities.** When an agent generates:
- A 200-line code file -- it must be split across multiple messages on every platform
- A comparison table -- it becomes an unreadable mess of monospace characters
- An interactive decision tree -- it's reduced to a linear sequence of button-reply pairs
- A dashboard with multiple data visualizations -- impossible to render natively

**Tool results need structured rendering, not text dumps.** When an agent calls an API and gets a JSON response, the ideal display is a formatted, collapsible data view -- not a raw JSON string pasted into a chat message. No platform supports this.

**The "artifact" concept doesn't exist.** Claude's artifacts (self-contained code, documents, interactive components) have no analog in any messaging platform. The closest is Slack Canvas, which allows persistent documents in a workspace, but canvases are not message-level artifacts -- they're separate surfaces that can't be inline within a conversation flow.

### What an Ideal Solution Would Look Like

- **Rich content blocks** -- messages composed of typed, renderable blocks: code (with syntax highlighting, line numbers, diffs), tables (sortable, filterable), charts, Markdown/LaTeX, interactive forms
- **Artifact containers** -- a grouped bundle of related outputs (files, code, data) with metadata and navigation, inline within the conversation
- **Live/interactive elements** -- components that can update in real-time (dashboards, progress trackers, approval workflows) without message-edit hacks
- **Extensible renderers** -- agents can define custom rendering schemas (similar to Slack's Block Kit, but with much richer primitives and no 50-block limit), and the client renders them natively

---

## 5. Protocol Limitations

### Current State

The underlying APIs of each platform were designed for simple message passing, not for the complex interaction patterns agents require.

**Telegram Bot API:**
- Strictly request-response. Bot sends HTTP request, gets response. No server-push except webhooks.
- Webhooks limited to ports 443, 80, 88, 8443. Max 100 concurrent connections (100,000 on local API server).
- No streaming endpoint. `editMessageText` is the only way to update content.
- No structured payloads on messages -- just text, media, and inline keyboards.
- No conversation/session abstraction -- the bot must track state externally.
- `getUpdates` long-polling returns max 100 updates at a time.

**Slack API:**
- Events API: webhook-based, 30,000 deliveries per workspace per app per 60 minutes. Good for moderate volume, inadequate for high-throughput agent scenarios.
- RTM (Real-Time Messaging) API: WebSocket-based, but messages capped at 16KB and the connection only supports 1 URL request per minute. Legacy, being deprecated.
- Message metadata provides structured invisible payloads, but with undocumented size limits.
- No streaming message creation endpoint. Must post then repeatedly update.
- Block Kit payload size: documented at 50 blocks, but real limit is ~13KB total payload, discovered only through runtime errors.

**Discord API:**
- Global rate limit: 50 requests/second. Individual route limits vary and are communicated via response headers.
- Interaction response deadline: 3 seconds initial, 15 minutes for follow-ups.
- Gateway (WebSocket) for real-time events, but archived threads not synced.
- MESSAGE_CONTENT privileged intent required to read message content -- apps verified for 100+ servers must apply for approval.
- No streaming. Deferred responses + follow-up edits only.

**WhatsApp Business API (Cloud API):**
- 24-hour conversation window: agent can only send free-form messages within 24 hours of user's last message. Outside this window, only pre-approved templates.
- Template approval: 24+ hours for review. Templates rejected for vague policy violations with minimal explanation.
- Message ordering not guaranteed. Sequential messages may arrive out of order.
- No message editing. Once sent, cannot be modified.
- No webhook replay or event log. Events retried for 7 days then permanently lost.
- Marketing messages to US numbers (+1) temporarily paused since April 2025.
- AI chatbots explicitly banned as of January 2026.

**iMessage / Apple Messages for Business:**
- No public API for iMessage. Apple Messages for Business requires:
  - A legal entity (no indie developers)
  - A Messaging Service Provider (MSP) middleware -- cannot connect directly
  - Only customer-initiated conversations (business cannot initiate)
  - Human operator required -- "shouldn't be fully automated"
  - No group chat support
  - One business chat account per company
  - Platform can revoke access at any time with no appeals process

### Why This Is a Problem for AI Agents

**No protocol-level support for agent patterns.** These APIs lack:
- **Streaming**: Essential for real-time response generation. Every platform requires the hack of post-then-edit-repeatedly.
- **Structured payloads**: Agent-to-agent communication needs typed, schema-validated data exchange. Only Slack's metadata comes close, but it's undocumented and limited.
- **State management**: No platform maintains conversation state server-side for bots. Every agent implementation requires an external database.
- **Capability negotiation**: No way for an agent to declare what it can do, what input it accepts, or what output it produces. Compare this to A2A's Agent Cards or MCP's tool/resource declarations.
- **Task lifecycle**: No concept of a task that progresses through states (created, in-progress, blocked, completed) with multiple agents contributing.
- **Transactional messaging**: No way to send a group of messages atomically. On WhatsApp, message ordering isn't even guaranteed.

**The emerging protocols (MCP, A2A) solve these problems but don't integrate with messaging platforms.** MCP (Model Context Protocol) standardizes agent-to-tool communication with typed tools, resources, and prompts. A2A (Agent-to-Agent) standardizes agent-to-agent communication with Agent Cards, task management, and streaming. But both are HTTP/gRPC protocols designed for server-to-server communication -- not for end-user messaging platforms. There is no bridge between "agent talks to agent via A2A" and "user sees the result in Telegram."

### What an Ideal Solution Would Look Like

- **Native streaming protocol** -- WebSocket or SSE-based message streams where tokens arrive individually and the client renders them progressively
- **Typed message schemas** -- messages carry a declared schema (JSON Schema, protobuf) so receiving agents can parse them without guessing the format
- **Built-in state machine** -- the platform tracks conversation state, available actions, and transition conditions, accessible to all participants
- **A2A/MCP-compatible transport** -- the messaging protocol natively supports Agent Cards, capability discovery, and tool declarations, allowing agents to negotiate capabilities at the protocol level
- **Transactional message groups** -- send multiple messages as an atomic unit, with guaranteed ordering and all-or-nothing delivery
- **Event sourcing** -- complete, replayable history of all events in a conversation, not just messages but state changes, tool invocations, handoffs, and metadata updates

---

## 6. Authentication and Trust

### Current State

Current messaging platforms have rudimentary identity systems designed for humans, not agents.

**How agent identity works today:**

| Platform | Identity Mechanism | Verification | Trust Model |
|----------|-------------------|--------------|-------------|
| Telegram | Bot token (HTTP Bearer) | "Verified" badge for organizations (applied for separately) | Anyone with the token IS the bot. Token compromise = full impersonation. |
| Slack | OAuth 2.0 app tokens (bot token, user token) | Slack app directory review (for distribution) | Workspace admin installs apps. Scoped permissions. |
| Discord | OAuth2 application with bot user | Verification required for bots in 100+ servers | Server admin authorizes. Privileged intents require approval. |
| WhatsApp | Cloud API access token + Business verification | Meta Business verification process | Phone number-based identity. Business profile. |
| iMessage | MSP-mediated, Apple Business registration | Apple approval process | Apple controls all access. Can revoke at any time. |

**What's missing for agent trust:**

1. **No capability attestation.** A user inviting a "coding assistant" bot to their Slack workspace has no way to verify what the bot can actually do, what data it accesses, or what external services it calls. The OAuth scopes tell you what Slack APIs it can call, not what the agent does with the data.

2. **No runtime permission boundaries.** Once a Telegram bot has its token, it can do anything the Bot API allows -- there's no per-conversation or per-user permission scoping. A bot invited to a group chat has the same capabilities for all users in that group.

3. **No agent-to-agent trust chain.** When Agent A delegates a task to Agent B (via @-mention, thread, or custom routing), there's no cryptographic proof that Agent A authorized the delegation, no scope limitation on what Agent B can do with the delegated authority, and no audit trail of the delegation chain.

4. **No verifiable computation.** When an agent says "I checked your account balance and it's $1,234," the user has no way to verify that the agent actually called the banking API, received that response, and is reporting it accurately. The message is indistinguishable from a hallucinated response.

5. **Token-based identity is fragile.** Telegram's entire trust model is a single HTTP bearer token. Slack's bot tokens, if leaked, grant full API access. There's no mutual TLS, no hardware-backed identity, no token rotation enforcement, and no anomaly detection ("this bot usually sends 10 messages/day, today it sent 10,000").

**The A2A protocol's approach to this is more mature:**
- Agent Cards include `securitySchemes` defining authentication requirements (Bearer, OAuth2, API Key)
- TLS certificate validation for server identity
- But: authentication schemes are declarative, not enforced. The protocol says "this agent requires OAuth2" but doesn't mandate a specific credential exchange flow. Implementation variability is high.

### Why This Is a Problem for AI Agents

**Agents act on behalf of users with broad authority.** Unlike a human typing messages, an agent may:
- Access external APIs with the user's credentials
- Execute code
- Make purchases
- Send messages to other people
- Modify data

The messaging platform has no visibility into these actions and no mechanism to constrain them. The trust boundary is binary: either the bot is in the chat or it isn't.

**Multi-agent scenarios amplify the trust problem.** If Agent A can delegate to Agent B, and Agent B can delegate to Agent C, the original user has no visibility into (or control over) the delegation chain. There's no "this agent was authorized by User X to perform Task Y with Scope Z" that can be cryptographically verified by Agent C.

**No accountability or audit trail.** When something goes wrong (agent sends wrong message, accesses wrong data, makes unauthorized API call), the messaging platform's logs show only "bot sent message." There's no structured audit trail of the agent's reasoning, tool calls, or decision-making process.

### What an Ideal Solution Would Look Like

- **Agent identity certificates** -- cryptographic identity bound to the agent's deployment, signed by the agent provider, verifiable by the platform and by other agents
- **Capability manifests** -- machine-readable declarations of what the agent can do, what data it accesses, what external services it calls, signed and verifiable
- **Scoped, delegatable permissions** -- fine-grained permission tokens that specify exactly what an agent can do in a specific conversation, with user-controlled delegation chains
- **Verifiable computation proofs** -- agents can attach cryptographic evidence of tool calls (API request/response hashes, execution logs) to their messages, allowing users and other agents to verify claims
- **Behavioral monitoring** -- the platform tracks agent behavior patterns and flags anomalies (sudden spike in message volume, accessing unusual data, deviating from declared capabilities)
- **Audit trail as a first-class primitive** -- every agent action (message sent, tool called, delegation made, data accessed) is logged in a structured, queryable, tamper-evident audit trail visible to authorized participants

---

## Summary: The Gap Between Current Platforms and Agent-Native Messaging

| Dimension | Current State | What Agents Need |
|-----------|--------------|------------------|
| **Context** | Flat text messages, no metadata, no state persistence, 2K-4K char limits | Structured context objects, unlimited payloads, server-side state, queryable history |
| **Interaction Model** | Human-speed rate limits, no streaming, basic UI primitives | Machine-speed throughput, native streaming, dynamic forms, tool-use visibility |
| **Multi-Agent** | Bots as isolated participants, no discovery, no handoffs | Agent registries, capability cards, structured handoffs, shared workspaces |
| **Rich Content** | Text + basic formatting + file attachments | Typed content blocks, artifact containers, live interactive elements |
| **Protocol** | Request-response HTTP, no structured payloads, no state machine | Streaming transport, typed schemas, built-in state, A2A/MCP compatibility |
| **Trust** | Token-based identity, binary permissions, no audit trail | Cryptographic identity, scoped permissions, verifiable computation, behavioral monitoring |

The fundamental insight is that current messaging platforms are **display layers for human communication** being force-fitted as **execution environments for autonomous agents**. Every workaround -- external state stores, message-edit streaming, callback-data hash tables, custom routing middleware -- is evidence of this architectural mismatch.

The emerging agent protocols (MCP for tool use, A2A for agent coordination) address the server-side communication patterns but leave the human-facing messaging layer completely untouched. There is a missing layer: **an agent-native messaging platform** that bridges the gap between agent-to-agent protocols and human-agent interaction, with first-class support for context, streaming, structured I/O, multi-agent coordination, rich content, and verifiable trust.
