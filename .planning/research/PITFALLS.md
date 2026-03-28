# Domain Pitfalls

**Domain:** Federated AI-native messaging protocol and reference iOS client
**Researched:** 2026-03-27

---

## Critical Pitfalls

Mistakes that cause rewrites, project failure, or years of accumulated technical debt.

---

### Pitfall 1: The Overspecification Trap (Specifying Everything Before Proving Anything)

**What goes wrong:** The team designs a comprehensive protocol specification covering every conceivable use case -- federation, security, UI rendering, memory management, agent discovery, versioning -- before building a single working implementation. The spec becomes a multi-hundred-page document. When implementation begins, dozens of theoretical decisions turn out to be wrong because they were never tested against real agent behavior.

**Why it happens:** Protocol-first thinking (which this project explicitly adopts) creates a gravitational pull toward completeness. The desire to "get the spec right" before coding feels responsible but produces the opposite outcome. Committee-driven protocol design historically suffers from this: "specifications that are so general and leave so many choices that implementer workshops are needed to agree on what to build" (InformIT Protocol Design Folklore).

**Real examples:**
- XMPP's XEP explosion: extensibility through Extension Protocols spawned incompatible optional features, with different servers and clients implementing different subsets, producing inconsistent cross-client experiences. The spec was comprehensive; adoption was fragmented.
- AMP (Agent Messaging Protocol) has 11 formal specification documents at v0.1.2 with a tiny community and limited implementations -- specification outpacing adoption.
- OPC UA's over-customization: the standard became so comprehensive that many implementations only support subsets, making interoperability unreliable.

**Consequences:**
- 6-12 months of spec work before any working code
- Spec decisions that are wrong-by-default because untested
- Community can't contribute because the spec is too complex to implement
- Competitor ships a working (if imperfect) product while you're still writing RFCs

**Prevention:**
- Define a "protocol kernel" -- the absolute minimum set of messages and behaviors needed for a single-server demo: send message, receive message, agent identity, streaming response. Spec that, implement it, validate it.
- Use a "living spec" approach: the spec is authoritative only for what has been implemented and tested. Unimplemented sections are explicitly marked as "draft/proposed."
- Follow MCP's model: date-based versioning (YYYY-MM-DD), only incrementing when backwards compatibility breaks. Keep the core small.
- Target: working demo within 4-6 weeks of starting implementation, not 4-6 months of spec writing.

**Detection (warning signs):**
- Spec documents exceeding 50 pages before any code exists
- Debates about edge cases in features nobody has tried to build yet
- "We need to figure out X before we can start" -- where X is 3+ layers removed from core messaging

**Phase relevance:** Phase 1 (Protocol Foundation). The spec-vs-implementation balance must be resolved from day one. Ship a working kernel, then expand.

---

### Pitfall 2: State Resolution Complexity in Federation (The Matrix Decade)

**What goes wrong:** Federated systems must resolve conflicts when two servers have different views of the same conversation state. This problem is deceptively simple in theory ("just merge the states") and devastatingly complex in practice. Matrix has spent 8+ years on state resolution and is still fixing critical bugs.

**Why it happens:** Federation means multiple authoritative sources of truth that can diverge due to network partitions, server failures, or malicious actors. Every federated protocol eventually discovers that distributed consensus in an open network is a fundamentally hard problem.

**Real examples:**
- Matrix's State Resolution v1 (2014-2018): Had bugs allowing "state resets" where room state rolled back to earlier points in time. Attackers could maliciously revert room state.
- Matrix's State Resolution v2 (2018-2025): Shipped with confidence in 2018. By 2025, "Project Hydra" uncovered two high-severity protocol vulnerabilities causing state resets, done under embargo. The algorithm's output was described as "counter-intuitive."
- Matrix rooms where "what makes a room 'large' is not message history, user count, or connections to homeservers, but rather how long the state resolution algorithm takes" -- joining large federated rooms can take 10-60 minutes.
- Different Matrix server implementations (Synapse, Dendrite, Conduit) handle JSON formatting and cryptographic signatures inconsistently, causing state resets when they federate.

**Consequences:**
- Messages appear out of order or disappear entirely
- Conversation permissions revert (kicked users reappear, moderators lose power)
- Performance degrades non-linearly with room size
- Years of engineering effort on a problem that has no simple solution
- User trust destroyed by "messages I sent yesterday are gone"

**Prevention:**
- Start with single-server only. Do not federate in Phase 1. Get the data model right for one server first.
- When adding federation, use a simpler model than Matrix's DAG-based state resolution. Consider:
  - Conversation-owner-authoritative model: one server is authoritative for each conversation, others are replicas. Simpler but less resilient.
  - CRDT-based state: Conflict-free Replicated Data Types for conversation metadata (membership, settings). More complex upfront but mathematically proven to converge.
  - Append-only message logs with separate state snapshots: messages never conflict (append-only), conversation state uses last-writer-wins with vector clocks.
- Build a state resolution test harness before the algorithm ships. Matrix's TARDIS tool (Time Agnostic Room DAG Inspection Service) was built after years of bugs -- build yours first.
- Design for the failure mode: when state does diverge, how does the user experience it? "Some messages may be reordered" is acceptable. "Your messages disappeared" is not.

**Detection:**
- Two servers showing different conversation membership
- Messages appearing in different order on different clients
- Join/leave operations taking progressively longer
- Any bug report containing the phrase "my message disappeared"

**Phase relevance:** Phase 3+ (Federation). Explicitly defer this to after single-server protocol is proven. When you do tackle it, allocate 3x the time you think it needs.

---

### Pitfall 3: Prompt Injection Through Federated Messages (The AI-Specific Attack Surface)

**What goes wrong:** In a federated messaging system where AI agents are first-class participants, every incoming message from a federated server is an untrusted input to an LLM. Malicious actors craft messages containing hidden instructions that cause receiving agents to leak data, take unauthorized actions, or propagate the attack to other agents. This is fundamentally different from human-to-human messaging security -- the messages are the attack vector AND the execution environment.

**Why it happens:** LLMs process message content as instructions. In a federated network, any server can send messages to any agent. This means any participant in the network can attempt to inject instructions into any agent's context window. The attack surface scales with the number of federated servers.

**Real examples:**
- OpenAI acknowledges "prompt injection, much like scams and social engineering on the web, is unlikely to ever be fully 'solved'" (Dec 2025).
- OWASP 2025 Top 10 for LLM Applications: prompt injection is the #1 critical vulnerability, appearing in 73%+ of production AI deployments.
- "Second-order" prompt injection in multi-agent systems: a low-privilege agent receives a malformed request that tricks it into asking a higher-privilege agent to perform unauthorized actions -- "an LLM-powered insider threat."
- A September 2025 state-sponsored attack used Claude as an automated intrusion engine, affecting ~30 organizations, with AI carrying out 80-90% of the operation.
- MCP tool poisoning: malicious instructions embedded in tool metadata (not user inputs) that manipulate agent behavior through protocol-level content.
- Meta-analysis of 78 studies (2021-2026): attack success rates against state-of-the-art defenses exceed 85% when adaptive attack strategies are employed.

**Consequences:**
- Agent data exfiltration across federation boundaries
- Cascading agent compromise (one poisoned message propagates through agent-to-agent handoffs)
- Unauthorized actions taken on behalf of users
- Complete loss of trust in the platform if agents are regularly manipulated
- Legal liability for actions taken by compromised agents

**Prevention:**
- Architectural separation of message content from agent instructions. MIT Technology Review's principle: "Rules fail at the prompt, succeed at the boundary." Do not rely on prompt-level defenses alone.
- Implement trust tiers for message sources:
  - Tier 1 (local server): messages from agents on the same server
  - Tier 2 (trusted federation): messages from servers with verified identity and history
  - Tier 3 (unknown federation): messages from unverified servers -- presented to agents with explicit untrusted markers and restricted context influence
- Cryptographic message signing (Ed25519, as in AMP) for every message. Agents can verify sender identity before processing content.
- Content sanitization layer between receiving a federated message and feeding it to an LLM. Strip known injection patterns, limit special characters, enforce message schemas.
- Agent sandboxing: agents should have least-privilege access. A "calendar agent" should not be able to access "banking agent" tools even if a prompt injection tells it to.
- Human-in-the-loop for high-stakes actions: the protocol should support "approval required" message types that pause agent execution and require human confirmation.
- Rate limiting and anomaly detection: flag agents that suddenly change behavior (sending unusual message types, accessing unusual resources, generating responses inconsistent with their capability card).

**Detection:**
- Agent responses that don't match their declared capabilities
- Sudden changes in agent behavior patterns
- Messages containing encoded or obfuscated instruction-like content
- Cross-agent privilege escalation attempts
- Agents referencing information they shouldn't have access to

**Phase relevance:** Phase 1-2 (Protocol Foundation and Security). Message signing and trust tiers must be in the protocol from day one. Content sanitization can iterate, but the architectural separation (boundary-level security, not prompt-level) must be designed in, not bolted on.

---

### Pitfall 4: The Adoption Chicken-and-Egg Death Spiral

**What goes wrong:** The protocol needs implementations to prove it works. Implementations need users to justify development effort. Users need a network of agents/people to message. Agents need the protocol to be adopted before they integrate it. Nobody moves first. The protocol remains an academic exercise.

**Why it happens:** Open protocols inherently suffer from network effects problems. Unlike a product (where you ship it and people use it), a protocol requires multiple independent parties to implement the same specification. Email took a decade to become universal. HTTP won partly because Tim Berners-Lee shipped a working browser and server together.

**Real examples:**
- AMP (Agent Messaging Protocol): well-designed spec, 11 documents, Apache 2.0 license, but v0.1.2 with a tiny community. The spec exists but the network doesn't.
- XMPP lost to proprietary solutions (WhatsApp, iMessage) despite being technically superior, because the federation complexity and inconsistent UX drove users to simpler, centralized alternatives.
- ActivityPub/Mastodon: despite being the most successful federated social protocol, remains <1% of social media usage. Content moderation, search, and onboarding friction are persistent barriers.
- Matrix: after 10+ years, still a niche protocol. The complexity barrier for running a homeserver keeps the network small relative to centralized alternatives.

**Consequences:**
- Months/years of protocol development with no users
- Spec becomes theoretical, disconnected from real usage patterns
- Competitors with inferior but functional products capture the market
- Contributors lose interest and move on

**Prevention:**
- Ship the iOS reference client as the FIRST implementation, not a theoretical exercise. The app IS the adoption vehicle. People don't adopt protocols; they adopt products.
- Build a hosted server (your own) where early users can get accounts immediately. Do not require self-hosting for the initial user base. Federation is a protocol capability; it doesn't mean every user runs a server.
- Pre-build 3-5 useful agents that come with the platform: a general assistant, a coding helper, a calendar/scheduling agent, a knowledge base agent. The app must have day-one utility without requiring the user to connect external agents.
- Bridge to existing platforms: allow agents on your protocol to also operate in Telegram/Slack/Discord. This brings existing agent developers into the ecosystem without requiring them to abandon their current users.
- Target developers first: make it trivially easy to build an agent for the protocol. Excellent SDK, 5-minute quickstart, hosted sandbox for testing.
- Open the spec but control the reference implementation quality. HTTP succeeded because NCSA Mosaic was good, not because the spec was perfect.

**Detection:**
- Zero external implementations 3+ months after spec publication
- GitHub stars but no forks or PRs from external contributors
- "Interesting protocol" compliments with no "I'm building with it" follow-ups
- Spec discussions dominated by the core team with no external voices

**Phase relevance:** Every phase, but especially Phase 1 (demo) and Phase 4 (iOS app). The demo must be compelling enough that people want to build on it. The iOS app must be good enough that people want to use it daily.

---

## Moderate Pitfalls

Mistakes that cause significant delays or technical debt but are recoverable.

---

### Pitfall 5: iOS Background Execution and Real-Time Messaging Mismatch

**What goes wrong:** The protocol design assumes persistent WebSocket connections for real-time streaming, but iOS aggressively suspends background apps and throttles network activity. Users see delayed messages, failed streaming, and agents that appear unresponsive -- not because the protocol is broken but because iOS won't let the app maintain the connection.

**Why it happens:** iOS uses a "tightly controlled background execution model where applications are generally suspended shortly after being minimized." WebSocket connections are terminated when the app goes to background. Silent push notifications are "opportunistic rather than guaranteed" -- Apple decides delivery based on battery level, device conditions, and recent activity. You cannot send more than 2-3 background push notifications per hour without Apple throttling you.

**Real examples:**
- Every iOS messaging app (including Signal, Telegram) must solve this. The standard pattern: APNs push notification wakes the app, app fetches messages from server, displays notification. The WebSocket is only maintained while the app is in the foreground.
- Streaming (showing tokens as they arrive) only works while the app is foregrounded. Background agent responses must be delivered as complete messages via push.
- APNs payload limit is 4KB -- you cannot deliver rich agent content through push alone.
- iOS apps get approximately 30 seconds of background execution time when woken by a push notification.

**Prevention:**
- Design the protocol with two delivery modes from the start:
  1. **Foreground/streaming mode**: WebSocket for real-time token-by-token streaming, live UI updates, tool-use visibility
  2. **Background/notification mode**: APNs push with minimal metadata (message ID, sender, preview text), app fetches full message content on launch
- Never embed full message content in push payloads. Push should contain only identifiers; the app fetches the complete message (including rich content, agent artifacts) when opened.
- Design agent responses to be both streamable AND deliverable as complete units. An agent response that only makes sense as a stream (e.g., intermediate tool-call status updates) must also have a final-state representation for background delivery.
- Implement message queuing server-side: when the recipient app is backgrounded, the server queues messages and delivers them as a batch when the app reconnects.
- Use Notification Service Extension (NSE) for processing push notifications before display -- this gives you a few seconds to fetch and decrypt message content for the notification preview.
- Test on actual iOS devices with aggressive battery settings from day one. The simulator does not accurately model background throttling.

**Detection:**
- "Messages arrive late" bug reports
- Agent streaming works in foreground but agent appears silent when app is minimized
- Push notifications showing generic "New message" instead of content previews
- Battery drain complaints (sign of too-aggressive background activity)

**Phase relevance:** Phase 4 (iOS Reference Client). This must be designed into the protocol transport layer, not patched onto the app later. The protocol must explicitly support both real-time and store-and-forward delivery.

---

### Pitfall 6: Rich Content Rendering Fragmentation (The HTML Email Problem Revisited)

**What goes wrong:** The protocol defines rich content types (forms, charts, code blocks, approval workflows, data tables), but different clients render them inconsistently or not at all. The experience becomes unpredictable: an agent sends a beautifully structured approval form, but the receiving client renders it as raw JSON or a degraded text fallback. The protocol becomes the new HTML email -- technically capable, practically a mess.

**Why it happens:** Rich content in messaging requires agreement between the sender (agent) and the receiver (client) on how to render content. In a federated protocol, you cannot control what client software participants use. Every client will implement a different subset of rendering capabilities at different quality levels.

**Real examples:**
- HTML email: 30+ years later, email clients still render the same HTML differently. Outlook uses Word's rendering engine. Gmail strips most CSS. Complex layouts require extensive testing across clients.
- Slack Block Kit: documented at 50 blocks per message, but real limit is ~13KB payload, discovered only through runtime errors. Complex interactive layouts fail silently.
- XMPP's XEP fragmentation: different clients implement different subsets of extensions, producing inconsistent cross-client experiences.

**Prevention:**
- Adopt A2UI's "UI as data, not code" approach. Agents send declarative JSON describing UI intent; clients render using their own native components. This prevents code injection and enables graceful degradation.
- Define a mandatory minimum rendering set: every compliant client MUST render text, markdown, code blocks, images, and simple buttons. Everything else is optional with a required text fallback.
- Require every rich content message to include a `fallback` field: plain text representation that any client can display. This is not optional -- it's a protocol requirement.
- Use capability negotiation: agents discover what the receiving client can render (via capability cards or session negotiation) and adapt their output accordingly. An agent talking to a rich client sends forms; talking to a text-only client sends structured text.
- Version the content schema independently from the protocol. Content types can evolve faster than the core protocol without breaking existing clients.
- Resist the urge to support "arbitrary HTML/JS" -- that path leads to security nightmares and rendering inconsistency. Constrain the component catalog.

**Detection:**
- Bug reports about content looking different on different clients
- Agents sending rich content that users can't interact with
- Developers requesting "just let us send HTML"
- Fallback text missing from rich content messages

**Phase relevance:** Phase 2 (Rich Content / Agent UI). The fallback requirement and mandatory minimum rendering set must be in the spec before any rich content features ship.

---

### Pitfall 7: Context/Memory Explosion in Multi-Agent Conversations

**What goes wrong:** In a conversation with multiple agents, every agent needs context to respond intelligently. Naive implementations feed the entire conversation history to every agent on every turn. With 3 agents in a 100-message thread, you're making 300+ LLM calls with growing context windows. Costs spiral, latency increases, and agents start hallucinating because their context windows overflow with irrelevant information from other agents' interactions.

**Why it happens:** The instinct is "give the agent all the context so it can respond well." This works for a 2-party conversation. It breaks in multi-agent threads where different agents need different slices of context, and the total conversation grows faster than any single agent's context window can handle.

**Real examples:**
- "Without active management, conversations exceed context limits within relatively few turns" -- even single-agent conversations hit this with modern LLMs.
- "Not every agent needs every message. By implementing role-based filtering and tiered storage, token consumption can be reduced by 60-80%."
- Letta/MemGPT's hierarchical memory architecture exists specifically because flat context windows don't scale: core memory (always in context), conversational memory (recent turns), archival memory (searchable long-term storage).
- "Naive truncation strategies that simply drop the oldest messages often discard information still relevant to the conversation, creating jarring user experiences."

**Prevention:**
- Design context management into the protocol, not the application layer. The protocol should define:
  - **Context scope**: what messages/state an agent receives when it's @-mentioned or enters a conversation
  - **Context summary**: a protocol-level mechanism for summarizing conversation history before delivering it to an agent
  - **Context filtering**: agents declare what context they need (via capability cards), and the server filters accordingly
- Implement Letta-style tiered memory at the protocol level:
  - **Active context**: the current message + recent N messages relevant to this agent
  - **Conversation summary**: server-maintained summary of the full conversation, updated incrementally
  - **Archival**: full conversation history, searchable but not loaded by default
- Make context a server responsibility, not an agent responsibility. The server should manage what context each agent receives, not leave it to each agent to fetch and filter the entire history.
- Set explicit context budgets: the protocol can specify maximum context sizes, and the server enforces them by summarizing or truncating as needed.

**Detection:**
- LLM API costs increasing non-linearly with conversation length
- Agent response latency increasing as conversations grow
- Agents repeating questions or contradicting earlier statements
- "The agent forgot what I said 5 minutes ago" complaints

**Phase relevance:** Phase 2-3 (Agent Capabilities and Federation). Must be designed in the protocol before multi-agent conversations ship. Retrofitting context management onto an existing system is extremely painful.

---

### Pitfall 8: Federation Spam and Abuse Without Centralized Moderation

**What goes wrong:** In a federated network, any server can send messages to any other server. Malicious servers flood the network with spam, phishing, or abuse content. There's no central authority to ban them. Individual server operators must independently decide whom to block, leading to fragmented blocklists, moderation burden on volunteers, and inconsistent user experiences.

**Why it happens:** Federation's core value proposition ("no single point of control") is also its core vulnerability. The same openness that prevents censorship also prevents effective spam prevention. Email is the canonical example: SMTP was designed for a trusted network and has spent 25+ years retrofitting anti-spam measures (SPF, DKIM, DMARC) onto a protocol that wasn't designed for them.

**Real examples:**
- Email/SMTP: "55% of mail servers were open relays in 1998." SMTP was designed in ARPANET days where all participants were trusted. The subsequent anti-spam infrastructure (SPF, DKIM, DMARC, reputation systems, ML classifiers) is larger and more complex than the original email protocol.
- Mastodon/ActivityPub: "Poorly optimized ActivityPub implementations can cause unintentional DDoS attacks on other servers." Moderation tools are limited; moderators must screen content post-by-post. Defederation (cutting off an entire server) is the nuclear option used because finer-grained tools don't exist.
- Matrix: federation APIs exposed to untrusted networks create security vulnerabilities. Server operators must independently manage allowlists.
- Carnegie Endowment research (2025): "trust and safety in decentralized spaces can be challenging given platform design and resourcing limitations, with few tools available for volunteer-run servers to address harmful content in bulk."

**Prevention:**
- Build reputation and trust systems into the protocol from day one, not as afterthoughts:
  - **Server reputation scores**: based on uptime, message quality, abuse reports from peer servers
  - **Graduated trust**: new servers start with limited messaging rates, earning higher limits over time
  - **Shared blocklists**: protocol-level mechanism for servers to subscribe to and share abuse reports
- Learn from email's mistakes -- design anti-abuse at the protocol level:
  - Require cryptographic server identity (like DKIM for email, but mandatory not optional)
  - Include sender reputation metadata in the federation protocol
  - Support rate limiting at the protocol level (servers can declare and enforce message rate limits for federation partners)
- For AI-specific abuse: agent capability verification. An agent claiming to be a "customer support bot" but sending phishing links should trigger automated trust downgrades.
- Provide moderation tooling as part of the reference server implementation, not as an afterthought. If server operators can't moderate effectively, they'll either give up or defederate aggressively, fragmenting the network.

**Detection:**
- Servers receiving high volumes of messages from unknown federation partners
- Users reporting spam/abuse from federated contacts
- Server operators maintaining ad-hoc blocklists
- Network fragmenting into isolated clusters that don't federate with each other

**Phase relevance:** Phase 3 (Federation). Must be designed before federation launches. SMTP's experience proves that retrofitting anti-abuse onto an open federation protocol is a decade-long, never-ending battle.

---

### Pitfall 9: Protocol Versioning That Breaks the Network

**What goes wrong:** The protocol evolves, but version negotiation is poorly designed. Servers running v1.2 can't talk to servers running v1.1. Agents built against an older spec break when the server upgrades. Clients must update to keep working. The network fragments along version lines, or evolution stalls because nobody wants to risk breaking compatibility.

**Why it happens:** Protocol versioning is an afterthought in most initial designs. The focus is on getting the features right, not on how features will change over time. But in a federated system, you cannot force all participants to upgrade simultaneously. You will always have a mix of versions, and the protocol must handle this gracefully.

**Real examples:**
- Protobuf's forwards/backwards compatibility: adding a new required field means older clients silently ignore it, "causing the two parties to speak different languages without realizing the semantic mismatch."
- Header-based API versioning: "usually the one most poorly implemented when considering backwards compatibility."
- Matrix's room versioning: rooms have a version that determines the state resolution algorithm. Upgrading a room version requires creating a new room and migrating users -- a disruptive process.

**Prevention:**
- Use MCP's date-based versioning (YYYY-MM-DD) -- simple, unambiguous, only increment on breaking changes.
- Design for additive evolution: new features are always optional additions, never modifications to existing message types. Old clients ignore fields they don't understand; new clients handle missing optional fields gracefully.
- Require version negotiation in the federation handshake: when two servers connect, they declare their supported protocol versions and agree on the highest common version.
- Establish a clear deprecation policy: features are deprecated with 6-12 months warning, then removed in the next major version. Deprecated features must continue to work during the deprecation period.
- Test version compatibility as part of CI: every protocol change must pass tests against the previous version's implementation.
- Never have more than 2 major versions active simultaneously. If v3 ships, v1 support can be dropped (with the 6-12 month deprecation window).

**Detection:**
- Federation connections failing with "unsupported version" errors
- Agents that work on one server but fail on another
- Pressure to "just break compatibility" to add a needed feature
- Version negotiation adding noticeable latency to initial connections

**Phase relevance:** Phase 1 (Protocol Foundation). Version negotiation must be in the initial protocol design. It's one of the few things that's nearly impossible to add retroactively.

---

## Minor Pitfalls

Mistakes that cause friction but are fixable without major rework.

---

### Pitfall 10: Streaming UX That Ignores Network Reality

**What goes wrong:** The protocol supports token-by-token streaming for agent responses, but the implementation doesn't account for network variability. On poor connections, tokens arrive in bursts (10 at once, then silence, then 5 more), creating a janky, stuttering display. On strong connections with a slow agent (thinking, doing tool calls), the stream shows nothing for 5+ seconds, and users think the app froze.

**Prevention:**
- Implement client-side token buffering with smooth rendering. Buffer tokens for 50-100ms and render in smooth batches rather than individually.
- Define "agent status" events in the protocol distinct from content tokens: `thinking`, `tool_call_started(tool_name)`, `tool_call_completed`, `generating`. The client can show meaningful status UI during silent periods.
- Support partial message snapshots: periodically during streaming, the server should checkpoint the partial response so that if the client reconnects, it can resume from the checkpoint rather than restarting.
- Set protocol-level timeouts: if no token or status event arrives within 30 seconds, the client should show a "response may be delayed" indicator, not just a spinning cursor.

**Phase relevance:** Phase 2 (Streaming/Rich Content). Important for UX but fixable iteratively.

---

### Pitfall 11: Agent Card / Capability Discovery Becoming Stale

**What goes wrong:** Agents declare their capabilities via Agent Cards (JSON manifests), but capabilities change over time (new tools added, old ones removed, model upgrades). Clients and other agents cache the capability card and make decisions based on outdated information -- routing requests to agents that no longer support that capability, or not offering features the agent now supports.

**Prevention:**
- Include a `ttl` (time-to-live) field in Agent Cards. Clients must re-fetch after TTL expires.
- Support push-based capability updates: when an agent's capabilities change, the server broadcasts an update event to connected clients.
- Design the protocol so that capability mismatches fail gracefully: if an agent receives a request it can't handle, it should return a structured "capability not supported" response that the client can act on (e.g., showing a fallback or re-routing).
- Version the Agent Card schema itself, so new capability types can be added without breaking existing parsers.

**Phase relevance:** Phase 2 (Agent Discovery/Capabilities). Important but the impact is limited to UX friction, not protocol failure.

---

### Pitfall 12: Leaking Internal Protocol Details to Users

**What goes wrong:** Technical protocol concepts (federation, server identity, message signing, capability negotiation) leak into the user interface. Users see error messages like "federation handshake failed" or "message signature invalid" or "agent capability card not found." The app feels like a developer tool, not a messaging app.

**Prevention:**
- Establish a strict separation between protocol-layer errors and user-facing messages. Every protocol error maps to a human-friendly message.
- XMPP's lesson: "terminology driven by technical requirements exposes protocol complexities, while normal IM users don't want to know about technical terms." Name things for users, not engineers.
- Examples:
  - "Federation handshake failed" -> "Could not reach [contact's server]. They'll get your message when their server is back online."
  - "Message signature invalid" -> "This message couldn't be verified. It may have been tampered with."
  - "Agent capability card not found" -> "This agent is temporarily unavailable."
- Test with non-technical users early. If they ask "what does this mean?", it's a bug.

**Phase relevance:** Phase 4 (iOS Reference Client). UX polish, but the mapping layer should be designed during protocol work so error codes are structured and translatable.

---

### Pitfall 13: Ignoring Thread Model Complexity

**What goes wrong:** The project specifies "flat conversations by default with optional sub-threading" but doesn't deeply consider how multi-agent conversations actually flow. An agent responds to a user, another agent chimes in, the user asks a follow-up that could be directed at either agent, a third agent is @-mentioned and needs context from only part of the conversation. The flat model becomes confusing; the threading model becomes a design rabbit hole.

**Prevention:**
- Study how Slack, Discord, and email handle this (each differently, each with tradeoffs).
- Default to flat with explicit @-mentions for agent targeting. An unaddressed message in a multi-agent thread goes to all agents (they decide whether to respond based on relevance to their capabilities).
- Sub-threads should be agent-initiated ("Let me handle this in a side thread to keep the main conversation clean") rather than user-initiated. This reduces UX complexity.
- Design thread context inheritance: a sub-thread inherits the parent conversation's context summary but builds its own detailed context. When the sub-thread resolves, a summary is posted back to the parent.
- Don't try to solve this theoretically. Build the flat model, observe how real conversations break, then add threading for the specific failure modes observed.

**Phase relevance:** Phase 2 (Conversation Model). Important for multi-agent UX but can be iterated on after initial flat model ships.

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Protocol Foundation (Phase 1) | Overspecification trap (#1) | Define kernel spec, implement immediately, iterate |
| Protocol Foundation (Phase 1) | Versioning afterthought (#9) | Include version negotiation in initial design |
| Security (Phase 1-2) | Prompt injection via messages (#3) | Architectural boundary-level security from day one, not prompt-level rules |
| Agent Capabilities (Phase 2) | Context/memory explosion (#7) | Protocol-level context management, not app-level |
| Rich Content (Phase 2) | Rendering fragmentation (#6) | Mandatory fallbacks, minimum rendering set, capability negotiation |
| Federation (Phase 3) | State resolution complexity (#2) | Start single-server, use simpler model than Matrix's DAG |
| Federation (Phase 3) | Spam/abuse without moderation (#8) | Protocol-level trust and reputation systems from first federated message |
| iOS Client (Phase 4) | Background execution limits (#5) | Dual delivery mode (streaming + store-and-forward) designed into protocol |
| Adoption (All phases) | Chicken-and-egg death spiral (#4) | Ship the product (iOS app), not just the protocol |

---

## Sources

### Federated Protocol Experience
- [Matrix State Resolution Analysis](https://matrix.org/blog/2020/06/16/matrix-decomposition-an-independent-academic-analysis-of-matrix-state-resolution/) -- Academic analysis of state resolution
- [Project Hydra: Improving State Resolution](https://matrix.org/blog/2025/08/project-hydra-improving-state-res/) -- 2025 security vulnerabilities in Matrix federation
- [Matrix Holiday Special 2025](https://matrix.org/blog/2025/12/24/matrix-holiday-special/) -- State resolution improvements via TARDIS tooling
- [Easy XMPP: The Challenges](https://xmpp.org/2017/08/easy-xmpp-the-challenges/) -- XMPP federation complexity and UX failures
- [Carnegie Endowment: Navigating Defederation](https://carnegieendowment.org/research/2025/03/fediverse-social-media-internet-defederation) -- Trust and safety in decentralized platforms
- [Wire: Federation Types & Compliance Risks](https://wire.com/en/blog/federating-security-or-spam) -- Open federation vs. spam tradeoffs
- [SMTP History and Security Evolution](https://sectigostore.com/blog/a-history-of-email-and-smpt-the-evolution-of-email-security/) -- Email's 25-year anti-abuse retrofit

### AI Agent Security
- [Security Threat Modeling for AI-Agent Protocols](https://arxiv.org/html/2602.11327) -- Academic threat model for agent protocols
- [From Prompt Injections to Protocol Exploits](https://arxiv.org/abs/2506.23260) -- Comprehensive threat taxonomy for LLM-agent workflows
- [OWASP Top 10 for LLM Applications](https://genai.owasp.org/llmrisk/llm01-prompt-injection/) -- Prompt injection as #1 vulnerability
- [OpenAI on Prompt Injection](https://techcrunch.com/2025/12/22/openai-says-ai-browsers-may-always-be-vulnerable-to-prompt-injection-attacks/) -- Acknowledgment that prompt injection is unsolvable
- [AI Agent Attacks Q4 2025](https://www.esecurityplanet.com/artificial-intelligence/ai-agent-attacks-in-q4-2025-signal-new-risks-for-2026/) -- Real-world attack patterns
- [AI Agents and Identity Risks 2026](https://www.cyberark.com/resources/blog/ai-agents-and-identity-risks-how-security-will-shift-in-2026) -- Non-human identity threats
- [Palo Alto Unit 42: Agentic AI Threats](https://unit42.paloaltonetworks.com/agentic-ai-threats/) -- Cross-agent exploitation patterns

### iOS and Mobile Constraints
- [App Background Activity 2026: OS Restrictions Guide](https://alexrooter.com/os-background-limits/) -- iOS 18 background execution model
- [iOS Push Notifications Guide 2026](https://www.pushwoosh.com/blog/ios-push-notifications/) -- APNs constraints and best practices
- [WebSockets and iOS: Client-side Challenges](https://ably.com/topic/websockets-ios) -- Persistent connection limitations on iOS
- [Apple: Pushing Background Updates](https://developer.apple.com/documentation/usernotifications/pushing-background-updates-to-your-app) -- Official background push documentation

### Protocol Design and Versioning
- [Protocol Design Folklore](https://www.informit.com/articles/article.aspx?p=20482) -- Simplicity vs. flexibility vs. optimality tradeoffs
- [MCP Versioning](https://modelcontextprotocol.io/specification/versioning) -- Date-based versioning model
- [Protocol Versioning Blog](https://blog.io7m.com/2020/12/15/protocol-versioning.xhtml) -- Protocol versioning design patterns

### Context and Memory Management
- [Context Window Management Strategies](https://www.getmaxim.ai/articles/context-window-management-strategies-for-long-context-ai-agents-and-chatbots/) -- Scaling strategies for agent context
- [Letta: Memory Blocks](https://www.letta.com/blog/memory-blocks) -- Hierarchical memory architecture for agents
- [JetBrains: Efficient Context Management](https://blog.jetbrains.com/research/2025/12/efficient-context-management/) -- Research on smarter context management

### Rich Content and Agent UI
- [A2UI: Agent-Driven Interfaces](https://developers.googleblog.com/introducing-a2ui-an-open-project-for-agent-driven-interfaces/) -- Declarative UI-as-data protocol
- [Complete Guide to A2UI Protocol](https://dev.to/czmilo/the-complete-guide-to-a2ui-protocol-building-agent-driven-uis-with-googles-a2ui-in-2026-146p) -- Implementation guide

### Adoption and Network Effects
- [Six Strategies for Overcoming Chicken-and-Egg Problems](https://cdixon.org/2009/08/25/six-strategies-for-overcoming-chicken-and-egg-problems/) -- Classic network effects strategies
- [Content Moderation Challenges in Mastodon Growth](https://policyreview.info/articles/analysis/content-moderation-challenges) -- Decentralized network growth pains
