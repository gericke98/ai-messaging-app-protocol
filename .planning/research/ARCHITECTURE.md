# Architecture Patterns for Federated Messaging Systems

**Domain:** Federated AI-native messaging protocol + reference server + iOS client
**Researched:** 2026-03-27
**Overall confidence:** HIGH (based on analysis of Matrix, XMPP, ActivityPub, SMTP -- all mature, well-documented federated systems)

---

## 1. How Existing Federated Systems Are Structured

### 1.1 The Universal Pattern

Every successful federated messaging system shares the same fundamental architecture, regardless of era or technology:

```
+-------------------+          +-------------------+
|   Client (MUA)    |          |   Client (MUA)    |
|  iOS / Web / CLI  |          |  iOS / Web / CLI  |
+--------+----------+          +--------+----------+
         |  Client-Server API           |  Client-Server API
+--------v----------+          +--------v----------+
|  Home Server (A)  | <------> |  Home Server (B)  |
|                   |  Server  |                   |
|  - Auth           |  Server  |  - Auth           |
|  - Message Store  |   API    |  - Message Store  |
|  - Federation     | (S2S)    |  - Federation     |
|  - Identity       |          |  - Identity       |
+-------------------+          +-------------------+
```

This pattern holds across all four systems:

| System | Client | Server | Federation Transport | Identity Format | Data Model |
|--------|--------|--------|---------------------|-----------------|------------|
| **SMTP/Email** | MUA (Thunderbird, Mail.app) | MTA+MDA (Postfix, Dovecot) | SMTP over TCP/TLS, MX DNS records | `user@domain` | Individual messages with headers |
| **XMPP** | Client (Conversations, Monal) | Server (ejabberd, Prosody) | XML streams over TCP/TLS, SRV DNS records | `user@domain/resource` | XML stanzas (message, presence, iq) |
| **Matrix** | Client (Element, FluffyChat) | Homeserver (Synapse, Conduit) | HTTPS + JSON, SRV DNS records | `@user:domain` | Events in a DAG per room |
| **ActivityPub** | Client (Mastodon Web, Ice Cubes) | Server (Mastodon, Akkoma) | HTTPS POST to inbox URLs | `@user@domain` (WebFinger) | Activities + Objects (JSON-LD) |

**Key insight:** The architecture separates into exactly three APIs: Client-Server (C2S), Server-Server (S2S/Federation), and Identity/Discovery. Every system has all three. The protocol spec must define all three.

### 1.2 Matrix Architecture (Most Relevant Precedent)

Matrix is the closest architectural precedent because it handles persistent group conversations with rich events, full history replication, and end-to-end encryption -- all requirements for this project.

**Core Concepts:**
- **Rooms** are the fundamental unit. Every conversation is a room. 1:1, group, public channels -- all rooms.
- **Events** are immutable JSON objects. Everything is an event: messages, state changes (member joins, room name changes, permissions), redactions.
- **Event Graph (DAG)** -- events in a room form a Directed Acyclic Graph. Each event references its parent events via `prev_events`. This creates a partial ordering that handles concurrent events from multiple servers.
- **State Events** -- special events that define room configuration (name, topic, join rules, power levels). The current "state" of a room is the set of latest state events of each type.
- **State Resolution** -- when servers diverge (network partition), their DAGs fork. When they reconnect, a deterministic state resolution algorithm merges the forks. This is Matrix's hardest problem -- v1 had "state resets" (malicious rollbacks), v2 fixed most issues but remains complex and counter-intuitive.

**Matrix Spec Structure (v1.18, released 2026-03-26):**
1. **Client-Server API** -- how clients talk to their homeserver (auth, sync, rooms, events, media, push rules, typing indicators, read receipts, presence)
2. **Server-Server API (Federation)** -- how homeservers talk to each other (event exchange, state queries, key exchange, backfill, device lists)
3. **Application Service API** -- bridging external services (IRC, Slack, Discord bridges)
4. **Identity Service API** -- mapping third-party identifiers (email, phone) to Matrix IDs
5. **Push Gateway API** -- delivering push notifications to mobile devices
6. **Room Versions** -- 12 versions documenting protocol evolution
7. **Olm/Megolm** -- end-to-end encryption specifications

**Lesson for us:** Matrix's 5-API structure is the right granularity. Our protocol needs at minimum: Client-Server, Server-Server (Federation), and Agent Discovery. The Application Service API equivalent is how external agents plug in.

### 1.3 XMPP Architecture

XMPP takes a fundamentally different approach from Matrix -- it is event-based (process and forward) rather than document-based (replicate and sync).

**Core Concepts:**
- **XML Streams** -- a persistent TCP connection over which XML stanzas flow. The connection IS the session.
- **Three Stanza Types:** `<message>` (one-way content delivery), `<presence>` (availability broadcast), `<iq>` (info/query request-response).
- **JID (Jabber ID)** -- `user@domain/resource` where resource identifies a specific device/session. A user can have multiple resources (phone, desktop) simultaneously.
- **Extensions (XEPs)** -- the core protocol is intentionally minimal. Features are added via XEP extension specifications (there are 400+ XEPs covering everything from file transfer to MUC to OMEMO encryption).
- **Server-to-Server Federation** -- servers discover each other via DNS SRV records, establish TLS connections, and validate using SASL or dialback.

**Key Architectural Differences from Matrix:**
- XMPP does NOT replicate full conversation history by default. Messages are routed and delivered; persistence is optional (via XEP-0313 MAM).
- No DAG, no state resolution -- simpler, but also means no offline-first guarantee and no censorship resistance.
- Presence is a first-class concept (online/away/busy/offline) -- Matrix added this but it's not as deeply integrated.

**Lesson for us:** XMPP's extensibility model (minimal core + extensions) is a better fit than Matrix's monolithic spec. We should define a small core protocol and use a formal extension mechanism for features like streaming, rich content, memory management, and agent capabilities. The resource concept in JIDs (`user@domain/device`) is useful for multi-device support.

### 1.4 ActivityPub Architecture

ActivityPub powers the Fediverse (Mastodon, Lemmy, PeerTube). It is the simplest federated protocol architecturally.

**Core Concepts:**
- **Actors** -- entities with an Inbox and an Outbox. An actor can be a person, group, application, or service.
- **Activities** -- JSON-LD objects representing actions (Create, Follow, Like, Announce, Delete).
- **Inbox/Outbox** -- each actor has two HTTP endpoints. POST to inbox = deliver activity to actor. GET outbox = read actor's public activities.
- **Shared Inbox** -- optimization where a server accepts activities for all its local actors at one endpoint, avoiding N copies for N followers.
- **WebFinger** -- discovery protocol. `GET /.well-known/webfinger?resource=acct:user@domain` returns the actor's profile URL.

**Key Design Decisions:**
- Push-based: the sending server delivers to each recipient's inbox. No pull/sync mechanism.
- No conversation model: there's no "room" or "thread" -- conversations are reconstructed from reply chains, which is why Mastodon threads are often incomplete (your server only has the replies it received).
- JSON-LD: semantic web heritage. In practice, most implementations treat it as regular JSON with `@context`.

**Lesson for us:** ActivityPub's actor model (inbox/outbox) maps cleanly to our agent model. Each agent IS an actor with message endpoints. WebFinger is the right discovery mechanism for email-like identities. But ActivityPub's lack of a conversation/room primitive is a clear anti-pattern -- we need rooms/threads as first-class objects. The push-based delivery model is simpler than Matrix's state replication but loses the "full history on every server" guarantee.

### 1.5 SMTP Architecture

Email is the oldest and most successful federated system. 50+ years of operation.

**Core Components:**
- **MUA** (Mail User Agent) -- the email client
- **MSA** (Mail Submission Agent) -- accepts messages from the client, enforces policy
- **MTA** (Mail Transfer Agent) -- routes messages between servers via SMTP
- **MDA** (Mail Delivery Agent) -- delivers messages to the recipient's mailbox
- **MX Records** -- DNS records that tell senders which server handles mail for a domain

**Why Email Succeeded at Federation:**
- Dead-simple discovery: look up MX record, connect, deliver.
- No state synchronization: messages are fire-and-forget. The receiving server decides what to do.
- Minimal coordination: servers don't need to agree on room state, membership, or history.
- Graceful degradation: if a server is down, the sending MTA queues and retries.

**Lesson for us:** Email-like addressing (`name@provider`) is the right identity model -- it's the most human-readable and intuitive format for federation. MX-record-style DNS discovery is battle-tested and should be the basis of server discovery. Email's weakness (no conversations, no real-time, no presence) is exactly what we're adding, but we should preserve email's simplicity of message delivery.

---

## 2. Recommended Architecture for AI-Native Messaging Protocol

### 2.1 System Components

Based on analysis of all four federated systems, the protocol defines these components:

```
+------------------------------------------------------------------+
|                        CLIENTS                                    |
|  +------------------+  +------------------+  +------------------+ |
|  | iOS App (ref)    |  | Web Client       |  | Agent SDK        | |
|  | SwiftUI + SDK    |  | (future)         |  | (headless)       | |
|  +--------+---------+  +--------+---------+  +--------+---------+ |
+-----------|----------------------|----------------------|---------+
            |                      |                      |
            +----------------------+----------------------+
                                   | Client-Server API (C2S)
                                   | WebSocket + HTTPS
+----------------------------------v-----------------------------------+
|                         HOME SERVER (Rust/Axum)                      |
|                                                                      |
|  +------------------+  +------------------+  +--------------------+  |
|  | Connection Mgr   |  | Auth & Identity  |  | Agent Registry     |  |
|  | (WebSocket/HTTP)  |  | (keys, tokens)   |  | (capability cards) |  |
|  +--------+---------+  +--------+---------+  +--------+----------+  |
|           |                      |                      |            |
|  +--------v---------+  +--------v---------+  +--------v----------+  |
|  | Message Router   |  | Thread Manager   |  | Stream Manager    |  |
|  | (local + fed)    |  | (rooms/threads)  |  | (real-time tokens)|  |
|  +--------+---------+  +--------+---------+  +--------+----------+  |
|           |                      |                      |            |
|  +--------v--------------------------------------------------+      |
|  |                    Event Store (PostgreSQL)                |      |
|  |  (append-only log of all events per thread)               |      |
|  +--------+--------------------------------------------------+      |
|           |                                                          |
|  +--------v---------+  +--------------------+  +------------------+  |
|  | Federation Sender|  | Push Gateway       |  | Media Store      |  |
|  | (outbound S2S)   |  | (APNs, FCM)       |  | (attachments)    |  |
|  +--------+---------+  +--------------------+  +------------------+  |
+-----------|------------------------------------------------------+
            | Server-Server API (S2S / Federation)
            | HTTPS + mutual TLS (rustls)
+-----------|------------------------------------------------------+
|           v                                                      |
|  +------------------+                                            |
|  | Federation Recv  |  REMOTE HOME SERVER                       |
|  | (inbound S2S)    |                                            |
|  +------------------+                                            |
+------------------------------------------------------------------+
```

### 2.2 Component Responsibilities

#### Client Layer

| Component | Responsibility | Communicates With |
|-----------|---------------|-------------------|
| **iOS App** (reference client) | Human-facing UI. Renders messages, threads, agent cards, rich content, streaming responses. SwiftUI + @Observable + Swift Concurrency. | Home Server via C2S API |
| **Agent SDK** | Headless client for AI agents. Sends/receives messages, declares capabilities, streams responses. TypeScript primary, Python secondary. | Home Server via C2S API |
| **Protocol SDK** (shared library) | Core protocol logic: message serialization, crypto, event validation. Used by both iOS app and Agent SDK. | N/A (library) |

#### Server Layer

| Component | Responsibility | Communicates With |
|-----------|---------------|-------------------|
| **Connection Manager** | Maintains WebSocket connections (tokio-tungstenite) for real-time streaming. Upgrades HTTP to WS. Handles connection lifecycle, heartbeats, reconnection. | Clients (C2S), Stream Manager |
| **Auth & Identity** | Ed25519 keypair management (ring crate). Account registration. Session tokens. Server key signing. Identity verification. | Clients (C2S), Federation (S2S) |
| **Agent Registry** | Stores and serves Agent Cards (capability manifests). Handles capability queries. Serves `/.well-known/agent-card.json`. | Clients (C2S), Federation (S2S) |
| **Message Router** | Receives events from clients or federation. Routes to correct thread. Determines local vs. federated delivery. Uses tokio broadcast channels for fan-out. | Connection Mgr, Thread Mgr, Federation Sender |
| **Thread Manager** | Manages thread (room) state: membership, permissions, metadata. Handles join/leave/invite. | Message Router, Event Store |
| **Stream Manager** | Manages real-time streaming sessions. Buffers partial tokens from agents. Broadcasts stream chunks to thread participants via tokio channels. | Connection Mgr, Message Router |
| **Event Store** | Append-only persistent storage (PostgreSQL via sqlx) of all events. Supports pagination, backfill, search. Each thread has an ordered event log. | All server components |
| **Federation Sender** | Queues and delivers events to remote servers. Handles retries, backoff, transaction batching. Signs outgoing events with Ed25519. | Remote servers (S2S) |
| **Federation Receiver** | Accepts inbound events from remote servers. Validates signatures, checks authorization, passes to Message Router. | Remote servers (S2S), Message Router |
| **Push Gateway** | Sends push notifications (APNs for iOS, FCM for Android). Formats notification payloads. Respects user preferences. | iOS devices via APNs |
| **Media Store** | Stores and serves file attachments, images, artifacts. Content-addressable. CDN-ready. | Clients (C2S), Federation (S2S) |

### 2.3 The Three APIs

Following the universal federated pattern, the protocol defines three APIs:

#### API 1: Client-Server (C2S)

How clients (iOS app, agent SDKs) communicate with their home server.

**Transport:** HTTPS for request-response operations. WebSocket for real-time streaming and sync. JSON-RPC 2.0 as the message envelope (aligned with MCP and A2A).

**Capabilities:**
- Authentication (register, login, token refresh, key management)
- Thread operations (create, join, leave, invite, set permissions)
- Message sending (text, rich content blocks, agent UI, artifacts)
- Sync (receive new events since last sync -- inspired by Matrix `/sync`)
- Streaming (open a stream, send tokens, receive streamed responses)
- Agent discovery (query agent capabilities, browse agent registry)
- Media upload/download
- Push notification registration
- Presence (online, busy, away, offline)
- Typing indicators and read receipts

**Why WebSocket for C2S (not just HTTPS):**
Streaming agent responses require persistent connections. Matrix's long-polling `/sync` works but is inefficient. XMPP's persistent TCP stream is the right model, modernized as WebSocket. The C2S API should support both: HTTPS for simple operations (send message, fetch history) and WebSocket for real-time features (sync, streaming, presence, typing). HTTP + SSE as fallback when WebSocket is blocked.

#### API 2: Server-Server (S2S / Federation)

How home servers communicate with each other.

**Transport:** HTTPS with mutual TLS (rustls) and signed requests (inspired by Matrix's Federation API and email's SMTP).

**Capabilities:**
- Event exchange (push events from one server to another)
- State queries (request current thread state from another server)
- Backfill (request historical events to fill gaps)
- Key exchange (servers verify each other's signing keys)
- Agent card exchange (servers share agent capability information)
- Media transfer (proxied or direct)
- Server discovery (DNS SRV records + well-known URIs, like email MX records)

**Federation Model Decision -- Push-Based (like ActivityPub/SMTP), NOT Full Replication (like Matrix):**

Matrix replicates the full event DAG across all participating servers. This provides:
- Censorship resistance (no single server can delete history)
- Offline access to full history
- But: massive storage overhead, complex state resolution, state reset bugs

For our protocol, use push-based federation with selective backfill:
- Events are pushed from originating server to participating servers (like ActivityPub inbox delivery / SMTP relay)
- Servers store only the events they receive, not the full history
- Clients can request backfill (historical events) from any server that has them
- No DAG, no state resolution algorithm -- thread state is authoritative on the creating server (the "origin server" model)

**Rationale:** The DAG and state resolution are Matrix's most painful architectural decisions. State resolution v2 took years to get right and is still described as "counter-intuitive" by its creators. For a v1 protocol, the complexity cost outweighs the censorship-resistance benefit. An origin-server model (the server that created the thread is authoritative for state) is simpler, works for 99% of use cases, and can be evolved toward a more decentralized model later.

#### API 3: Agent Discovery

How agents and servers are discovered on the network.

**Transport:** HTTPS (standard web requests).

**Capabilities:**
- **Well-Known Discovery**: `GET /.well-known/ai-messaging` returns server configuration (endpoints, supported protocol version, public keys)
- **WebFinger**: `GET /.well-known/webfinger?resource=acct:agent@provider` returns the agent's profile (home server, public key, agent card URL)
- **Agent Cards**: JSON documents describing agent capabilities (skills, input/output types, supported interaction modes, authentication requirements) -- inspired by A2A Agent Cards
- **DNS SRV Records**: `_aim._tcp.provider.com` for server-to-server discovery (like XMPP's `_xmpp-server._tcp` and email's MX records)

---

## 3. Data Flow: How a Message Moves Through the System

### 3.1 Local Message (Same Server)

```
Human (iOS) types message in thread with Agent (same server)
    |
    v
iOS App serializes event -> WebSocket -> Connection Manager
    |
    v
Auth validates session token
    |
    v
Message Router: destination is local thread
    |
    v
Thread Manager: validates sender has permission to post
    |
    v
Event Store: appends event with sequential ID (PostgreSQL)
    |
    v
Connection Manager: broadcasts event to all connected
thread participants via their WebSocket connections
(tokio broadcast channel -> per-connection tasks)
    |
    v
Agent SDK receives event -> Agent processes -> Agent
sends response event (possibly streamed)
    |
    v
Stream Manager: if streaming, buffers tokens and
broadcasts chunks to thread participants in real-time
    |
    v
iOS App renders streamed tokens progressively
```

### 3.2 Federated Message (Cross-Server)

```
Human on Server A sends message to Agent on Server B
    |
    v
iOS App -> WebSocket -> Server A Connection Manager
    |
    v
Server A Auth validates, Message Router identifies
remote participant (agent@serverB.com)
    |
    v
Server A Thread Manager validates permissions
    |
    v
Server A Event Store appends event
    |
    v
Server A Federation Sender:
  1. Looks up Server B via DNS SRV / well-known
  2. Signs event with Server A's Ed25519 key (ring crate)
  3. POST event to Server B's federation endpoint (mutual TLS)
    |
    v
Server B Federation Receiver:
  1. Validates Server A's signature against known keys
  2. Validates event format and authorization
  3. Passes to Server B Message Router
    |
    v
Server B Event Store appends event
    |
    v
Server B Connection Manager delivers to
Agent SDK via WebSocket
    |
    v
Agent processes, sends streamed response back
through Server B -> Federation -> Server A -> iOS App
```

### 3.3 Streamed Agent Response (The Novel Flow)

This is the flow no existing federated protocol handles well. This is where we innovate.

```
Agent begins generating response
    |
    v
Agent SDK opens stream on home server:
  JSON-RPC method: "stream.start" {thread_id}
  Returns: stream_id
    |
    v
Agent SDK sends token chunks via WebSocket:
  JSON-RPC notification: "stream.chunk"
  {stream_id, tokens: "Here is the"}
  {stream_id, tokens: " analysis of"}
  {stream_id, tokens: " your data..."}
    |
    v
Stream Manager on agent's home server:
  - Buffers chunks (in-memory via dashmap)
  - Broadcasts to local participants via tokio broadcast
  - Pushes to federated servers for remote participants
    |
    v
Remote server's Connection Manager:
  - Forwards stream chunks to connected clients
    |
    v
iOS App renders tokens progressively with streaming cursor
(AsyncStream -> @Observable ViewModel -> SwiftUI)
    |
    v
Agent SDK closes stream:
  JSON-RPC method: "stream.complete"
  {stream_id, final_event: {full message event}}
    |
    v
Stream Manager finalizes: replaces stream with
a complete message event in the Event Store
```

**Critical design decision:** Streams are ephemeral. When complete, they collapse into a single immutable event in the Event Store. Clients that were online see the stream in real-time. Clients that come online later see the final message. This avoids the complexity of persisting partial stream state.

---

## 4. What the Protocol Specification Document Should Contain

Based on analysis of Matrix (5 APIs, 12 room versions, 2 crypto specs), XMPP (3 core RFCs + 400 XEPs), and ActivityPub (1 W3C Recommendation), the spec should be structured as follows:

### 4.1 Recommended Spec Structure

Published as a Docusaurus site with Markdown prose + JSON Schema definitions + AsyncAPI transport bindings.

```
AI-Native Messaging Protocol Specification
============================================

1. Introduction
   1.1 Goals and Non-Goals
   1.2 Protocol Overview (box-and-arrows diagram)
   1.3 Terminology and Conventions (MUST/SHOULD/MAY per RFC 2119)
   1.4 Versioning Policy

2. Architecture
   2.1 System Model (clients, servers, federation)
   2.2 Identifiers (thread IDs, event IDs, user/agent IDs)
   2.3 Identity Model (name@provider addressing)
   2.4 Threat Model

3. Data Model
   3.1 Events (base event schema, extensibility)
   3.2 Event Types (message, state, ephemeral, stream)
   3.3 Threads (lifecycle, membership, permissions)
   3.4 Agent Cards (capability manifests)
   3.5 Rich Content Blocks (typed content: text, code, table,
       form, chart, approval, artifact)
   3.6 Memory/Context Objects (active, conversational, archival)

4. Client-Server API
   4.1 Transport (HTTPS + WebSocket, JSON-RPC 2.0 envelope)
   4.2 Authentication (registration, login, key management)
   4.3 Sync (event delivery to clients)
   4.4 Thread Operations (CRUD, membership, permissions)
   4.5 Messaging (sending events, rich content)
   4.6 Streaming (opening, sending chunks, closing streams)
   4.7 Agent Discovery (querying capabilities)
   4.8 Media (upload, download, thumbnails)
   4.9 Presence (online state, typing, read receipts)
   4.10 Push Notifications (registration, payload format)

5. Server-Server API (Federation)
   5.1 Transport (HTTPS + mutual TLS)
   5.2 Server Discovery (DNS SRV, well-known)
   5.3 Server Authentication (key exchange, signatures)
   5.4 Event Exchange (pushing events, transaction model)
   5.5 Backfill (requesting historical events)
   5.6 State Queries (thread state from remote servers)
   5.7 Media Federation (proxying, direct transfer)

6. Agent Discovery API
   6.1 Well-Known URIs
   6.2 WebFinger Integration
   6.3 Agent Card Schema
   6.4 DNS SRV Records

7. Security
   7.1 Cryptographic Primitives (Ed25519, key derivation)
   7.2 Message Signing (canonical JSON per RFC 8785, then Ed25519)
   7.3 Identity Verification
   7.4 Anti-Prompt-Injection (trust annotations, content boundaries)
   7.5 End-to-End Encryption (future -- not v1)

8. Extension Mechanism
   8.1 How to Define Extensions (XMPP XEP-inspired model)
   8.2 Extension Namespacing
   8.3 Capability Negotiation
   8.4 Registry of Standard Extensions

Appendices
   A. JSON Schemas for All Data Types
   B. Error Codes and Error Handling
   C. Example Flows (with full JSON payloads)
   D. AsyncAPI Transport Binding Definitions
```

### 4.2 What to Defer vs. Include in v1

| Include in v1 | Defer to later |
|---------------|---------------|
| Core event model, thread model | End-to-end encryption (complex, needs Olm/Megolm equivalent) |
| C2S API (full) | Application Service API (bridges to other systems) |
| S2S Federation (basic) | Advanced federation (DAG-based replication, state resolution) |
| Agent Discovery + Cards | Agent marketplace/store |
| Streaming protocol | Voice/video |
| Rich content blocks | Custom renderer extensions |
| Ed25519 signing | Advanced key management (cross-signing, device verification) |
| Basic presence | Rich presence (status messages, activity sharing) |

---

## 5. How the Reference Server Should Be Structured

### 5.1 Technology Choice

Use **Rust with Axum** for the reference server. Rationale:
- Memory safety without GC pauses -- critical for real-time streaming where predictable p99 latency matters
- Matrix's Conduit homeserver proves Rust works for federated messaging (32 MiB RAM fresh install vs Synapse's hundreds)
- Compile-time concurrency safety prevents bugs that plague protocol implementations
- Native async/await via Tokio matches the event-driven architecture perfectly
- Excellent crypto ecosystem (ring crate for Ed25519, rustls for TLS)
- JSON handling via serde is fast and ergonomic despite being a systems language

See STACK.md for full technology justification and alternatives considered.

### 5.2 Server Module Structure

```
reference-server/
  src/
    main.rs                      # Entry point, Axum router setup
    config.rs                    # Server configuration (YAML-based)

    core/
      events.rs                  # Event model, validation, serialization
      threads.rs                 # Thread state, membership, permissions
      identity.rs                # Identity model, addressing
      crypto.rs                  # Ed25519 signing via ring, JCS canonicalization

    api/
      c2s/                       # Client-Server API handlers
        mod.rs                   # Axum router for C2S
        auth.rs                  # Registration, login, tokens
        sync.rs                  # Event delivery via WebSocket
        threads.rs               # Thread CRUD, membership
        messages.rs              # Send events, rich content
        streaming.rs             # Stream lifecycle management
        agents.rs                # Agent discovery, card queries
        media.rs                 # Upload, download, thumbnails
        presence.rs              # Online state, typing, receipts
      s2s/                       # Server-Server API handlers
        mod.rs                   # Axum router for S2S
        federation.rs            # Event exchange, transactions
        discovery.rs             # Well-known, keys
        backfill.rs              # Historical event requests
        state.rs                 # Thread state queries
      discovery/                 # Agent Discovery API
        wellknown.rs             # /.well-known/ handlers
        webfinger.rs             # WebFinger endpoint
        agentcards.rs            # Agent card serving

    transport/
      http.rs                    # Axum HTTPS server setup
      websocket.rs               # tokio-tungstenite connection management
      connection_manager.rs      # Client connection lifecycle, heartbeats

    federation/
      sender.rs                  # Outbound event delivery, queuing
      receiver.rs                # Inbound event validation
      resolver.rs                # DNS SRV, well-known resolution
      keystore.rs                # Remote server key management (dashmap)

    storage/
      mod.rs                     # Storage trait definitions
      event_store.rs             # Append-only event persistence (sqlx)
      thread_store.rs            # Thread state persistence
      user_store.rs              # Account data
      agent_store.rs             # Agent cards, capabilities
      media_store.rs             # File storage

    stream/
      stream_manager.rs          # Active stream tracking (dashmap)
      stream_buffer.rs           # Token chunk buffering
      stream_broadcaster.rs      # Fan-out via tokio broadcast channels

    push/
      apns.rs                    # Apple Push Notification Service

  migrations/                    # sqlx database migrations
  tests/
    unit/
    integration/
    federation/                  # Multi-server federation tests (docker-compose)
```

### 5.3 Database Design

Use **PostgreSQL 16+** with **sqlx** (compile-time verified SQL). The event store is the critical component.

**Core Tables:**

```sql
-- Accounts (both humans and agents)
accounts (
  id          UUID PRIMARY KEY,     -- UUIDv7 for time-ordered
  address     TEXT UNIQUE NOT NULL,  -- name@provider
  public_key  BYTEA NOT NULL,        -- Ed25519 public key
  created_at  TIMESTAMPTZ NOT NULL,
  account_type TEXT NOT NULL          -- 'human' | 'agent'
)

-- Threads (conversations)
threads (
  id            UUID PRIMARY KEY,
  origin_server TEXT NOT NULL,       -- the server that created this thread
  created_at    TIMESTAMPTZ NOT NULL,
  state         JSONB NOT NULL        -- current thread state snapshot
)

-- Thread membership
thread_members (
  thread_id   UUID REFERENCES threads(id),
  account_id  UUID,                   -- NULL for federated remote members
  address     TEXT NOT NULL,           -- canonical address
  role        TEXT NOT NULL,           -- 'admin' | 'member' | 'observer'
  joined_at   TIMESTAMPTZ NOT NULL,
  PRIMARY KEY (thread_id, address)
)

-- Events (the core append-only log)
events (
  id               UUID PRIMARY KEY,
  thread_id        UUID REFERENCES threads(id),
  sender           TEXT NOT NULL,      -- sender address
  type             TEXT NOT NULL,      -- 'message' | 'state' | 'stream.start' | ...
  content          JSONB NOT NULL,     -- the event payload
  origin_server_ts TIMESTAMPTZ NOT NULL, -- timestamp from origin server
  received_ts      TIMESTAMPTZ NOT NULL, -- timestamp on this server
  signature        BYTEA NOT NULL,     -- Ed25519 signature
  sequence         BIGINT NOT NULL     -- per-thread monotonic ordering
)
-- Index: (thread_id, sequence) for efficient pagination
-- Index: (thread_id, type) for state event queries

-- Agent capability cards
agent_cards (
  agent_address TEXT PRIMARY KEY,
  card          JSONB NOT NULL,
  updated_at    TIMESTAMPTZ NOT NULL
)

-- Federation key cache
federation_keys (
  server_name TEXT NOT NULL,
  key_id      TEXT NOT NULL,
  public_key  BYTEA NOT NULL,
  valid_until TIMESTAMPTZ NOT NULL,
  PRIMARY KEY (server_name, key_id)
)

-- Media storage metadata
media (
  id           TEXT PRIMARY KEY,      -- content-addressable hash
  content_type TEXT NOT NULL,
  size_bytes   BIGINT NOT NULL,
  storage_path TEXT NOT NULL,
  uploaded_by  TEXT NOT NULL
)
```

**Key decision: Linear ordering, NOT a DAG.** Events within a thread on a single server have a monotonic sequence number. When federated events arrive, they are assigned the next sequence number on the receiving server. This means different servers may have different orderings for concurrent events, but each server has a consistent, simple, queryable order. This avoids Matrix's state resolution complexity entirely.

---

## 6. How the iOS Client Should Be Architected

### 6.1 Architecture Pattern: MVVM + Repository

Following the updated STACK.md recommendation, use a four-layer architecture with MVVM + Repository pattern and @Observable for state management:

```
+------------------------------------------------------+
|  UI LAYER (SwiftUI Views)                            |
|  - ThreadListView, ThreadView, MessageBubble,        |
|    AgentCardView, StreamingMessageView, RichContent   |
+------------------------------------------------------+
|  PRESENTATION LAYER (ViewModels, @Observable)        |
|  - ThreadListViewModel, ThreadViewModel,              |
|    AgentBrowserViewModel                              |
+------------------------------------------------------+
|  DOMAIN LAYER (Repositories + Services)              |
|  - ThreadRepository, MessageRepository,               |
|    AuthService, StreamService, AgentDiscoveryService   |
+------------------------------------------------------+
|  PROTOCOL LAYER (SDK)                                |
|  - AIMClient (WebSocket + HTTP), EventSerializer,     |
|    CryptoService (CryptoKit Ed25519), ConnectionMgr   |
+------------------------------------------------------+
```

### 6.2 Module Structure

```
AINativeMessaging/
  App/
    AINativeMessagingApp.swift        # Entry point
    AppState.swift                    # @Observable root state (auth, connectivity)

  SDK/                                # Protocol SDK (separate Swift Package)
    Client/
      AIMClient.swift                 # Primary API interface
      WebSocketTransport.swift        # URLSessionWebSocketTask
      HTTPTransport.swift             # URLSession HTTPS requests
      SyncManager.swift               # Event sync loop (AsyncStream)
    Model/
      Event.swift                     # Core event types (Codable)
      Thread.swift                    # Thread model
      Identity.swift                  # Address, keys
      AgentCard.swift                 # Agent capabilities
      RichContent.swift               # Content block types
    Crypto/
      Signing.swift                   # CryptoKit Curve25519.Signing
      KeyStore.swift                  # Keychain-backed key storage
    Storage/
      LocalEventStore.swift           # GRDB.swift SQLite cache

  Features/
    Auth/
      LoginViewModel.swift            # @Observable
      LoginView.swift
      RegisterViewModel.swift
      RegisterView.swift
    ThreadList/
      ThreadListViewModel.swift       # @Observable
      ThreadListView.swift
      ThreadListItemView.swift
    Thread/
      ThreadViewModel.swift           # @Observable
      ThreadView.swift
      MessageViews/
        TextMessageView.swift
        StreamingMessageView.swift    # Live token rendering
        RichContentView.swift         # Forms, tables, code, charts
        AgentActivityView.swift       # Tool-use indicators
        ApprovalView.swift            # Human-in-the-loop
      InputBar/
        MessageInputView.swift
        AttachmentPicker.swift
    AgentDiscovery/
      AgentBrowserViewModel.swift
      AgentBrowserView.swift
      AgentCardDetailView.swift
    Settings/
      SettingsView.swift
      AccountView.swift

  Repositories/
    ThreadRepository.swift            # Network + local cache coordination
    MessageRepository.swift           # Event fetching, caching, pagination
    AgentRepository.swift             # Agent card fetching and caching

  Navigation/
    AppNavigationPath.swift           # NavigationStack + NavigationPath
    DeepLinkHandler.swift             # Open specific thread/agent

  Shared/
    Components/
      AvatarView.swift
      StatusBadge.swift
      LoadingIndicator.swift
    Extensions/
    Theme/
```

### 6.3 Key iOS Architecture Decisions

**State management: @Observable (iOS 17+)** -- simpler than ObservableObject, less boilerplate, Apple's recommended approach from WWDC 2024+. ViewModels are @Observable classes, views observe them automatically.

**Navigation: NavigationStack + NavigationPath** -- type-safe, programmatic navigation. Works for deep linking (open specific thread from push notification).

**Local storage: GRDB.swift** -- for caching events and thread state locally. Not CoreData (too heavy, poor concurrency model). Not SwiftData (immature -- no background tasks, poor large dataset performance). GRDB is lightweight, Swift-native, and performant for append-only event logs.

**Networking: URLSession + URLSessionWebSocketTask** -- no third-party HTTP libraries. Native Apple networking with built-in TLS, certificate pinning, and background session support.

**Async patterns: Swift Concurrency (async/await + AsyncStream)** -- not Combine (Apple has stopped evolving it) or RxSwift (third-party). AsyncStream is the natural fit for WebSocket message handling and streaming token delivery.

**Crypto: Apple CryptoKit** -- native Ed25519 via Curve25519.Signing. Hardware-accelerated. No third-party crypto dependency.

**Rich content rendering: Native SwiftUI components, NOT WebViews.** Following A2UI's "UI as data" principle, agents send JSON content blocks that the client maps to native SwiftUI components. This is faster, more accessible, and more secure than rendering HTML/WebViews. Define a component catalog (text, code, table, form, chart, button, approval) and render each natively.

**Streaming rendering:** Use a custom `StreamingTextView` that appends characters as they arrive via WebSocket/AsyncStream, with a blinking cursor. This is the single most important UX feature that differentiates us from every existing messaging app.

---

## 7. Key Architectural Decisions Summary

| Decision | Chosen | Rejected | Rationale |
|----------|--------|----------|-----------|
| Event ordering | Linear per-server sequence | DAG (Matrix-style) | DAG + state resolution is the #1 source of complexity and bugs in Matrix. Linear ordering is simpler, sufficient for v1, and can be evolved. |
| Federation model | Push-based with selective backfill | Full history replication (Matrix) | Avoids massive storage overhead. Origin server is authoritative. Simpler to implement and operate. |
| C2S transport | WebSocket + HTTPS (JSON-RPC 2.0) | Long-polling only (Matrix `/sync`) or TCP stream (XMPP) | WebSocket is the modern equivalent of XMPP's persistent stream. JSON-RPC 2.0 aligns with MCP and A2A. HTTPS for non-realtime operations. SSE as fallback. |
| S2S transport | HTTPS with signed requests + mutual TLS | Custom TCP (XMPP) or HTTPS only (ActivityPub) | HTTPS is firewall-friendly, well-understood, and allows request-level signing. Mutual TLS via rustls for server authentication. |
| Server discovery | DNS SRV + well-known | Custom registry, blockchain-based (ANP) | DNS is the proven approach used by email, XMPP, and Matrix. Well-known URIs add flexibility. |
| Identity format | `name@provider` | `@name:provider` (Matrix), JID with resource | Email-like is the most universally understood format. Resource/device identifiers go in session metadata, not the address. |
| Spec extensibility | Core spec + formal extensions (XMPP XEP model) | Monolithic spec (Matrix), informal extensions | Allows the protocol to grow without breaking changes. Extensions can be independently versioned and adopted. |
| Reference server | Rust / Axum | TypeScript, Go, Python | Predictable latency for streaming, memory safety for protocol correctness, proven by Conduit (Matrix's Rust homeserver). See STACK.md. |
| iOS architecture | MVVM + Repository with @Observable | TCA, Coordinator+MVVM, VIPER | @Observable is Apple's modern pattern. Repository pattern cleanly separates network + cache. Simpler than TCA, more structured than bare MVVM. |
| Rich content | Native SwiftUI components from JSON schema | WebViews, HTML rendering | Faster, more secure (no code injection), better accessibility, follows A2UI principle. |
| Thread state authority | Origin server authoritative | Distributed consensus, DAG-based | Dramatically simpler. One server is the source of truth for thread state. If that server goes down, the thread is read-only on other servers until it returns. Acceptable trade-off for v1. |

---

## 8. Build Order (Dependencies)

The components have clear dependency chains that dictate build order:

### Phase 1: Protocol Specification (No Dependencies)
1. **Data Model** -- Event schema, thread model, identity model, agent card schema (JSON Schema files)
2. **Crypto** -- Ed25519 signing/verification specification (canonical JSON + signature format)
3. **Protocol Spec Draft** -- Sections 1-3 (Introduction, Architecture, Data Model) + AsyncAPI transport bindings
4. **Spec Website** -- Docusaurus site with published schemas

### Phase 2: Single-Server (Depends on Phase 1)
5. **Event Store** -- PostgreSQL schema + sqlx queries (the foundation everything else reads/writes)
6. **C2S API: Auth** -- Registration, login, session management (Axum handlers)
7. **C2S API: Threads** -- Create, join, send messages
8. **C2S API: Sync** -- Deliver events to connected clients (WebSocket via tokio-tungstenite)
9. **Basic iOS Client** -- Login, thread list, send/receive text messages (SwiftUI + GRDB)

### Phase 3: Agent Features (Depends on Phase 2)
10. **Streaming Protocol** -- Stream lifecycle, token buffering, fan-out (tokio broadcast)
11. **Agent Registry** -- Agent cards, capability queries
12. **Rich Content Blocks** -- Define content block types, iOS renderers
13. **Agent SDK** -- TypeScript headless client library for agents
14. **iOS: Streaming UI** -- Render streamed responses progressively (AsyncStream -> @Observable)
15. **iOS: Rich Content** -- Native SwiftUI renderers for all content block types

### Phase 4: Federation (Depends on Phase 2)
16. **S2S API** -- Event exchange, server authentication, key management (mutual TLS via rustls)
17. **Server Discovery** -- DNS SRV, well-known, WebFinger
18. **Federation Sender/Receiver** -- Queue, deliver, validate remote events
19. **Backfill** -- Request historical events from remote servers
20. **Federation Tests** -- Multi-server integration test suite (docker-compose with 2+ servers)

### Phase 5: Production Hardening (Depends on Phases 3+4)
21. **Push Notifications** -- APNs integration
22. **Media Store** -- File upload/download, thumbnails
23. **Presence** -- Online state, typing indicators, read receipts
24. **Memory/Context** -- Hierarchical context management (active, conversational, archival)
25. **Protocol Spec v1.0** -- Complete, reviewed specification document
26. **iOS Polish** -- Full feature parity with spec, performance optimization

### Dependency Graph

```
Phase 1: Spec + Data Model + Crypto + AsyncAPI + Docusaurus
    |
    v
Phase 2: Event Store -> Auth -> Threads -> Sync -> Basic iOS
    |                                          |
    +---> Phase 3: Streaming -> Agent          |
    |     Registry -> Rich Content ->          |
    |     Agent SDK -> iOS Streaming/Rich      |
    |                                          |
    +---> Phase 4: S2S API -> Discovery ->     |
          Fed Sender/Recv -> Backfill          |
                    |                          |
                    v                          v
              Phase 5: Push + Media + Presence + Memory + Spec v1.0
```

**Critical path:** Phases 3 and 4 can run in parallel. The agent features (streaming, rich content) and federation are independent concerns that converge in Phase 5. This parallelism is a significant advantage of the architecture.

---

## 9. Anti-Patterns to Avoid

### Anti-Pattern 1: Matrix's State Resolution Complexity
**What:** Implementing a DAG-based event graph with algorithmic state resolution.
**Why bad:** Matrix's state resolution v1 took years to stabilize and still had critical bugs (state resets). V2 is functional but "counter-intuitive" by the creators' own admission. For a new protocol, this is an enormous complexity tax with diminishing returns.
**Instead:** Use origin-server authority for thread state. Simple, predictable, debuggable.

### Anti-Pattern 2: XMPP's XML Verbosity
**What:** Using XML for the wire format.
**Why bad:** XML parsing is heavier, XML schemas are more complex to write and validate, and JSON has won the ecosystem (every LLM, every API, every modern protocol uses JSON).
**Instead:** JSON everywhere. JSON Schema for validation. Content-type negotiation if binary efficiency is needed later (MessagePack, CBOR, or optional Protobuf encoding).

### Anti-Pattern 3: ActivityPub's Missing Conversation Model
**What:** Relying on reply chains to reconstruct conversations instead of having a first-class thread/room concept.
**Why bad:** ActivityPub threads are notoriously broken in Mastodon -- you frequently see incomplete conversations because your server only has the replies it received. This is fundamentally unsuitable for messaging.
**Instead:** Threads are first-class objects with explicit membership. All participants receive all events.

### Anti-Pattern 4: Slack's Undocumented Payload Limits
**What:** Having implicit, undocumented limits that developers discover only at runtime.
**Why bad:** Block Kit's real limit is ~13KB, not the documented 50 blocks. This causes silent failures in production.
**Instead:** Every limit is explicit in the spec. Error responses include the specific limit that was exceeded.

### Anti-Pattern 5: Over-Engineering the Identity System
**What:** Building W3C DID / blockchain-based decentralized identity for v1 (as ANP proposes).
**Why bad:** Adds enormous complexity for a theoretical benefit. No consumer messaging app has succeeded with blockchain-based identity.
**Instead:** `name@provider` addressing with Ed25519 keypairs. Simple, familiar, works. Decentralized identity can be layered on later as an extension.

### Anti-Pattern 6: Synapse's Python/Twisted Performance Issues
**What:** Choosing a garbage-collected runtime for a real-time messaging server.
**Why bad:** GC pauses spike p99 latency during streaming. Synapse's Python/Twisted architecture is the primary reason Matrix moved to the Rust SDK. Higher per-connection memory means fewer concurrent connections per server.
**Instead:** Rust with Tokio provides predictable latency, minimal per-connection overhead, and compile-time safety for concurrent protocol handling.

---

## Sources

### Specifications and Official Documentation
- [Matrix Specification v1.18](https://spec.matrix.org/latest/) -- MEDIUM confidence (reviewed spec structure, not all details)
- [Matrix Server-Server API](https://spec.matrix.org/v1.6/server-server-api/) -- HIGH confidence (reviewed federation model)
- [XMPP Core RFC 6120](https://xmpp.org/rfcs/rfc6120.html) -- HIGH confidence (well-established standard)
- [ActivityPub W3C Recommendation](https://www.w3.org/TR/activitypub/) -- HIGH confidence (W3C standard)
- [Agent Messaging Protocol (AMP)](https://github.com/agentmessaging/protocol) -- MEDIUM confidence (reviewed spec structure and addressing)
- [A2A Protocol](https://github.com/a2aproject/A2A) -- MEDIUM confidence (reviewed Agent Cards and task model)
- [A2UI Spec](https://a2ui.org/) -- MEDIUM confidence (reviewed UI-as-data approach)
- [RFC 2360 - Guide for Internet Standards Writers](https://datatracker.ietf.org/doc/html/rfc2360) -- HIGH confidence (IETF standard)
- [RFC 4101 - Writing Protocol Models](https://www.rfc-editor.org/rfc/rfc4101) -- HIGH confidence (IETF standard)
- [RFC 8785 - JSON Canonicalization Scheme](https://www.rfc-editor.org/rfc/rfc8785) -- HIGH confidence (IETF standard for deterministic JSON signing)

### Architecture References
- [Synapse Architecture Overview (DeepWiki)](https://deepwiki.com/matrix-org/synapse/1.2-architecture-overview) -- HIGH confidence (detailed codebase analysis)
- [Element X iOS Architecture (DeepWiki)](https://deepwiki.com/element-hq/element-x-ios/1-overview) -- HIGH confidence (detailed codebase analysis)
- [Matrix State Resolution v2](https://matrix.org/docs/older/stateres-v2/) -- HIGH confidence (official Matrix documentation)
- [Matrix Decomposition (academic analysis)](https://matrix.org/blog/2020/06/16/matrix-decomposition-an-independent-academic-analysis-of-matrix-state-resolution/) -- HIGH confidence (peer-reviewed)
- [Matrix vs XMPP Comparison (ProcessOne)](https://www.process-one.net/blog/xmpp-matrix/) -- MEDIUM confidence (vendor perspective, but technically detailed)
- [Conduit (Rust Matrix homeserver)](https://conduit.rs/) -- HIGH confidence (proves Rust viability for federated messaging)

### Implementation References
- [Element X iOS GitHub](https://github.com/element-hq/element-x-ios) -- HIGH confidence (active reference implementation)
- [Matrix Rust SDK](https://github.com/element-hq/matrix-rust-components-swift) -- HIGH confidence (production SDK)
- [Email Architecture (IETF draft)](https://bbiw.net/specifications/draft-crocker-email-arch-03.html) -- HIGH confidence (IETF author)
- [AsyncAPI v3.1.0 Specification](https://www.asyncapi.com/docs/reference/specification/v3.1.0) -- HIGH confidence (standard for event-driven API specs)
