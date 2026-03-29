# Architecture Patterns: Federation Integration

**Domain:** Federated AI-native messaging -- integrating multi-server federation into existing single-server Axum architecture
**Researched:** 2026-03-28
**Overall confidence:** HIGH (based on existing spec, Matrix/XMPP federation precedents, current codebase analysis)

---

## 1. Current Architecture Baseline

The existing demo server (`demo/src/server.rs`) is a single-process Axum application with these components:

```
                          Axum Router
                         /          \
                        /            \
              WebSocket Handler    HTTP Handler
              (ws_upgrade_handler) (agent_card_handler)
                      |
              handle_ws_connection
                   /        \
    handle_agent_connection  handle_human_connection
```

**Current state (in-memory, no persistence):**

| Component | Implementation | What It Does |
|-----------|---------------|--------------|
| `ServerState` | Struct with `Clone` | Holds all shared state |
| `IdentityStore` | `Arc<Mutex<HashMap>>` | Maps `address -> IdentityRecord` |
| `to_agent_tx` / `to_human_tx` | `broadcast::Sender<String>` | Routes messages between agent and human connections |
| `threads` | `Arc<Mutex<Vec<String>>>` | Tracks active thread IDs |
| `archival_store` | `Arc<Mutex<HashMap>>` | In-memory archival fact storage |

**Critical observation:** The current routing is hard-coded for exactly two participants (one human, one agent) using broadcast channels. There is no concept of "which server does this participant live on?" -- all participants are local. Federation requires fundamentally rethinking message routing.

### 1.1 What Can Be Preserved

| Component | Keep? | Rationale |
|-----------|-------|-----------|
| Axum Router structure | **YES** | Add federation routes alongside existing WebSocket/HTTP routes |
| WebSocket handshake flow | **YES** | Client-to-server protocol is unchanged by federation |
| Ed25519 signing/verification | **YES** | Federation adds server-level signatures on top of existing message signatures |
| JSON-RPC framing | **YES** | Both C2S and S2S use JSON-RPC 2.0 |
| `IdentityStore` interface | **MODIFY** | Must add remote identity resolution (HTTP fetch from `.well-known`) |
| `broadcast::Sender` routing | **REPLACE** | Cannot route to remote servers. Need participant-addressed routing |
| In-memory `threads` | **REPLACE** | Threads need persistent storage with origin-server metadata |

### 1.2 What Must Be Added

| New Component | Purpose |
|---------------|---------|
| **Federation Router** | Axum handler for `POST /ainm/v1/relay` endpoint |
| **Server Identity** | `GET /.well-known/ainm/server` endpoint + server Ed25519 keypair |
| **Message Router** | Decides local delivery vs remote relay per-participant |
| **Federation Sender** | Outbound queue with per-destination retry logic |
| **Remote Identity Resolver** | HTTP client to fetch `.well-known/ainm/{name}` from remote providers |
| **Thread Registry** | Persistent thread metadata with origin-server tracking |
| **Offline Queue** | Store-and-forward for messages to offline local participants |
| **Backfill Handler** | Serves thread history to remote servers requesting it |
| **mTLS Client/Server** | Mutual TLS for all server-to-server connections |

---

## 2. Federation Architecture Design

### 2.1 Recommended Architecture

```
                              AINM Server
  ================================================================
  |                                                              |
  |  +------------------+    +------------------+               |
  |  | WebSocket        |    | HTTP             |               |
  |  | Handler (C2S)    |    | Handlers         |               |
  |  |                  |    |                  |               |
  |  | - ainm.initialize|    | - /.well-known/  |               |
  |  | - ainm.message   |    |   ainm/{name}    |               |
  |  |   .send          |    | - /.well-known/  |               |
  |  | - ainm.thread    |    |   ainm/server    |               |
  |  |   .create        |    | - /.well-known/  |               |
  |  | - ainm.stream.*  |    |   ainm/register  |               |
  |  | - ainm.memory.*  |    | - /ainm/v1/relay |               |
  |  +--------+---------+    | - /ainm/v1/      |               |
  |           |               |   backfill       |               |
  |           |               +--------+---------+               |
  |           v                        v                         |
  |  +------------------------------------------------+         |
  |  |              Message Router                     |         |
  |  |                                                 |         |
  |  |  For each participant in thread:                |         |
  |  |    - Local + online? -> deliver via WebSocket   |         |
  |  |    - Local + offline? -> queue in offline store |         |
  |  |    - Remote? -> enqueue in federation sender    |         |
  |  +----------+--------------------+---------+------+         |
  |             |                    |         |                 |
  |    +--------v------+    +-------v----+ +--v-----------+    |
  |    | Local Delivery |    | Offline    | | Federation   |    |
  |    | (Session Mgr)  |    | Queue      | | Sender       |    |
  |    |                |    |            | |              |    |
  |    | DashMap:       |    | Per-user   | | Per-server   |    |
  |    |  session_id -> |    | message    | | destination  |    |
  |    |  ws_sender     |    | queue      | | queue with   |    |
  |    +----------------+    +------------+ | retry/backoff|    |
  |                                         +---------+---+    |
  |                                                   |         |
  |                                          +--------v---+    |
  |                                          | HTTP Client |    |
  |                                          | (mTLS)      |    |
  |                                          | reqwest +   |    |
  |                                          | rustls      |    |
  |                                          +-------------+    |
  |                                                              |
  |  +------------------------------------------------+         |
  |  |              Thread Registry                    |         |
  |  |                                                 |         |
  |  |  thread_id -> {                                 |         |
  |  |    origin_server,                               |         |
  |  |    participants: [{identity, home_server}],     |         |
  |  |    created_at, title, parent_thread_id          |         |
  |  |  }                                              |         |
  |  +------------------------------------------------+         |
  |                                                              |
  |  +------------------------------------------------+         |
  |  |              Identity Resolver                  |         |
  |  |                                                 |         |
  |  |  Local: HashMap lookup (existing IdentityStore) |         |
  |  |  Remote: HTTPS GET -> cache with TTL            |         |
  |  +------------------------------------------------+         |
  ================================================================
```

### 2.2 Component Boundaries

| Component | Responsibility | Communicates With |
|-----------|---------------|-------------------|
| **WebSocket Handler** (existing, modified) | Client connections, JSON-RPC dispatch, session lifecycle | Message Router, Thread Registry, Session Manager |
| **Federation Relay Handler** (new) | Receives `ainm.relay` from remote servers, validates triple-layer security | Message Router, Identity Resolver, Server Identity |
| **Message Router** (new, core) | Determines delivery path for each message per-participant | Session Manager, Offline Queue, Federation Sender, Thread Registry |
| **Session Manager** (new, replaces broadcast channels) | Tracks active WebSocket sessions by participant identity | WebSocket Handler |
| **Federation Sender** (new) | Per-destination outbound queues, retry with exponential backoff, batch messages | HTTP Client, Server Identity Store |
| **Thread Registry** (new, replaces `Vec<String>`) | Thread metadata, participant lists, origin-server tracking | Message Router, Backfill Handler |
| **Identity Resolver** (modified from IdentityStore) | Local identity lookup + remote HTTPS resolution with caching | All components needing identity data |
| **Server Identity** (new) | This server's Ed25519 keypair, TLS cert fingerprint, served at `/.well-known/ainm/server` | Federation Relay Handler, Federation Sender |
| **Offline Queue** (new) | Persists messages for offline local participants, drains on reconnect | Message Router, WebSocket Handler (on reconnect) |
| **Backfill Handler** (new) | Serves thread history to remote servers | Thread Registry, Message Store |
| **mTLS HTTP Client** (new) | Makes outbound federation requests with mutual TLS | Federation Sender |

---

## 3. Data Flow: Federated Message Lifecycle

### 3.1 Local Sender to Remote Recipient

```
Alice@server-a.com sends message to thread with Bob@server-b.com

1. Alice's client -> WebSocket -> server-a.com
   |  ainm.message.send { from: "alice@server-a.com", thread_id: "..." }

2. Server A: WebSocket Handler
   |  Verify Alice's Ed25519 signature (existing logic)
   |  Respond: { accepted: true, server_timestamp: "..." }

3. Server A: Message Router
   |  Look up thread participants from Thread Registry
   |  For each participant:
   |    alice@server-a.com -> local, online -> Session Manager delivers
   |    bob@server-b.com   -> remote -> Federation Sender queue

4. Server A: Federation Sender
   |  Resolve server-b.com server identity (GET /.well-known/ainm/server)
   |  Build relay payload: { messages: [...], from_server, server_signature }
   |  Sign relay payload with Server A's Ed25519 key
   |  POST https://server-b.com/ainm/v1/relay (mTLS)

5. Server B: Federation Relay Handler
   |  Validate mTLS certificate fingerprint
   |  Verify server_signature against server-a.com's public key
   |  Verify Alice's message signature against alice@server-a.com's public key
   |  Pass to Message Router

6. Server B: Message Router
   |  bob@server-b.com -> local, online -> Session Manager delivers
   |  bob@server-b.com -> local, offline -> Offline Queue stores

7. Server B -> Bob's client: ainm.message.deliver { ... }
```

### 3.2 Thread Creation Across Servers

```
Alice@server-a.com creates thread with Bob@server-b.com

1. Alice's client -> ainm.thread.create {
     thread_id: "...",
     participants: ["alice@server-a.com", "bob@server-b.com"]
   }

2. Server A: WebSocket Handler
   |  For each participant, resolve identity:
   |    alice@server-a.com -> local lookup
   |    bob@server-b.com   -> remote HTTPS fetch /.well-known/ainm/bob
   |  Validate all identities exist

3. Server A: Thread Registry
   |  Store thread with origin_server = "server-a.com"
   |  Participants include home_server for each identity

4. Server A: Federation Sender
   |  Send thread.create event to server-b.com via relay
   |  (As a message with type: "thread.create")

5. Server B: Federation Relay Handler
   |  Receive thread.create, store in own Thread Registry
   |  Mark origin_server = "server-a.com" (Server B is NOT the authority)
   |  Deliver thread.create notification to Bob if online
```

### 3.3 Routing Decision Flowchart

```
Message arrives (from client OR from federation relay)
  |
  v
Verify sender signature
  |
  v
Look up thread_id in Thread Registry
  |-- Not found?
  |     |-- From local client? -> Error -32024 (thread not found)
  |     |-- From federation relay? -> Accept if it's a thread.create,
  |           otherwise request backfill from origin server
  |
  v
For each participant in thread:
  |
  +-- Is participant local? (provider matches our domain)
  |     |
  |     +-- Is participant online? (session exists in Session Manager)
  |     |     |-- YES -> deliver via WebSocket (ainm.message.deliver)
  |     |     |-- NO  -> enqueue in Offline Queue
  |     |
  +-- Is participant remote?
        |-- Group by home_server domain
        |-- Enqueue in Federation Sender (per-destination queue)
```

---

## 4. Lessons from Matrix and XMPP

### 4.1 What Matrix Gets Right (Adopt)

| Pattern | Matrix Implementation | AINM Adaptation |
|---------|----------------------|-----------------|
| **Per-destination send queues** | Synapse's `per_destination_queue.py` -- each remote server has its own outbound queue with independent retry/backoff state | Federation Sender maintains a `DashMap<ServerDomain, DestinationQueue>`. Each queue has its own backoff timer. If server-b.com is down, messages to server-c.com are unaffected. |
| **Layered validation** | mTLS + server signatures + event signatures (3 layers) | Already specified in transport.md Section 5.4. Implement exactly as specced. |
| **Server identity at well-known URL** | `/.well-known/matrix/server` for delegation | Already specced at `/.well-known/ainm/server`. |
| **Backfill API** | `GET /_matrix/federation/v1/backfill/{roomId}` returns events before a given point | Implement `GET /ainm/v1/backfill/{thread_id}` returning messages in reverse chronological order from a given message_id. |
| **Transaction batching** | Up to 50 PDUs per transaction | Batch up to 50 messages per relay request. Reduces HTTP overhead for chatty threads. |
| **Caching remote server identity** | 1-hour TTL on server key responses | Cache `/.well-known/ainm/server` responses per the Cache-Control header, default 1 hour. |

### 4.2 What Matrix Gets Wrong (Avoid)

| Anti-Pattern | Matrix Problem | AINM Design |
|-------------|----------------|-------------|
| **DAG-based event ordering** | Events form a DAG with `prev_events` references. Fork/merge creates state resolution nightmares. State resets plagued early Matrix. State Resolution v2 is 2000+ lines of complex algorithm. | **Use origin-server authority instead.** Threads have a single authoritative server (where the thread was created). That server assigns `server_timestamp` ordering. No DAG, no forks, no state resolution needed. |
| **Full room state replication** | Every server in a room gets a full copy of all state events. Joining a large room downloads megabytes of state. | **Selective state.** Remote servers store thread metadata (participants, title, origin) but do NOT replicate full thread state. They request backfill on demand. |
| **Complex join handshake** | `make_join` / `send_join` is a multi-step dance involving auth chains, state snapshots, and room version negotiation. | **Simple join via relay.** Thread creator's server is authoritative. To add a remote participant, the origin server sends a `thread.create` or `thread.join` event via normal relay. No handshake dance. |
| **Unbounded state accumulation** | Rooms accumulate state events forever. Large rooms (Matrix HQ) have enormous state. | **Bounded thread metadata.** Thread metadata is the participant list + title. Memory/archival is per-server, not replicated via federation. |

### 4.3 What XMPP Gets Right (Adopt)

| Pattern | XMPP Implementation | AINM Adaptation |
|---------|---------------------|-----------------|
| **Persistent S2S connections** (optional) | XMPP maintains long-lived TCP connections between servers for high-traffic pairs | Consider connection pooling (not persistent WebSocket but HTTP/2 connection reuse via reqwest) for high-volume server pairs. Start with stateless HTTP POST. |
| **Simple identity model** | `user@domain` -- domain IS the server | Already using `name@provider`. Provider domain hosts `.well-known` endpoints. |
| **SRV DNS records** for server discovery | `_xmpp-server._tcp.example.com` | Optional future enhancement. For v1, `/.well-known/ainm/server` is sufficient. |

### 4.4 What XMPP Gets Wrong (Avoid)

| Anti-Pattern | XMPP Problem | AINM Avoidance |
|-------------|--------------|----------------|
| **Dialback verification** | Weak authentication via DNS -- subject to DNS poisoning | Use mTLS with certificate fingerprint pinning (already specced in transport.md) |
| **XML stanza complexity** | Verbose, hard to parse, poor tooling | JSON-RPC 2.0 -- already decided and correct |
| **No message history federation** | XMPP has no built-in backfill protocol for federated chat history | Backfill API is a first-class federation feature |

---

## 5. Thread Ownership Model: Origin-Server Authority

This is the most critical architectural decision for AINM federation. The choice is between Matrix-style distributed authority (DAG + state resolution) and origin-server authority.

**Recommendation: Origin-server authority.** The server where a thread was created is the authoritative source for that thread's state.

### 5.1 How It Works

```
Thread created on server-a.com:

  server-a.com (ORIGIN)           server-b.com (PARTICIPANT)
  ========================        ========================
  Thread Registry:                Thread Registry:
    thread_id: "abc-123"            thread_id: "abc-123"
    origin_server: self             origin_server: "server-a.com"
    participants: [                 participants: [
      alice@server-a.com,             alice@server-a.com,
      bob@server-b.com                bob@server-b.com
    ]                               ]
    messages: [full history]        messages: [locally cached]

  Authority:                      Authority:
    - Assigns server_timestamp      - Caches participant list
    - Authoritative participant     - Requests backfill from
      list                            server-a.com on demand
    - Accepts/rejects joins         - Forwards thread ops to
    - Serves backfill                  server-a.com
```

### 5.2 Origin-Server Responsibilities

| Operation | Origin Server | Participant Server |
|-----------|--------------|-------------------|
| **Message ordering** | Assigns `server_timestamp` to all messages (local and relayed) | Uses origin's `server_timestamp` for ordering |
| **Participant management** | Authoritative add/remove of participants | Forwards `thread.join`/`thread.leave` to origin via relay |
| **Thread metadata** | Authoritative title, settings | Caches metadata, refreshes from origin |
| **Message storage** | Full message history | Stores messages it has seen (for local participant access) |
| **Backfill** | Serves history via `/ainm/v1/backfill/{thread_id}` | Requests history from origin when client requests older messages |

### 5.3 Why Not Distributed Authority (Matrix-Style)

| Criterion | Origin-Server Authority | Distributed (DAG) |
|-----------|------------------------|-------------------|
| **Complexity** | Simple -- one server decides | Extremely complex state resolution algorithms |
| **Consistency** | Strong -- single source of truth | Eventual -- forks must merge |
| **Message ordering** | Total order via `server_timestamp` | Partial order via DAG, requires resolution |
| **Failure mode** | Origin server down = thread read-only temporarily | Any server can accept messages, but conflicts accumulate |
| **Implementation effort** | Weeks | Months (Matrix state resolution is 2000+ LOC of edge cases) |
| **Trade-off** | Single point of failure per thread | No single point of failure but complex failure modes |

**The trade-off is acceptable for v1.** If the origin server goes down, the thread becomes temporarily unavailable for new messages. Participant servers still have cached messages for offline reading. This is the same model as email -- if gmail.com goes down, you cannot send FROM gmail, but existing messages are cached locally.

### 5.4 Future Migration Path

If origin-server authority proves too limiting (e.g., origin server permanently lost), a future protocol version can add:
- **Thread migration:** Transfer origin-server authority to another server
- **Multi-master writes:** Allow participant servers to accept messages during origin outage, with reconciliation on recovery

These are additive -- they do not require rearchitecting the v1 design.

---

## 6. New Components: Detailed Design

### 6.1 Message Router (Core New Component)

The Message Router replaces the current `broadcast::Sender` pattern with participant-aware routing.

```rust
// Conceptual structure -- not literal implementation
pub struct MessageRouter {
    session_manager: SessionManager,
    federation_sender: FederationSender,
    offline_queue: OfflineQueue,
    thread_registry: ThreadRegistry,
    local_domain: String,
}

impl MessageRouter {
    /// Route a validated message to all thread participants
    pub async fn route(&self, message: &SignedMessage) -> Result<()> {
        let thread = self.thread_registry.get(&message.thread_id)?;

        for participant in &thread.participants {
            let provider = participant.identity.split('@').nth(1).unwrap();

            if provider == self.local_domain {
                // Local participant
                if self.session_manager.is_online(&participant.identity) {
                    self.session_manager.deliver(&participant.identity, message).await?;
                } else {
                    self.offline_queue.enqueue(&participant.identity, message).await?;
                }
            } else {
                // Remote participant -- group by server and batch
                let home_server = &participant.home_server;
                self.federation_sender.enqueue(home_server, message).await?;
            }
        }
        Ok(())
    }
}
```

**Key design decisions:**
- The router does NOT know about WebSocket specifics -- it delegates to `SessionManager`
- Remote messages are grouped by destination server, not individual recipient
- The router is called from both the WebSocket handler (client sends) AND the Federation Relay handler (remote server sends)

### 6.2 Session Manager (Replaces Broadcast Channels)

```rust
pub struct SessionManager {
    // Maps participant identity -> active WebSocket sender
    sessions: DashMap<String, Arc<Mutex<SplitSink<WebSocket, Message>>>>,
}

impl SessionManager {
    pub fn register(&self, identity: &str, sender: Arc<Mutex<SplitSink<WebSocket, Message>>>);
    pub fn unregister(&self, identity: &str);
    pub fn is_online(&self, identity: &str) -> bool;
    pub async fn deliver(&self, identity: &str, message: &SignedMessage) -> Result<()>;
}
```

**Why DashMap instead of broadcast channels:** The current architecture uses two broadcast channels (`to_agent_tx`, `to_human_tx`) which hard-codes exactly two participants. `DashMap` supports any number of concurrent sessions with lock-free reads. This is the pattern Conduit uses for connection tracking.

### 6.3 Federation Sender (Per-Destination Queues)

Modeled after Synapse's `per_destination_queue.py` and Conduit's `sending::Service`:

```rust
pub struct FederationSender {
    // Per-destination outbound queues
    queues: DashMap<String, DestinationQueue>,
    server_identity: ServerIdentity,
    http_client: reqwest::Client,  // configured with mTLS
}

pub struct DestinationQueue {
    messages: VecDeque<SignedMessage>,
    state: DestinationState,
    last_attempt: Option<Instant>,
    retry_count: u32,
}

pub enum DestinationState {
    Ready,
    Sending,
    BackingOff { until: Instant },
    Failed { reason: String },
}
```

**Retry strategy (matching Matrix best practices):**
1. Immediate first attempt
2. On failure: exponential backoff starting at 1s, doubling to max 60s
3. After 10 consecutive failures: mark destination as `Failed`, stop retrying
4. Background task periodically retries `Failed` destinations every 5 minutes
5. Successful send resets backoff to `Ready`

**Batching:** Queue accumulates messages. When a send attempt fires, batch up to 50 messages into a single `ainm.relay` request. This is critical for chatty threads -- sending one HTTP request per message would be prohibitively expensive.

### 6.4 Thread Registry

```rust
pub struct ThreadRegistry {
    threads: DashMap<String, FederatedThread>,
}

pub struct FederatedThread {
    pub thread_id: String,
    pub title: Option<String>,
    pub origin_server: String,  // Domain of the server where thread was created
    pub created_at: String,
    pub created_by: String,
    pub parent_thread_id: Option<String>,
    pub parent_message_id: Option<String>,
    pub participants: Vec<FederatedParticipant>,
}

pub struct FederatedParticipant {
    pub identity: String,       // e.g., "alice@server-a.com"
    pub participant_type: String, // "human" or "agent"
    pub home_server: String,    // e.g., "server-a.com"
    pub joined_at: String,
    pub role: String,           // "creator" or "member"
}
```

**Key addition vs current:** Every participant now carries a `home_server` field, derived from the identity's provider domain during identity resolution. This is what the Message Router uses to decide local vs remote delivery.

### 6.5 Identity Resolver (Extended)

```rust
pub struct IdentityResolver {
    // Local identities (registered on this server)
    local_store: DashMap<String, IdentityRecord>,
    // Cached remote identities with TTL
    remote_cache: DashMap<String, CachedIdentity>,
    http_client: reqwest::Client,
    local_domain: String,
}

pub struct CachedIdentity {
    pub record: IdentityRecord,
    pub fetched_at: Instant,
    pub ttl: Duration,  // from Cache-Control header, default 1 hour
}

impl IdentityResolver {
    /// Resolve an identity -- local lookup first, then remote HTTPS fetch
    pub async fn resolve(&self, address: &str) -> Result<IdentityRecord> {
        let provider = address.split('@').nth(1).ok_or(Error::InvalidAddress)?;

        if provider == self.local_domain {
            // Local lookup
            self.local_store.get(address)
                .map(|r| r.clone())
                .ok_or(Error::IdentityNotFound)
        } else {
            // Check cache first
            if let Some(cached) = self.remote_cache.get(address) {
                if cached.fetched_at.elapsed() < cached.ttl {
                    return Ok(cached.record.clone());
                }
            }
            // Fetch from remote provider
            let url = format!("https://{}/.well-known/ainm/{}", provider, name);
            let record = self.http_client.get(&url).send().await?.json().await?;
            self.remote_cache.insert(address.to_string(), CachedIdentity {
                record: record.clone(),
                fetched_at: Instant::now(),
                ttl: Duration::from_secs(3600),
            });
            Ok(record)
        }
    }
}
```

### 6.6 Backfill Protocol

When a remote server needs thread history (e.g., a new participant joins and their client scrolls up):

```
GET /ainm/v1/backfill/{thread_id}?before={message_id}&limit={n}

Response:
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "messages": [ ... ],  // Signed message envelopes, reverse chronological
    "has_more": true
  }
}
```

**Access control:** Only servers with at least one participant in the thread may request backfill. The backfill handler checks the requesting server's identity against the thread's participant list.

**Selective backfill (not full replication):** Unlike Matrix which replicates the entire room DAG, AINM backfill is on-demand. A participant server only fetches history when its local client requests it. This dramatically reduces federation bandwidth and storage requirements.

### 6.7 Offline Queue

```rust
pub struct OfflineQueue {
    // Per-participant queues for messages awaiting delivery
    queues: DashMap<String, VecDeque<QueuedMessage>>,
}

pub struct QueuedMessage {
    pub message: SignedMessage,
    pub queued_at: Instant,
}

impl OfflineQueue {
    pub async fn enqueue(&self, identity: &str, message: &SignedMessage);

    /// Drain all queued messages for a participant (called on reconnect)
    pub async fn drain(&self, identity: &str) -> Vec<SignedMessage>;
}
```

Messages are drained in chronological order when a client reconnects and completes `ainm.initialized`. The spec requires at least 7 days retention (transport.md Section 6.2).

---

## 7. Server Identity and mTLS

### 7.1 Server Identity Endpoint

New Axum route: `GET /.well-known/ainm/server`

```rust
async fn server_identity_handler(State(state): State<ServerState>) -> impl IntoResponse {
    Json(json!({
        "server_id": state.server_identity.domain,
        "public_key": state.server_identity.public_key_b64(),
        "relay_endpoint": format!("https://{}/ainm/v1/relay", state.server_identity.domain),
        "tls_fingerprint": state.server_identity.tls_fingerprint(),
        "supported_versions": ["0.1.0"]
    }))
}
```

### 7.2 Server Ed25519 Keypair

Separate from participant keypairs. The server has its own Ed25519 identity used exclusively for:
1. Signing relay payloads (`server_signature` field)
2. Publishing at `/.well-known/ainm/server`

This key is generated once at server startup and persisted. It is NOT the same as any participant's key.

### 7.3 mTLS for Federation

| Direction | Certificate | Verification |
|-----------|-------------|-------------|
| Outbound relay | This server presents its TLS certificate | Remote server verifies against our `tls_fingerprint` |
| Inbound relay | Remote server presents its TLS certificate | We verify against their published `tls_fingerprint` at `/.well-known/ainm/server` |

**Implementation:** Use `reqwest` with `rustls` for the HTTP client. Configure `ClientConfig` with client certificates for mTLS. Use `axum-server` with `rustls` for the HTTPS listener accepting federation connections.

---

## 8. Modified Components: What Changes

### 8.1 WebSocket Handler Changes

**Before (current):**
```
handle_ws_connection
  -> is_agent? handle_agent_connection : handle_human_connection
  -> Hard-coded broadcast channel routing
```

**After:**
```
handle_ws_connection
  -> authenticate via ainm.initialize (unchanged)
  -> register session in SessionManager (NEW)
  -> drain offline queue for this identity (NEW)
  -> dispatch JSON-RPC methods to handlers
  -> on disconnect: unregister from SessionManager (NEW)
```

The handler no longer distinguishes agent vs human. All participants are handled uniformly. The Message Router handles delivery logic.

### 8.2 Message Send Handler Changes

**Before:** Directly broadcasts to the other channel (`to_agent_tx` or `to_human_tx`).

**After:**
1. Verify signature (unchanged)
2. Check thread membership (unchanged)
3. Assign `server_timestamp` (unchanged -- but only if we are the origin server for this thread)
4. Pass to Message Router (NEW)
5. Message Router handles local/remote/offline delivery

### 8.3 Thread Create Handler Changes

**Before:** Pushes thread_id to `Vec<String>`.

**After:**
1. Validate participants (resolve all identities, including remote) (MODIFIED)
2. Create `FederatedThread` in Thread Registry with `origin_server = self` (NEW)
3. For each remote participant's home server, relay the `thread.create` event via Federation Sender (NEW)

---

## 9. Build Order (Dependency-Aware)

The components have clear dependencies. Build in this order:

```
Phase 1: Foundation (no external dependencies)
  |
  +-- 1a. Server Identity (keypair generation, well-known endpoint)
  +-- 1b. Identity Resolver (extend IdentityStore with remote HTTPS fetch + cache)
  +-- 1c. Session Manager (DashMap-based, replaces broadcast channels)
  |
  v
Phase 2: Local Routing Refactor
  |
  +-- 2a. Thread Registry (replace Vec<String> with FederatedThread)
  +-- 2b. Message Router (local-only first: Session Manager + Offline Queue)
  +-- 2c. Offline Queue (per-participant message storage)
  +-- 2d. Refactor WebSocket handler to use Session Manager + Message Router
  |       (This phase must produce a working single-server system
  |        equivalent to the current demo -- no regression)
  |
  v
Phase 3: Federation Send Path
  |
  +-- 3a. mTLS HTTP client (reqwest + rustls with client certs)
  +-- 3b. Federation Sender (per-destination queues, batching, retry)
  +-- 3c. Extend Message Router to route remote participants to Federation Sender
  |
  v
Phase 4: Federation Receive Path
  |
  +-- 4a. Relay endpoint (POST /ainm/v1/relay with triple-layer validation)
  +-- 4b. Remote server identity verification (fetch + validate .well-known/ainm/server)
  +-- 4c. Inbound message processing (validate -> Message Router -> local delivery)
  |
  v
Phase 5: Thread Federation
  |
  +-- 5a. Thread creation across servers (origin-server creates, relays to participants)
  +-- 5b. Thread join/leave across servers (forward to origin server)
  +-- 5c. Participant management with remote identities
  |
  v
Phase 6: Backfill & History
  |
  +-- 6a. Message persistence (store messages for backfill -- even in-memory for demo)
  +-- 6b. Backfill endpoint (GET /ainm/v1/backfill/{thread_id})
  +-- 6c. Backfill client (request history from origin server)
  |
  v
Phase 7: Demo & Validation
  |
  +-- 7a. docker-compose with 2+ federated servers
  +-- 7b. Cross-server thread creation and messaging demo
  +-- 7c. Offline queue + reconnection demo
  +-- 7d. Backfill demo
  +-- 7e. Updated JSON schemas and AsyncAPI definitions
```

**Why this order:**
- Phase 1 has no dependencies -- pure additive components
- Phase 2 refactors the core routing without adding federation. This is the riskiest phase because it changes existing behavior. Must verify no regression before adding federation complexity.
- Phases 3-4 are the federation send/receive paths. Send before receive because we can test send against a mock/echo server.
- Phase 5 builds on 3+4 for thread lifecycle across servers
- Phase 6 is backfill, which requires message persistence from Phase 2+
- Phase 7 is the integration demo proving everything works together

---

## 10. Scalability Considerations

| Concern | At 2 servers (demo) | At 10 servers | At 100+ servers |
|---------|--------------------|--------------|--------------------|
| **Federation send queues** | 1 destination queue | 9 destination queues | Per-destination queues in DashMap, background task pool |
| **Identity resolution** | 1-2 remote lookups | Cache becomes critical, TTL-based expiry | Consider DNS-style hierarchical caching |
| **Thread participant scatter** | 2-3 participants, 1-2 servers | 10+ participants across multiple servers | Group relay by destination server (already designed) |
| **Backfill volume** | Minimal | Moderate -- cache responses | Rate limit backfill requests per-server |
| **mTLS connections** | 1 peer | 9 peers, consider HTTP/2 connection reuse | Connection pooling via reqwest, idle timeout management |
| **Message storage** | In-memory HashMap | SQLite or PostgreSQL for persistence | PostgreSQL with partitioned message tables |

---

## 11. Patterns to Follow

### Pattern 1: Participant-Addressed Routing (Not Topic-Based)
**What:** Every message routes to individual participants via their home server, not broadcast to a "room" that all servers subscribe to.
**When:** All message delivery.
**Why:** Simpler than Matrix's room-based replication. Only servers with active participants receive messages. A server that had a participant who left stops receiving messages immediately.

### Pattern 2: Origin-Server Timestamp Authority
**What:** The server where a thread was created assigns the authoritative `server_timestamp` to all messages in that thread, whether from local or remote senders.
**When:** Every message in a federated thread.
**Why:** Provides total ordering without a DAG. Remote servers relay messages to the origin for timestamp assignment, then the origin relays to all participants.

**Important nuance for remote messages:** When Server B relays a message from Bob to a thread owned by Server A, Server A assigns the `server_timestamp` and then relays the timestamped message to all participants (including back to Server B for Bob's confirmation). This means remote messages have slightly higher latency (one extra hop through origin) but gain guaranteed ordering.

### Pattern 3: Lazy History (Backfill on Demand)
**What:** Participant servers do NOT proactively replicate thread history. They request it from the origin server only when a local client scrolls up.
**When:** Client requests messages older than what the server has locally.
**Why:** Dramatically reduces federation bandwidth. Most users never scroll past the recent 50 messages.

---

## 12. Anti-Patterns to Avoid

### Anti-Pattern 1: Full State Replication
**What:** Replicating all thread state and history to every participating server.
**Why bad:** Exponential storage growth. Privacy concerns (departed participants' servers still have history). Consistency nightmares.
**Instead:** Origin-server authority with selective backfill.

### Anti-Pattern 2: Synchronous Federation
**What:** Waiting for remote server acknowledgment before confirming message to the local sender.
**Why bad:** A slow/unreachable remote server blocks local users. Federation is inherently asynchronous.
**Instead:** Accept message immediately, queue for async federation delivery. Local sender gets instant `{ accepted: true }`. Remote delivery is eventually-consistent.

### Anti-Pattern 3: Global Ordering Without a Central Authority
**What:** Trying to maintain a total ordering across all servers without an origin-server.
**Why bad:** Impossible without consensus protocols (Raft, Paxos) which add enormous complexity and latency.
**Instead:** Origin-server provides total ordering for each thread. Different threads may have different origin servers, distributing the load.

### Anti-Pattern 4: Persistent Server-to-Server Connections
**What:** Maintaining long-lived WebSocket or TCP connections between federated servers.
**Why bad:** N servers = N*(N-1)/2 connections. Connection management complexity explodes. Servers behind NAT/firewalls have trouble accepting inbound connections.
**Instead:** Stateless HTTP POST for federation relay. Connection per-request with HTTP/2 connection reuse for efficiency.

---

## Sources

- [Matrix Server-Server API Specification v1.9](https://spec.matrix.org/v1.9/server-server-api/)
- [Matrix State Resolution v2](https://matrix.org/docs/older/stateres-v2/)
- [Conduit Architecture Overview (DeepWiki)](https://deepwiki.com/timokoesters/conduit/1-overview)
- [Synapse per_destination_queue.py](https://github.com/matrix-org/synapse/blob/develop/synapse/federation/sender/per_destination_queue.py)
- [XMPP Server Dialback (XEP-0220)](https://xmpp.org/extensions/xep-0220.html)
- [XMPP Inter-Domain Federation Protocol Flows (XEP-0238)](https://xmpp.org/extensions/xep-0238.html)
- [Prosody IM S2S Documentation](https://prosody.im/doc/s2s)
- [Linearized Matrix Proposal (MSC3995)](https://github.com/matrix-org/matrix-spec-proposals/blob/travis/msc/linearized-matrix/proposals/3995-linearized-matrix.md)
- [Conduit/Tuwunel (Successor)](https://github.com/matrix-construct/tuwunel)
- AINM Spec: `transport.md` Section 5 (HTTP Transport Binding / Federation Relay)
- AINM Spec: `identity.md` Section 4 (Identity Resolution)
- AINM Spec: `security.md` (Message Signing / Verification)
- AINM Spec: `threading.md` (Thread Model)
- AINM Demo: `demo/src/server.rs` (Current architecture baseline)
