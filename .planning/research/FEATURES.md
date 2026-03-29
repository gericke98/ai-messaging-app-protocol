# Feature Research: Federation for AINM v3.2

**Domain:** Multi-server federation for AI-native messaging protocol
**Researched:** 2026-03-28
**Confidence:** HIGH

## Existing AINM Infrastructure (Federation Foundations Already Built)

Before categorizing new features, here is what AINM v3.1 already provides that federation builds on:

| Existing Infrastructure | Spec | Federation Relevance |
|------------------------|------|---------------------|
| Email-like addressing (`name@provider`) | identity.md | Natural federation routing -- the `provider` portion determines the home server |
| Identity resolution via `.well-known/ainm/{name}` | identity.md | Remote servers can resolve any participant's public key and home server |
| Server identity at `.well-known/ainm/server` | transport.md | Federation discovery -- servers publish their relay endpoint, public key, TLS fingerprint |
| HTTP relay endpoint `POST /ainm/v1/relay` | transport.md | The basic S2S message relay mechanism (defined but needs full implementation) |
| Mutual TLS for S2S | transport.md | Bidirectional server authentication on relay connections |
| Server-level Ed25519 signatures on relay payloads | transport.md | Relay payload integrity -- originating server signs the batch |
| Per-message Ed25519 signatures | security.md | End-to-end message integrity across federation hops |
| Three-layer relay validation (mTLS + server sig + message sig) | transport.md | Defense-in-depth already specified |
| Store-and-forward for offline participants | transport.md | Messages queued when recipient is disconnected |
| Ed25519 key rotation with history | identity.md | Historical key verification for messages signed with rotated keys |
| JSON-RPC 2.0 framing | transport.md | Consistent wire format for both client-server and server-server |
| UUIDv7 message IDs | security.md | Time-ordered, globally unique, deduplication-ready |
| Thread-addressed delivery | threading.md | Messages go to threads, not individuals -- simplifies cross-server routing |

**What this means:** AINM's v3.1 spec already defines the federation relay endpoint, server identity discovery, mTLS, and three-layer signature verification. The v3.2 milestone is about making this actually work end-to-end with full implementation, filling gaps the v3.1 spec sketched but did not exercise.

---

## Feature Landscape

### Table Stakes (Users/Developers Expect These)

Features that anyone evaluating a "federated messaging protocol" assumes exist. Missing any of these makes federation feel like a demo, not a real capability.

| # | Feature | Why Expected | Complexity | AINM Dependency | Notes |
|---|---------|--------------|------------|-----------------|-------|
| TS-1 | **Cross-server message delivery** | The fundamental promise of federation. alice@server-a.com sends a message, bob@server-b.com receives it. Without this, "federation" is a lie. | MEDIUM | Builds on existing `ainm.relay` endpoint in transport.md. Needs full implementation of relay request construction, sending, validation, and local delivery on the receiving side. | Matrix does this via transactions (PUT /send/{txnId}). XMPP does it via S2S TCP streams. Email does it via MX record lookup + SMTP. AINM's HTTP POST relay is closest to email's model. |
| TS-2 | **Server discovery and identity verification** | Servers must find each other and prove they are who they claim to be. Without this, federation is insecure and unroutable. | LOW | `.well-known/ainm/server` endpoint already specified. Needs implementation of server identity resolution, caching, and TLS fingerprint verification during mTLS handshake. | Matrix uses `.well-known/matrix/server` + SRV records. XMPP uses `_xmpp-server._tcp` SRV records + STARTTLS + dialback. AINM's `.well-known` approach is simpler and sufficient -- no SRV records needed for v1. |
| TS-3 | **Offline relay queues** | When Server B is unreachable, Server A must queue messages and retry delivery later. Without this, a brief outage means lost messages. | MEDIUM | Store-and-forward for clients exists in transport.md. Server-to-server queuing is new -- need persistent queue, retry with exponential backoff, delivery confirmation. | Email has this (SMTP retries for days). Matrix retries transactions. ActivityPub specifies async delivery with retry on network error. Queue retention policy matters: email retries for 5 days, Matrix retries indefinitely. |
| TS-4 | **Message deduplication across federation** | At-least-once delivery means duplicates will arrive. Servers must not deliver the same message twice. | LOW | UUIDv7 deduplication already specified in security.md Section 7. Needs to be enforced at the relay level -- receiving server must track relayed message IDs to prevent re-delivery. | Already well-designed in AINM. UUIDv7's time-ordered nature makes sliding-window deduplication efficient. |
| TS-5 | **Origin-server authority for thread state** | One server must be the authoritative source of truth for each thread's participant list, metadata, and ordering. Without this, split-brain on "who is in this thread" is inevitable. | HIGH | Thread metadata (threading.md) has `created_by` which implies origin. Needs explicit `origin_server` field on thread metadata, and rules for which server's state wins on conflict. | AINM deliberately chose NOT to use Matrix's DAG-based eventual consistency. Origin-server authority (like XMPP MUC where the room-hosting server is authoritative) is simpler and correct for AINM's flat threading model. This is the single most important architectural decision for federation. |
| TS-6 | **Federated thread creation with remote participants** | Creating a thread with participants on different servers must work. Server A creates the thread, then notifies Server B that bob@server-b.com has been added. | MEDIUM | `ainm.thread.create` exists with participant list. Needs federation extension: origin server notifies each participant's home server of the thread creation + initial state. | Matrix uses an invite/join handshake (make_join/send_join). XMPP MUC has an explicit join flow. AINM should use a simpler push model: origin server sends a `thread.invite` relay to each remote participant's server. |
| TS-7 | **Federated thread join/leave** | Participants on remote servers must be able to join and leave threads hosted on other servers, with state propagated to all participating servers. | MEDIUM | `ainm.thread.join` and `ainm.thread.leave` exist for client-server. Needs S2S equivalents: relay messages for join/leave events propagated to all participating servers. | The origin server is the authority. Join/leave requests go TO the origin server, which validates and then broadcasts the state change to all participating servers. |
| TS-8 | **Cross-server identity resolution** | When Server A receives a message from alice@server-b.com, it must be able to resolve Alice's public key to verify the message signature. | LOW | Identity resolution via HTTPS GET to `.well-known/ainm/{name}` already fully specified. Needs caching implementation with TTL, cache invalidation on signature verification failure. | This is already designed. Implementation needs: HTTP client for identity resolution, local cache with 1-hour TTL, re-fetch on verification failure. |
| TS-9 | **Federation rate limiting** | Servers must be able to throttle relay traffic from abusive or misconfigured peer servers. Without this, a single bad server can overwhelm the network. | MEDIUM | Error code `-32022` (Rate limited) already defined. Needs implementation: per-source-server rate limits on the relay endpoint, `retry_after` in error responses. | Matrix has had significant issues with federation rate limiting (PR #5621 fixing rate limiter bugs). ActivityPub specifies async delivery but leaves rate limiting to implementations. AINM should define sensible defaults (e.g., 100 relay requests/minute per source server). |
| TS-10 | **Relay delivery confirmation/rejection** | The originating server must know whether the target server accepted or rejected each relayed message, with per-message error details. | LOW | Already specified in transport.md Section 5.3: response includes `accepted`, `rejected`, `errors` array with per-message details. Needs implementation. | Well-designed in existing spec. |

### Differentiators (Competitive Advantage)

Features that set AINM federation apart from Matrix/XMPP/ActivityPub. Not required for federation to "work," but valuable for AINM's AI-native positioning.

| # | Feature | Value Proposition | Complexity | AINM Dependency | Notes |
|---|---------|-------------------|------------|-----------------|-------|
| DF-1 | **Selective thread backfill** | When a new server joins a thread (because a remote participant was added), the origin server provides selective history -- not the full DAG, not zero context, but a curated window of recent messages + context frame. This avoids Matrix's "download the entire room history" problem. | HIGH | Builds on memory.md's context frame system (`ainm.context.frame`). Needs a federation-specific backfill request: remote server asks origin for recent history of a thread. Origin responds with messages + context frame, bounded by configurable limits. | Matrix's backfill fetches raw event history (GET /backfill/{roomId}). AINM's memory system (active context + conversation history + archival) can provide *intelligent* backfill -- a context frame summarizing key decisions + recent messages. This is a genuine differentiator for AI agents joining threads. |
| DF-2 | **Agent capability federation** | When an agent on Server A is added to a thread on Server B, Server B can resolve the agent's Agent Card to understand its capabilities before routing messages to it. Cross-server agent discovery enables "bring your own agent" across providers. | MEDIUM | Agent Cards already specified in agent-card.md with `agent_card_url` in identity records. Needs: federation-aware Agent Card resolution, capability matching for thread operations. | No other federated protocol has this. Matrix has no concept of "agent capabilities" at the protocol level. This makes AINM uniquely suited for multi-provider agent ecosystems. |
| DF-3 | **Federated presence with agent-specific states** | Presence updates (`processing`, `busy`, `available`) for agents propagate across servers, so participants on Server B see that weather-bot@server-a.com is "processing: Fetching weather data." | MEDIUM | Presence system exists in transport.md (ainm.presence.update/notify). Needs S2S presence relay -- origin server broadcasts presence changes to all participating servers. | Matrix relays presence via EDUs in federation transactions. AINM's agent-specific presence states (`processing` with detail strings) are more expressive than Matrix's generic `online/unavailable/offline`. Presence relay should be best-effort (not queued for offline servers). |
| DF-4 | **Federated streaming responses** | When an agent on Server A streams a response to a thread on Server B, the stream events (`ainm.stream.start/delta/end`) are relayed in real-time to Server B's connected clients. | HIGH | Streaming spec (streaming.md) defines the event model. Needs: real-time relay of stream events from origin server to participating servers, WebSocket-level forwarding. HTTP relay (request/response) may be too slow -- may need WebSocket S2S for streaming or SSE push. | This is genuinely hard. Matrix does not support streaming agent responses. AINM could relay stream deltas over the existing HTTP relay (batched at short intervals, e.g., every 100ms), or introduce a WebSocket-based S2S channel for streaming. HTTP relay with short-interval batching is pragmatic for v1 federation. |
| DF-5 | **Server allowlist/denylist** | Operators can configure which servers they federate with. Allowlist mode (only talk to these servers) for enterprise. Denylist mode (talk to everyone except these) for public. | LOW | No existing spec coverage. Needs: server configuration for allowed/denied servers, enforcement at relay endpoint. | Matrix has `federation_domain_whitelist` in Synapse config. XMPP has TLS-based trust levels. Essential for enterprise deployments where you want controlled federation (only your org's servers) rather than open federation. |
| DF-6 | **Cross-server memory and context** | When a participant on Server B queries archival memory (`ainm.memory.query`) for facts stored on Server A's thread, the query is federated to the origin server. | HIGH | Memory spec (memory.md) defines store/query/evict. Needs: federation routing for memory queries to origin server, response relay back to requesting server. | This is unique to AINM. No other federated protocol has structured memory at the protocol level. Deferred to later if complexity is too high for v3.2, but should be designed for. |
| DF-7 | **Federated tool call visibility** | When an agent on Server A executes a tool call in a thread with participants on Server B, the tool call events (started/progress/completed) are relayed so all participants see what the agent is doing. | MEDIUM | Tool visibility spec (tool-visibility.md) defines events. These are delivered via `ainm.message.deliver`, so they naturally flow through the relay. Needs: ensuring tool call events are included in relay batches. | This should work with minimal extra effort if relay handles all message types correctly. |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good for federation but create disproportionate complexity, security risk, or architectural problems. Explicitly not building these for v3.2.

| # | Feature | Why Requested | Why Problematic | Alternative |
|---|---------|---------------|-----------------|-------------|
| AF-1 | **DAG-based eventual consistency (Matrix model)** | "What if two servers are partitioned and both accept messages? Need a DAG to merge!" | Matrix's state resolution algorithm is notoriously complex (two major versions, academic papers analyzing correctness, rooms taking 10-60 minutes to join due to state resolution). AINM's flat threading model does not need this. Origin-server authority with store-and-forward is simpler and correct: if the origin server is unreachable, messages queue until it comes back. No conflicting state to merge. | Origin-server authority. The server that created the thread is the source of truth for thread state (participants, metadata). Other servers cache this state but defer to origin on conflicts. Messages queue when origin is unreachable. |
| AF-2 | **Persistent S2S connections (XMPP model)** | "Persistent connections between servers reduce latency for high-volume federation." | Complexity of connection lifecycle management, reconnection, multiplexing. Every server must maintain connections to every other server it federates with -- O(n^2) connections in a large network. HTTP POST is stateless and simpler. | Stateless HTTP POST relay with connection pooling. HTTP/2 or HTTP/3 can multiplex requests over a single connection for efficiency without the complexity of managing persistent protocol-level connections. |
| AF-3 | **Global presence federation** | "Broadcast presence to all federated servers so everyone knows who's online everywhere." | Matrix found that presence in large rooms causes catastrophic performance (50,000+ HTTP requests/hour for one user typing in an 800-server room). Global presence does not scale. | Thread-scoped presence only. Presence is relayed only to servers participating in the same thread. A server with no shared threads gets no presence updates. This bounds the blast radius. |
| AF-4 | **Multi-hop relay (transitive federation)** | "Server A can relay through Server B to reach Server C, like email relays." | Creates complexity in signature verification chains, blame attribution for failures, and potential for relay abuse (open relay problem from email). Each hop adds latency. | Direct server-to-server relay only. Server A resolves Server C's identity directly and sends the relay. No intermediaries. This is simpler and more secure. |
| AF-5 | **End-to-end encrypted federation (Signal protocol)** | "Messages should be encrypted so even federated servers can't read them." | Fundamentally conflicts with AINM's AI-native design: agents need to read message content to respond. E2EE would require agents to hold decryption keys, which defeats the purpose. Also vastly complex for multi-party threads across servers. | Transport-layer encryption (TLS 1.3) + per-message signing (Ed25519). Messages are encrypted in transit (TLS) and authenticated (signatures), but readable by servers for agent processing. E2EE is a v2+ consideration for human-only threads. |
| AF-6 | **Full message history replication** | "When a new server joins, replicate the complete thread history." | Unbounded data transfer. A thread with 100K messages should not force a new server to download all of them. Bandwidth, storage, and privacy concerns. | Selective backfill (DF-1). Origin server provides a bounded window of recent messages plus a context frame summarizing key history. Configurable limit (e.g., last 100 messages + context summary). |
| AF-7 | **Federated thread migration** | "Move a thread's origin from Server A to Server B." | Extremely complex state transfer. All participating servers need to update their authority pointer atomically. Partial migration leaves the system in an inconsistent state. | Threads are permanently homed on their origin server. If a server shuts down, threads are lost (like email -- if your mail server dies, your mailbox dies). Future protocol versions could add migration, but it is not v3.2 scope. |
| AF-8 | **Cross-server moderation / admin API** | "Admins on Server A should be able to moderate threads and users on Server B." | Violates server autonomy. Each server admin controls their own domain. Cross-server moderation creates authority disputes and abuse vectors. | Per-server moderation only. Each server enforces its own policies. The origin server of a thread can remove participants, but cannot control what other servers do with messages already delivered. Server operators can use allowlists/denylists (DF-5) to control which servers they federate with. |

---

## Feature Dependencies

```
[TS-2: Server Discovery]
    |
    +--required-by--> [TS-1: Cross-Server Message Delivery]
    |                      |
    |                      +--required-by--> [TS-3: Offline Relay Queues]
    |                      +--required-by--> [TS-4: Deduplication at Relay Level]
    |                      +--required-by--> [TS-10: Delivery Confirmation]
    |                      +--required-by--> [DF-7: Federated Tool Visibility]
    |
    +--required-by--> [TS-8: Cross-Server Identity Resolution]
    |                      |
    |                      +--required-by--> [TS-1: Cross-Server Message Delivery]
    |                      +--required-by--> [DF-2: Agent Capability Federation]
    |
    +--required-by--> [TS-5: Origin-Server Authority]
                           |
                           +--required-by--> [TS-6: Federated Thread Creation]
                           +--required-by--> [TS-7: Federated Thread Join/Leave]
                           +--required-by--> [DF-1: Selective Backfill]
                           +--required-by--> [DF-6: Cross-Server Memory]

[TS-1: Cross-Server Delivery]
    +--enhances--> [DF-3: Federated Presence]
    +--enhances--> [DF-4: Federated Streaming]

[TS-5: Origin-Server Authority]
    +--conflicts-with--> [AF-1: DAG-based Eventual Consistency]
    +--conflicts-with--> [AF-7: Thread Migration]

[DF-5: Server Allowlist/Denylist]
    +--independent-- (can be added at any time, enforced at relay endpoint)
```

### Dependency Notes

- **TS-2 (Server Discovery) is the foundation.** Nothing works without servers being able to find and authenticate each other. Must be implemented first.
- **TS-8 (Identity Resolution) is the second foundation.** Message signature verification across servers requires resolving remote identities. Already specified, needs implementation.
- **TS-5 (Origin-Server Authority) is the critical architectural decision.** It determines how thread state is managed and conflicts with the Matrix DAG model (AF-1). Must be decided and implemented before any thread-level federation features.
- **TS-1 (Cross-Server Delivery) is the core feature.** Everything else (presence, streaming, backfill) depends on the basic relay working.
- **TS-3 (Offline Queues) is required for reliability.** Without it, federation is fragile and messages are lost during outages.
- **DF-1 (Selective Backfill) requires TS-5 (Origin Authority)** because the origin server decides what history to provide.
- **DF-4 (Federated Streaming) enhances TS-1** but requires additional design for real-time relay (HTTP batching vs WebSocket S2S).
- **DF-5 (Allowlist/Denylist) is independent** and can be enforced at the relay endpoint without touching other features.
- **AF-1 conflicts with TS-5.** AINM cannot have both origin-server authority and DAG-based eventual consistency. The explicit choice is origin-server authority.

---

## MVP Definition

### Launch With (v3.2 Core -- Federation Works)

Minimum viable federation -- what's needed to demo two servers exchanging messages.

- [x] **TS-2: Server discovery and identity verification** -- Foundation. Servers can find and authenticate each other.
- [x] **TS-8: Cross-server identity resolution** -- Servers can resolve remote participants' public keys.
- [x] **TS-5: Origin-server authority for thread state** -- Each thread has a single authoritative server.
- [x] **TS-1: Cross-server message delivery** -- The relay endpoint sends and receives messages.
- [x] **TS-10: Relay delivery confirmation/rejection** -- Originating server knows if delivery succeeded.
- [x] **TS-4: Message deduplication across federation** -- No duplicate messages delivered.
- [x] **TS-6: Federated thread creation** -- Create threads with remote participants.
- [x] **TS-7: Federated thread join/leave** -- Remote participants can join/leave threads.
- [x] **TS-9: Federation rate limiting** -- Protect against relay abuse.

### Add After Core (v3.2 Complete -- Federation is Reliable)

Features to add once basic federation is proven.

- [ ] **TS-3: Offline relay queues** -- trigger: first network partition during testing reveals message loss
- [ ] **DF-1: Selective thread backfill** -- trigger: remote participant added to existing thread needs context
- [ ] **DF-3: Federated presence** -- trigger: demo needs to show agent status across servers
- [ ] **DF-5: Server allowlist/denylist** -- trigger: multi-server demo needs access control
- [ ] **DF-7: Federated tool call visibility** -- trigger: should flow naturally through relay if all message types are handled

### Future Consideration (v3.3+)

Features to defer until federation is battle-tested.

- [ ] **DF-2: Agent capability federation** -- Requires Agent Card cross-server resolution patterns; defer until agent ecosystem grows
- [ ] **DF-4: Federated streaming responses** -- Requires real-time S2S channel design; HTTP batching may introduce unacceptable latency
- [ ] **DF-6: Cross-server memory queries** -- Requires federation routing for memory.query to origin; complex authorization model

---

## Feature Prioritization Matrix

| # | Feature | User Value | Implementation Cost | Priority |
|---|---------|------------|---------------------|----------|
| TS-2 | Server discovery & verification | HIGH | LOW | **P1** |
| TS-8 | Cross-server identity resolution | HIGH | LOW | **P1** |
| TS-5 | Origin-server authority | HIGH | HIGH | **P1** |
| TS-1 | Cross-server message delivery | HIGH | MEDIUM | **P1** |
| TS-10 | Delivery confirmation | MEDIUM | LOW | **P1** |
| TS-4 | Deduplication at relay level | MEDIUM | LOW | **P1** |
| TS-6 | Federated thread creation | HIGH | MEDIUM | **P1** |
| TS-7 | Federated thread join/leave | HIGH | MEDIUM | **P1** |
| TS-9 | Rate limiting | MEDIUM | MEDIUM | **P1** |
| TS-3 | Offline relay queues | HIGH | MEDIUM | **P2** |
| DF-1 | Selective thread backfill | HIGH | HIGH | **P2** |
| DF-3 | Federated presence | MEDIUM | MEDIUM | **P2** |
| DF-5 | Server allowlist/denylist | MEDIUM | LOW | **P2** |
| DF-7 | Federated tool visibility | MEDIUM | LOW | **P2** |
| DF-2 | Agent capability federation | MEDIUM | MEDIUM | **P3** |
| DF-4 | Federated streaming | HIGH | HIGH | **P3** |
| DF-6 | Cross-server memory | HIGH | HIGH | **P3** |

**Priority key:**
- P1: Required for v3.2 launch -- federation demo with 2+ servers
- P2: Should have for v3.2 completeness -- reliability and AI-native differentiation
- P3: Future consideration -- complex features that need more design

---

## Competitor Feature Analysis

| Feature | Matrix | XMPP | ActivityPub | Email (SMTP) | AINM Approach |
|---------|--------|------|-------------|--------------|---------------|
| **Server discovery** | `.well-known/matrix/server` + SRV records | `_xmpp-server._tcp` SRV + STARTTLS dialback | WebFinger + actor URL resolution | MX records (DNS) | `.well-known/ainm/server` -- simplest approach, no SRV records needed |
| **S2S authentication** | Ed25519 HTTP signatures + TLS | TLS + SASL + dialback (multiple fallback levels) | HTTP Signatures (typically RSA-SHA256) | STARTTLS (opportunistic), DKIM/SPF/DMARC | Mutual TLS + Ed25519 server signatures -- strongest model, no fallback levels |
| **Message integrity** | Event signatures (per-event) | No per-message signing (TLS only) | HTTP Signatures on delivery (not per-message) | DKIM (per-message, domain-level) | Ed25519 per-message signatures -- strongest model, end-to-end verifiable across hops |
| **State management** | DAG + state resolution (v2) | Origin-server authority (MUC host) | No shared state (unidirectional follow model) | No shared state (store-and-forward) | Origin-server authority -- like XMPP MUC, not like Matrix DAG |
| **History backfill** | Full event graph (GET /backfill) | No standardized cross-server history (MUC host stores all) | No history (timeline is append-only) | No history (each server keeps its own copies) | Selective backfill: bounded window + context frame (unique to AINM) |
| **Offline handling** | Transaction retries (indefinite) | Queue with retry | Async delivery with retry on error | Queue for up to 5 days, then bounce | Persistent relay queue with configurable retention (7 days default) |
| **Presence** | EDUs in transactions (per-room) | Global presence + per-room | No presence concept | No presence | Thread-scoped only -- avoids Matrix's scaling nightmare |
| **Typing indicators** | EDUs (ephemeral, per-room) | XEP-0085 chat states | No typing indicators | N/A | Agent-specific states (processing, busy) -- richer than Matrix |
| **Group chat federation** | Fully distributed (every server has full room state) | Centralized on MUC host, FMUC experimental | No group chat (public timeline model) | Mailing lists (centralized on list server) | Origin-server authority, state cached on participating servers |
| **Rate limiting** | Per-server, bug-prone (Synapse #5621) | Per-connection throttling | Implementation-specific | Greylisting, per-IP throttling | Per-source-server rate limits on relay endpoint with `retry_after` |
| **Agent/bot support** | Application services (bridge concept) | No protocol-level agent concept | No agent concept | No agent concept | **First-class agents with federated capability discovery (Agent Cards)** |

---

## Key Insights from Protocol Research

### Why Origin-Server Authority Over DAG (Critical Decision)

**Matrix's approach:** Every server in a room has a full copy of the event DAG. When servers disagree on state (who is in the room, room settings), a state resolution algorithm merges the DAGs. This creates:
- Joining large rooms takes 10-60 minutes due to state resolution
- State resolution has had two major algorithm versions, with formal correctness analyses
- Memory usage scales with room participation (800 servers = 3GB+ for typing notifications)
- Synapse's database grows to 45GB+ with federation history

**AINM's approach (recommended):** The server that created a thread is the authority. Other servers cache thread state but defer to origin. This means:
- No state resolution algorithm needed (eliminates the hardest part of Matrix federation)
- Thread creation is O(1), not O(n) where n is the number of participating servers
- Presence is thread-scoped, not global
- Trade-off: if origin server is down, the thread is read-only (cached state only)

**Why this is correct for AINM:** AINM uses flat threads with one level of sub-threading. There are no merge conflicts to resolve because the origin server applies operations sequentially. Matrix needs DAGs because rooms are fully distributed with no authority. AINM's origin-server model is closer to email (your mailbox lives on your server) and XMPP MUC (the room lives on the hosting server).

### Selective Backfill is the Real Differentiator

Every federated protocol handles "new server joins a conversation" differently:
- **Matrix:** Download the full event graph from the point of joining. Potentially gigabytes.
- **XMPP MUC:** Room host sends configurable history (last N messages or last N seconds).
- **ActivityPub:** No history -- you see posts from the time you followed.
- **Email:** No history -- join a mailing list, see new messages only.

**AINM can do better** by leveraging its memory system:
- Send a context frame (memory.md) summarizing key decisions and context
- Send the last N messages (configurable, default 100)
- The context frame gives agents immediate situational awareness without downloading the full history
- This is genuinely novel -- no other federated protocol has structured context summarization

### Ephemeral vs. Persistent Data in Federation

Matrix distinguishes between PDUs (persistent, stored in DAG) and EDUs (ephemeral, not stored). This distinction is important:

| Data Type | Matrix | AINM Equivalent | Federation Handling |
|-----------|--------|-----------------|---------------------|
| Messages | PDU (persistent) | Messages (signed, stored) | Relay with queue for offline servers |
| Typing indicators | EDU (ephemeral) | Presence updates (typing could be a presence state) | Best-effort relay, no queue |
| Read receipts | EDU (ephemeral) | Not yet specified | Best-effort relay |
| Presence | EDU (ephemeral) | Presence updates | Best-effort relay, thread-scoped only |

**Recommendation:** AINM should distinguish between "must-deliver" (messages, thread operations) and "best-effort" (presence, typing) at the relay level. Must-deliver items go into offline queues; best-effort items are dropped if the target server is unreachable. This prevents presence/typing from overwhelming the relay queue during outages.

---

## Sources

- [Matrix Server-Server API v1.16](https://spec.matrix.org/v1.16/server-server-api/) -- Federation endpoints, transactions, backfill, state resolution
- [Matrix State Resolution v2](https://matrix.org/docs/older/stateres-v2/) -- How Matrix resolves conflicting state across servers
- [Matrix Federation Security Improvements 2025](https://media.ccc.de/v/matrix-conf-2025-74569-improving-the-security-of-the-federation-protocol)
- [Synapse Federation Rate Limiter Fixes](https://github.com/matrix-org/synapse/pull/5621) -- Real-world rate limiting challenges
- [Synapse Performance on SBCs](https://github.com/matrix-org/synapse/issues/8428) -- Federation resource usage issues
- [Conduwuit: Lightweight Matrix Server](https://edu4rdshl.dev/posts/about-to-leave-matrix-oh-wait-there-s-conduwuit/) -- 500MB vs 45GB database comparison
- [XEP-0238: XMPP Inter-Domain Federation](https://xmpp.org/extensions/xep-0238.html) -- XMPP S2S protocol flows
- [XEP-0288: Bidirectional S2S Connections](https://xmpp.org/extensions/xep-0288.html)
- [XEP-0289: Federated MUC](https://xmpp.org/extensions/xep-0289.html) -- Distributed multi-user chat
- [XEP-0045: Multi-User Chat](https://xmpp.org/extensions/xep-0045.html) -- Standard MUC specification
- [Isode Federated MUC Whitepaper](https://www.isode.com/whitepaper/federated-multi-user-chat/) -- SOURCE/SHADOW room model
- [W3C ActivityPub Specification](https://www.w3.org/TR/activitypub/) -- Inbox/outbox delivery model
- [ActivityPub Shared Inbox](https://github.com/w3c/activitypub/issues/242) -- Federation efficiency via shared inboxes
- [Threema: Federated vs. Centralized](https://threema.ch/en/work/blog/posts/federated-vs-centralized) -- Critique of federation tradeoffs
- [Wire: Federation Types & Compliance](https://wire.com/en/blog/federating-security-or-spam) -- Open vs closed federation security
- [Matrix Policy Servers (2025)](https://matrix.org/blog/2025/04/introducing-policy-servers/) -- Anti-abuse tooling for federation
- [Cloudflare: DNS MX Records](https://www.cloudflare.com/learning/dns/dns-records/dns-mx-record/) -- How email federation routing works
- [AINM Transport Specification v0.1.0](spec/transport.md) -- Existing federation relay specification
- [AINM Identity Specification v0.1.0](spec/identity.md) -- Identity resolution and key management
- [AINM Security Specification v0.1.0](spec/security.md) -- Message signing and verification

---
*Feature research for: AINM v3.2 Federation*
*Researched: 2026-03-28*
