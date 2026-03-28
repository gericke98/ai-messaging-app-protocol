# Memory Management Specification

**Status:** Draft
**Version:** 0.1.0
**Last Updated:** 2026-03-28

## 1. Overview

Memory management in the AI-Native Messaging Protocol (AINM) enables agents to work with structured context instead of raw message dumps. When an agent joins a thread or is mentioned in an ongoing conversation, the server delivers a structured context payload -- a **context frame** -- that contains exactly what the agent needs to participate effectively: recent messages, a conversation summary, relevant long-term knowledge, and a participant list. This replaces the naive approach of dumping the entire message history into an agent's context window.

AINM defines a three-tier memory model inspired by Letta/MemGPT's hierarchical memory architecture, adapted for a multi-participant protocol where multiple humans and agents share a conversation space:

1. **Active context** -- the most recent messages and thread metadata, always included in every context frame delivered to a participant. This is the "working memory" that keeps agents grounded in the current conversation.

2. **Conversation memory** -- the full message history of the current thread, queryable on demand. Agents do not receive the entire conversation history automatically; they query for specific messages or time ranges when needed.

3. **Archival memory** -- persistent, cross-conversation knowledge stored as structured facts. Each participant has their own private archival store. Agents use archival memory to remember user preferences, learned patterns, and long-term knowledge that spans multiple conversations.

The server is authoritative for all memory management (decision D-13). Participants can request memory operations -- storing facts, querying history, evicting stale knowledge -- but the server enforces storage quotas, retention policies, and access control. This centralized authority prevents memory abuse and ensures consistent behavior across implementations.

Memory operations are private exchanges between a participant and the server. When an agent queries conversation memory for earlier discussion or looks up a user's preferences from archival memory, those queries are NOT visible to other thread participants (decision from RESEARCH.md Open Question 3 and UI-SPEC Contract 13). This privacy is essential: agents should be able to recall context without broadcasting their retrieval strategy to the entire thread.

This specification builds on four prior AINM specifications. Memory operations use JSON-RPC methods transmitted over the WebSocket binding defined in `transport.md`, following the `ainm.{domain}.{action}` naming convention. Context frames reference thread structure from `threading.md` and participant identities from `identity.md`. Memory operations are authenticated by the WebSocket session established during `ainm.initialize` (see `transport.md` Section 4.3) and are therefore covered by the session's cryptographic identity verification defined in `security.md`.

## 2. Terminology

| Term | Definition |
|------|-----------|
| **Memory tier** | One of three layers in the AINM memory model: active context, conversation memory, or archival memory. Each tier has different scope, persistence, and access patterns. |
| **Active context** | The first memory tier. Contains the most recent messages, thread metadata, and participant list. Always included in context frames. Not explicitly queryable -- it IS the current working context. |
| **Conversation memory** | The second memory tier. Contains the full message history of the current thread. Queryable via `ainm.memory.query` with text search, time-range filtering, and sender filtering. Shared across all thread participants. |
| **Archival memory** | The third memory tier. Contains structured facts (key-value pairs) that persist across conversations. Scoped per-participant -- each participant has their own private archival store. Operations: store, query, evict via `ainm.memory.*` methods. |
| **Context frame** | A structured payload delivered by the server to a participant, containing active context, a conversation summary, relevant archival facts, and the participant list. Delivered via `ainm.context.frame` notification. |
| **Fact** | A unit of archival memory. A structured record with `key`, `value`, `tags`, `stored_by`, and `stored_at` fields. Facts are the building blocks of long-term agent knowledge. |
| **Memory operation** | A JSON-RPC request that reads or modifies memory: `ainm.memory.store`, `ainm.memory.query`, or `ainm.memory.evict`. |
| **Context window** | The portion of an agent's processing capacity allocated to context. Context frames are sized to fit within a reasonable context window budget. |
| **Context budget** | The `max_context_frame_size` parameter that limits the size of a context frame in bytes. The server enforces this budget during context frame assembly. |
| **Eviction** | The removal of a fact from archival memory, either by participant request (`ainm.memory.evict`) or by server policy (quota enforcement, LRU eviction). |

## 3. Three-Tier Memory Model

AINM organizes memory into three tiers with increasing persistence and decreasing immediacy. This model prevents context explosion (Pitfall 3 from Phase 2 research) while giving agents access to both recent conversation and long-term knowledge.

```
+----------------------------------------------------------+
|                    ACTIVE CONTEXT                         |
|  Recent messages + thread metadata + participant list     |
|  Scope: shared | Delivery: every context frame            |
|  Size: bounded by max_context_frame_size                  |
+----------------------------------------------------------+
                         |
                         v (query for more)
+----------------------------------------------------------+
|                 CONVERSATION MEMORY                        |
|  Full thread message history                              |
|  Scope: shared | Access: ainm.memory.query                |
|  Retention: thread lifetime                               |
+----------------------------------------------------------+
                         |
                         v (cross-conversation knowledge)
+----------------------------------------------------------+
|                  ARCHIVAL MEMORY                           |
|  Structured facts, preferences, long-term knowledge       |
|  Scope: per-participant | Access: store/query/evict       |
|  Retention: server-configurable                           |
+----------------------------------------------------------+
```

### 3.1 Active Context

Active context is the "working memory" of the conversation -- the most recent state that every participant needs to follow the thread. It is always included in every context frame and is not explicitly queryable because it IS the current context.

**Contents:**

- **Recent messages**: The most recent messages from the current thread, ordered chronologically. Each message is represented as a summary (not the full message envelope) containing `id`, `from`, `timestamp`, and a `summary` field with a text excerpt or server-generated summary.
- **Thread metadata**: The thread's `thread_id`, `title` (if set), `created_at`, `created_by`, and lineage information (`parent_thread_id`, `parent_message_id`) as defined in `threading.md` Section 3.3.
- **Participant list**: Current thread participants with their identity, type (human or agent), and presence status, as defined in `threading.md` Section 3.3.

**Scope:** Shared across all thread participants (decision D-16). Every participant receives the same active context in their context frame.

**Delivery:** Included in every context frame delivered via `ainm.context.frame`. The server assembles active context from the current thread state at the time the context frame is generated.

**Size:** Bounded by the `max_context_frame_size` parameter. Active context receives approximately 50% of the total context frame budget (see Section 4.2 for the full budget allocation). The number of recent messages included is determined by the server to fit within this allocation.

**Not queryable:** Active context is not a separate store that participants query. Agents do not call `ainm.memory.query` with tier `"active"` -- the active context is delivered proactively in every context frame. If an agent needs messages beyond what is in the active context, it queries conversation memory.

### 3.2 Conversation Memory

Conversation memory is the full message history of the current thread, preserved by the server for the thread's lifetime. While active context provides a sliding window of recent messages, conversation memory gives agents access to the entire conversation when they need to look back.

**Contents:**

- The complete history of messages sent in the current thread, including all message types (`message`, `thread.create`, `thread.join`, `thread.leave`, `presence.update`) as defined in `security.md` Section 3.3.
- The server MAY generate and cache conversation summaries for efficient context frame assembly. Summary generation is implementation-defined.

**Scope:** Shared across all thread participants (decision D-16). Any participant in the thread can query the full conversation history. There is no per-participant filtering of conversation memory -- if a message was sent to the thread, all participants can find it via query.

**Access:** Queryable via `ainm.memory.query` with `tier` set to `"conversation"`. Supported query parameters include text search across message content, time-range filtering (`time_range.start`, `time_range.end`), and sender filtering (`from_identity`). See Section 5.2 for the full query method specification.

**Conversation summaries:** The server MAY maintain rolling summaries of the conversation to efficiently populate the `conversation_summary` field in context frames. Summary generation strategy is implementation-defined -- servers MAY use LLM-based summarization, extractive methods, or any other approach. Summaries are cached and refreshed as the conversation evolves.

**Retention:** Conversation memory is retained for the lifetime of the thread. When a thread is archived or deleted, its conversation memory MAY be archived or deleted according to server policy. Servers MAY impose a maximum thread age or message count, beyond which older messages are evicted from conversation memory. Such limits SHOULD be documented in the server's configuration.

### 3.3 Archival Memory

Archival memory is the long-term knowledge store. It persists across conversations, enabling agents to remember user preferences, learned patterns, and accumulated knowledge. Unlike active context and conversation memory, archival memory is scoped per-participant -- each agent or user has their own private store.

**Contents:**

Archival memory stores **facts** -- structured key-value records with metadata. Each fact has:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `fact_id` | string | REQUIRED | Server-assigned unique identifier for this fact (UUIDv7). Returned in the `ainm.memory.store` response. Used for targeted eviction. |
| `key` | string | REQUIRED | A dot-namespaced identifier for the fact. Examples: `"user_preference.timezone"`, `"learned.coding_style"`, `"project.current_deadline"`. Keys MUST be between 1 and 256 characters. Keys MUST contain only ASCII alphanumeric characters, dots (`.`), underscores (`_`), and hyphens (`-`). |
| `value` | string or object | REQUIRED | The fact's content. MAY be a plain string (e.g., `"Europe/Madrid"`) or a JSON object (e.g., `{"language": "TypeScript", "framework": "Next.js"}`). Maximum size: 4KB. |
| `tags` | array of strings | OPTIONAL | Categorization tags for filtering during queries. Each tag MUST be between 1 and 64 characters. Maximum 10 tags per fact. Examples: `["preference", "locale"]`, `["project", "deadline"]`. |
| `stored_by` | string | REQUIRED | The AINM address of the participant who stored this fact. Set by the server based on the authenticated session identity. Clients MUST NOT set this field -- the server overrides any client-provided value. |
| `stored_at` | string | REQUIRED | ISO 8601 timestamp (UTC) of when the fact was stored. Set by the server. |

**Scope:** Per-participant (decision D-16). Each participant has their own archival store. An agent's archival memory is invisible to other participants. A human's archival memory (if the client supports it) is similarly private.

**Why per-participant?** In a multi-participant protocol, shared long-term memory creates ownership confusion (Pitfall 6 from Phase 2 research). If Agent A stores a fact, should Agent B be able to see it? In AINM, the answer is no -- archival memory is private. To share archival knowledge with other participants, an agent posts a message to the thread, making it part of shared conversation memory. This explicit sharing model is predictable and auditable.

**Operations:**

- **Store** (`ainm.memory.store`): Add a new fact to the authenticated participant's archival memory. The server assigns `fact_id`, `stored_by`, and `stored_at`.
- **Query** (`ainm.memory.query` with `tier: "archival"`): Search the authenticated participant's own archival facts by text query, key prefix, or tags.
- **Evict** (`ainm.memory.evict`): Remove a fact by `fact_id` or `key`. Only the participant who stored the fact (or the server via policy enforcement) can evict it.

**Retention:** Server-configurable. See Section 7 for recommended defaults and eviction policies.

## 4. Context Frame

A context frame is a structured payload that the server delivers to a participant to bootstrap or refresh their conversational context. Context frames replace raw message dumps with curated, budget-constrained payloads that fit within a reasonable context window (decision D-14).

### 4.1 Context Frame Schema

Context frames are delivered via `ainm.context.frame` as a JSON-RPC notification from server to client.

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.context.frame",
  "params": {
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "frame": {
      "active_context": {
        "recent_messages": [
          {
            "id": "019580a5-1111-7000-8000-aaaaaaaaaaaa",
            "from": "alice@messaging.example.com",
            "timestamp": "2026-03-28T12:39:00.000Z",
            "summary": "Asked about authentication patterns in the codebase"
          },
          {
            "id": "019580a5-2222-7000-8000-bbbbbbbbbbbb",
            "from": "bob@messaging.example.com",
            "timestamp": "2026-03-28T12:39:30.000Z",
            "summary": "Suggested using JWT refresh tokens with rotation"
          }
        ],
        "window_start": "2026-03-28T12:30:00.000Z",
        "message_count": 15
      },
      "conversation_summary": "Discussion about refactoring the authentication middleware. Alice asked for code review. Bob suggested using JWT refresh tokens. The team is evaluating two approaches: middleware-level validation vs per-route guards.",
      "archival_facts": [
        {
          "fact_id": "019580b0-aaaa-7000-8000-111111111111",
          "key": "user_preference.code_style",
          "value": "Prefer functional patterns over OOP",
          "tags": ["preference", "coding"],
          "stored_by": "code-bot@agents.example.com",
          "stored_at": "2026-03-25T10:00:00.000Z"
        }
      ],
      "participants": [
        {
          "identity": "alice@messaging.example.com",
          "type": "human",
          "presence": "online"
        },
        {
          "identity": "bob@messaging.example.com",
          "type": "human",
          "presence": "away"
        },
        {
          "identity": "code-bot@agents.example.com",
          "type": "agent",
          "presence": "online"
        }
      ]
    }
  }
}
```

**Field definitions:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `thread_id` | string | REQUIRED | The UUIDv7 of the thread this context frame pertains to. |
| `frame` | object | REQUIRED | The context frame payload. |
| `frame.active_context` | object | REQUIRED | The active context tier. |
| `frame.active_context.recent_messages` | array | REQUIRED | Array of recent message summaries, ordered chronologically (oldest first). Each entry contains `id` (UUIDv7), `from` (AINM address), `timestamp` (RFC 3339 UTC), and `summary` (string, server-generated or extracted). MAY be empty for new threads with no messages. |
| `frame.active_context.window_start` | string | REQUIRED | RFC 3339 timestamp (UTC) marking the beginning of the active context window. Messages before this timestamp are not included in `recent_messages` but are available via conversation memory query. |
| `frame.active_context.message_count` | integer | REQUIRED | The total number of messages in the thread at the time of frame assembly. Agents can compare this with the length of `recent_messages` to understand how much history is available beyond the active context. |
| `frame.conversation_summary` | string or null | REQUIRED | A server-generated summary of the conversation history beyond the active context window. `null` if the thread has no history beyond the active context (e.g., a new thread) or if the server does not support summarization. |
| `frame.archival_facts` | array | REQUIRED | Array of archival facts belonging to the receiving participant that the server deems relevant to this thread. The server selects facts based on relevance (implementation-defined). Only the receiving participant's own archival facts are included -- never another participant's archival facts. MAY be empty. |
| `frame.participants` | array | REQUIRED | Array of current thread participants. Each entry contains `identity` (AINM address), `type` (`"human"` or `"agent"` per `identity.md` Section 4.3), and `presence` (current thread-scoped presence status per `threading.md` Section 7). |

### 4.2 Context Frame Budget

Context frames MUST be bounded in size to prevent context explosion (Pitfall 3 from Phase 2 research). The `max_context_frame_size` parameter defines the maximum size of the serialized `frame` object in bytes.

**Recommended default:** 32,768 bytes (32KB).

The server communicates `max_context_frame_size` during capability negotiation (see Section 6). Servers MAY configure a different value. Clients SHOULD be prepared to receive context frames up to the negotiated size.

**Budget allocation:**

The server SHOULD allocate the context frame budget across components using the following priority order:

| Priority | Component | Recommended Allocation | Description |
|----------|-----------|----------------------|-------------|
| 1 | Recent messages | 50% of budget | Most critical for grounding the agent in the current conversation. The server includes as many recent messages as fit within this allocation, starting from the most recent. |
| 2 | Conversation summary | 20% of budget | Provides broader context beyond the active window. The server truncates the summary if it exceeds this allocation. |
| 3 | Archival facts | 20% of budget | The receiving participant's relevant archival facts. The server selects the most relevant facts (relevance scoring is implementation-defined) that fit within this allocation. |
| 4 | Participant list | 10% of budget | Current thread participants with presence. In threads with very many participants, the server MAY truncate the list, prioritizing participants who have been recently active. |

These allocations are recommendations, not normative requirements. Implementations MAY adjust the percentages based on thread characteristics (e.g., a new thread with no history might allocate more budget to archival facts). The server MUST ensure the total serialized `frame` object does not exceed `max_context_frame_size`.

### 4.3 Context Frame Delivery

The server delivers context frames as `ainm.context.frame` notifications (JSON-RPC 2.0 notifications -- no `id` field, no response expected) in the following situations:

1. **Thread join:** When a participant joins a thread via `ainm.thread.join` (as defined in `threading.md` Section 5), the server MUST deliver a context frame to the joining participant. This bootstraps the agent with the current conversation state.

2. **@-mention:** When a participant is @-mentioned in a thread they are already a member of (as defined in `content.md` TextBlock `mentions` array), the server SHOULD deliver an updated context frame to the mentioned participant. This refreshes the agent's context when they are explicitly called upon.

3. **Server-initiated refresh:** The server MAY deliver context frames at other times when it determines a context refresh is beneficial. This is implementation-defined and MAY include scenarios such as: a long period of inactivity followed by new messages, a significant change in thread participants, or a participant reconnecting after a disconnection. Servers SHOULD NOT deliver context frames more frequently than once per minute to the same participant for the same thread, to avoid flooding.

Context frames are NOT thread messages. They are private notifications delivered only to the specific participant over the authenticated WebSocket session. Other participants in the thread do not see the context frame and cannot observe that it was delivered.

## 5. Memory Operations (JSON-RPC Methods)

Memory operations are JSON-RPC 2.0 request/response exchanges between a client and the server. They use the `ainm.{domain}.{action}` naming convention established in `transport.md` Section 3.5. All memory operations require an active, initialized WebSocket session (the `ainm.initialize` / `ainm.initialized` handshake MUST have completed). The server identifies the requesting participant from the authenticated session -- there is no explicit `from` field in memory operation parameters.

Memory operations are private (UI-SPEC Contract 13). They are NOT thread messages and are NOT visible to other thread participants. The server processes the operation and returns a response only to the requesting participant.

### 5.1 ainm.memory.store

Stores a fact in the authenticated participant's archival memory.

**Direction:** Client to server (request/response).

**Request:**

```json
{
  "jsonrpc": "2.0",
  "id": 100,
  "method": "ainm.memory.store",
  "params": {
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "tier": "archival",
    "fact": {
      "key": "user_preference.timezone",
      "value": "Europe/Madrid",
      "tags": ["preference", "locale"]
    }
  }
}
```

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `thread_id` | string | REQUIRED | The thread context for this operation. While archival facts persist across conversations, the `thread_id` provides audit context for when and where the fact was stored. |
| `tier` | string | REQUIRED | MUST be `"archival"`. Active context and conversation memory are managed by the server and cannot be written to by participants. Attempting to store to `"conversation"` or `"active"` returns error `-32051`. |
| `fact` | object | REQUIRED | The fact to store. |
| `fact.key` | string | REQUIRED | Dot-namespaced identifier. MUST be 1-256 characters. MUST match pattern `^[a-zA-Z0-9][a-zA-Z0-9._-]{0,255}$`. |
| `fact.value` | string or object | REQUIRED | The fact content. Maximum 4KB when serialized as JSON. |
| `fact.tags` | array of strings | OPTIONAL | Categorization tags. Maximum 10 tags, each 1-64 characters. |

**Response (success):**

```json
{
  "jsonrpc": "2.0",
  "id": 100,
  "result": {
    "stored": true,
    "fact_id": "019580c0-aaaa-7000-8000-999999999999"
  }
}
```

**Response fields:**

| Field | Type | Description |
|-------|------|-------------|
| `stored` | boolean | Always `true` on success. |
| `fact_id` | string | Server-assigned UUIDv7 identifier for the stored fact. Use this for targeted eviction via `ainm.memory.evict`. |

**Server behavior:**

1. Validate the `tier` is `"archival"`. If not, return error `-32051`.
2. Validate the `fact` object schema (key format, value size, tag constraints). If invalid, return error `-32053`.
3. Check the participant's archival storage quota. If exceeded, return error `-32052`.
4. Set `stored_by` to the authenticated session identity (overriding any client-provided value).
5. Set `stored_at` to the current UTC timestamp.
6. Assign a `fact_id` (UUIDv7).
7. Store the fact and return the `fact_id`.

**Key collision behavior:** If a fact with the same `key` already exists for the same participant, the server MUST replace the existing fact with the new value. The old fact's `fact_id` is retired, and a new `fact_id` is assigned. This allows agents to update preferences without explicitly evicting the old value first.

### 5.2 ainm.memory.query

Queries conversation memory or archival memory.

**Direction:** Client to server (request/response).

**Request (conversation tier):**

```json
{
  "jsonrpc": "2.0",
  "id": 101,
  "method": "ainm.memory.query",
  "params": {
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "tier": "conversation",
    "query": "authentication discussion",
    "limit": 10,
    "offset": 0,
    "time_range": {
      "start": "2026-03-28T10:00:00.000Z",
      "end": "2026-03-28T13:00:00.000Z"
    },
    "from_identity": "alice@messaging.example.com"
  }
}
```

**Request (archival tier):**

```json
{
  "jsonrpc": "2.0",
  "id": 102,
  "method": "ainm.memory.query",
  "params": {
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "tier": "archival",
    "query": "user_preference",
    "limit": 20,
    "offset": 0
  }
}
```

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `thread_id` | string | REQUIRED | For `"conversation"` tier: identifies which thread's history to search. For `"archival"` tier: provides context (archival facts are not thread-scoped, but the thread_id enables future relevance scoring). |
| `tier` | string | REQUIRED | `"conversation"` or `"archival"`. Querying `"active"` tier is not supported and returns error `-32051` -- active context is delivered in context frames, not queried. |
| `query` | string | OPTIONAL | Text search query. For `"conversation"` tier: searches across message content. For `"archival"` tier: searches across fact keys, values, and tags. If omitted, returns results ordered by recency (conversation) or storage time (archival). |
| `limit` | integer | OPTIONAL | Maximum number of results to return. Default: 10. Maximum: 100. Values above 100 are clamped to 100. |
| `offset` | integer | OPTIONAL | Number of results to skip for pagination. Default: 0. |
| `time_range` | object | OPTIONAL | Filter results by time range. Only applicable to `"conversation"` tier. |
| `time_range.start` | string | REQUIRED if `time_range` present | RFC 3339 timestamp (UTC). Results MUST have timestamps >= this value. |
| `time_range.end` | string | REQUIRED if `time_range` present | RFC 3339 timestamp (UTC). Results MUST have timestamps <= this value. |
| `from_identity` | string | OPTIONAL | Filter by sender's AINM address. Only applicable to `"conversation"` tier. When specified, only messages from this sender are returned. |

**Response (conversation tier):**

```json
{
  "jsonrpc": "2.0",
  "id": 101,
  "result": {
    "results": [
      {
        "id": "019580a3-4444-7000-8000-cccccccccccc",
        "from": "alice@messaging.example.com",
        "timestamp": "2026-03-28T11:15:00.000Z",
        "summary": "What authentication pattern should we use for the API middleware?"
      },
      {
        "id": "019580a3-5555-7000-8000-dddddddddddd",
        "from": "bob@messaging.example.com",
        "timestamp": "2026-03-28T11:17:30.000Z",
        "summary": "I recommend JWT with refresh token rotation. Stateless, works well with our microservice setup."
      }
    ],
    "total": 47,
    "has_more": true
  }
}
```

**Response (archival tier):**

```json
{
  "jsonrpc": "2.0",
  "id": 102,
  "result": {
    "results": [
      {
        "fact_id": "019580b0-aaaa-7000-8000-111111111111",
        "key": "user_preference.code_style",
        "value": "Prefer functional patterns over OOP",
        "tags": ["preference", "coding"],
        "stored_by": "code-bot@agents.example.com",
        "stored_at": "2026-03-25T10:00:00.000Z"
      },
      {
        "fact_id": "019580b0-bbbb-7000-8000-222222222222",
        "key": "user_preference.timezone",
        "value": "Europe/Madrid",
        "tags": ["preference", "locale"],
        "stored_by": "code-bot@agents.example.com",
        "stored_at": "2026-03-27T14:00:00.000Z"
      }
    ],
    "total": 2,
    "has_more": false
  }
}
```

**Response fields:**

| Field | Type | Description |
|-------|------|-------------|
| `results` | array | Array of result objects. For `"conversation"` tier: message summaries with `id`, `from`, `timestamp`, `summary`. For `"archival"` tier: fact objects with `fact_id`, `key`, `value`, `tags`, `stored_by`, `stored_at`. |
| `total` | integer | Total number of matching results (not just the current page). |
| `has_more` | boolean | `true` if there are more results beyond the current page (i.e., `offset + limit < total`). |

**Server behavior:**

1. Validate the `tier` is `"conversation"` or `"archival"`. If `"active"`, return error `-32051`.
2. Check that the server supports the requested tier (see Section 6). If not, return error `-32050`.
3. For `"conversation"` tier: search the thread's message history using the provided filters.
4. For `"archival"` tier: search only the authenticated participant's own archival facts. A participant MUST NOT be able to query another participant's archival memory.
5. Return results with pagination metadata.

**Privacy:** Memory queries are private operations. The server MUST NOT generate thread messages, notifications, or side effects visible to other participants when processing a memory query. The query and its results are exchanged solely between the requesting participant and the server.

### 5.3 ainm.memory.evict

Removes a fact from the authenticated participant's archival memory.

**Direction:** Client to server (request/response).

**Request (by fact_id):**

```json
{
  "jsonrpc": "2.0",
  "id": 103,
  "method": "ainm.memory.evict",
  "params": {
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "tier": "archival",
    "fact_id": "019580b0-aaaa-7000-8000-111111111111"
  }
}
```

**Request (by key):**

```json
{
  "jsonrpc": "2.0",
  "id": 104,
  "method": "ainm.memory.evict",
  "params": {
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "tier": "archival",
    "key": "user_preference.code_style"
  }
}
```

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `thread_id` | string | REQUIRED | Provides audit context for the eviction operation. |
| `tier` | string | REQUIRED | MUST be `"archival"`. Active context and conversation memory cannot be evicted by participants. Attempting to evict from `"conversation"` or `"active"` returns error `-32051`. |
| `fact_id` | string | CONDITIONAL | The server-assigned UUIDv7 of the fact to evict. Either `fact_id` or `key` MUST be provided. If both are provided, `fact_id` takes precedence. |
| `key` | string | CONDITIONAL | The key of the fact to evict. If multiple facts share the same key (should not occur due to key-collision replacement, but possible via race conditions), the most recently stored fact with that key is evicted. |

**Response (success):**

```json
{
  "jsonrpc": "2.0",
  "id": 103,
  "result": {
    "evicted": true
  }
}
```

**Response fields:**

| Field | Type | Description |
|-------|------|-------------|
| `evicted` | boolean | Always `true` on success. |

**Server behavior:**

1. Validate the `tier` is `"archival"`. If not, return error `-32051`.
2. Validate that either `fact_id` or `key` is provided. If neither is provided, return error `-32053`.
3. Look up the fact in the authenticated participant's archival memory.
4. If the fact is not found, return error `-32054`.
5. Verify that the fact's `stored_by` matches the authenticated session identity. If not (i.e., the participant is trying to evict another participant's fact -- which should not happen given per-participant scoping, but is a defense-in-depth check), return error `-32055`.
6. Remove the fact and return success.

### 5.4 ainm.context.frame

A server-to-client notification that delivers a context frame to a participant.

**Direction:** Server to client (notification -- no response expected).

This method is documented in Section 4 (Context Frame). It is listed here for completeness in the method catalog.

**Notification format:**

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.context.frame",
  "params": {
    "thread_id": "...",
    "frame": { ... }
  }
}
```

Since `ainm.context.frame` is a JSON-RPC notification (no `id` field), no response is expected from the client. The client SHOULD process the context frame upon receipt and update its internal state.

**Delivery triggers:**

1. Participant joins a thread (`ainm.thread.join` -- see `threading.md` Section 5).
2. Participant is @-mentioned in a thread they are already a member of.
3. Server-initiated refresh (implementation-defined).

See Section 4.3 for complete delivery semantics.

## 6. Capability Negotiation

Memory capabilities are negotiated during the `ainm.initialize` handshake (defined in `transport.md` Section 4.3). The capabilities object is extended with a `memory` field.

### 6.1 Client Capabilities

The client declares memory support in the `ainm.initialize` request:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "ainm.initialize",
  "params": {
    "protocolVersion": "0.1.0",
    "identity": "code-bot@agents.example.com",
    "signature": "...",
    "capabilities": {
      "streaming": true,
      "subThreads": true,
      "contentTypes": ["text", "code", "image", "file"],
      "memory": {
        "conversation": true,
        "archival": true
      }
    }
  }
}
```

**Client memory capability fields:**

| Field | Type | Description |
|-------|------|-------------|
| `memory.conversation` | boolean | Client supports receiving context frames with conversation summaries and can issue `ainm.memory.query` with `tier: "conversation"`. |
| `memory.archival` | boolean | Client supports archival memory operations: `ainm.memory.store`, `ainm.memory.query` with `tier: "archival"`, and `ainm.memory.evict`. |

If the `memory` object is absent from the client's capabilities, the server MUST assume the client does not support any memory features. The server SHOULD still deliver context frames with active context (recent messages and participant list) but MAY omit `conversation_summary` and `archival_facts`.

### 6.2 Server Capabilities

The server declares memory support in the `ainm.initialize` response:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "0.1.0",
    "serverInfo": {
      "name": "messaging.example.com",
      "version": "0.1.0"
    },
    "capabilities": {
      "streaming": true,
      "subThreads": true,
      "storeAndForward": true,
      "maxContentBlocks": 50,
      "maxMessageSize": 1048576,
      "memory": {
        "conversation": true,
        "archival": true,
        "maxContextFrameSize": 32768
      }
    }
  }
}
```

**Server memory capability fields:**

| Field | Type | Description |
|-------|------|-------------|
| `memory.conversation` | boolean | Server supports conversation memory queries (`ainm.memory.query` with `tier: "conversation"`). |
| `memory.archival` | boolean | Server supports archival memory operations (`ainm.memory.store`, `ainm.memory.query` with `tier: "archival"`, `ainm.memory.evict`). |
| `memory.maxContextFrameSize` | integer | Maximum context frame size in bytes. Recommended default: 32768 (32KB). |

### 6.3 Effective Capabilities

A memory feature is available for the session only if BOTH the client and server declare support. This follows the same negotiation model as other AINM capabilities (see `transport.md` Section 4.3).

| Client declares | Server declares | Feature available |
|----------------|----------------|------------------|
| `memory.conversation: true` | `memory.conversation: true` | Yes -- conversation memory queries allowed |
| `memory.conversation: true` | `memory.conversation: false` | No -- `ainm.memory.query` with `tier: "conversation"` returns error `-32050` |
| `memory.archival: true` | `memory.archival: true` | Yes -- archival memory operations allowed |
| `memory.archival: false` | `memory.archival: true` | No -- server will not include archival facts in context frames for this client |

If `memory.archival` is not available (either client or server does not support it), the following methods return error `-32050` (Memory tier unavailable):
- `ainm.memory.store`
- `ainm.memory.evict`
- `ainm.memory.query` with `tier: "archival"`

If `memory.conversation` is not available, `ainm.memory.query` with `tier: "conversation"` returns error `-32050`.

Context frames (`ainm.context.frame`) are always delivered regardless of memory capability negotiation. The `active_context` field (recent messages, participant list) is always populated. The `conversation_summary` and `archival_facts` fields are populated only if the respective capabilities are available.

## 7. Retention and Eviction Policies

Memory retention and eviction are server-managed. The server is authoritative for all policy decisions (decision D-13). This section defines recommended defaults and normative constraints.

### 7.1 Active Context

Active context has no explicit retention policy -- it is always the current thread state. The server assembles active context from the live thread at the time a context frame is generated. There is nothing to retain or evict; active context is ephemeral by nature.

### 7.2 Conversation Memory

Conversation memory is retained for the lifetime of the thread.

- Servers MUST retain conversation memory for at least the duration of an active thread (a thread with at least one connected participant).
- Servers MAY archive old threads and their conversation memory after a period of inactivity. The archival threshold is server-configurable.
- Servers MAY impose a maximum message count per thread, beyond which the oldest messages are evicted. If implemented, the server SHOULD set this threshold to at least 10,000 messages.
- Servers SHOULD NOT delete conversation memory for active threads. If a server must reclaim storage, it SHOULD archive to secondary storage rather than delete.

### 7.3 Archival Memory

Archival memory retention is server-configurable with the following recommended defaults:

| Parameter | Recommended Default | Description |
|-----------|-------------------|-------------|
| Maximum facts per participant | 1,000 | The maximum number of archival facts a single participant can store. |
| Maximum storage per participant | 100KB | The maximum total serialized size of all archival facts for a single participant. |
| Eviction strategy | LRU (Least Recently Used) | When a participant exceeds their quota, the server evicts the least recently accessed facts. "Recently accessed" includes both storage time and last query time. |

**Quota enforcement:**

When a participant's archival memory exceeds the quota (either fact count or storage size), the server MUST take one of the following actions:

1. **Reject the store request** with error `-32052` (Storage quota exceeded). The server SHOULD include the current usage and quota in the error `data` field:

```json
{
  "jsonrpc": "2.0",
  "id": 100,
  "error": {
    "code": -32052,
    "message": "Archival memory quota exceeded",
    "data": {
      "current_facts": 1000,
      "max_facts": 1000,
      "current_bytes": 98304,
      "max_bytes": 102400
    }
  }
}
```

2. **Evict the least recently used facts** to make room, then store the new fact. If the server uses this approach, it SHOULD notify the participant via a system message in the thread where the store was requested:

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.message.deliver",
  "params": {
    "id": "019580d0-aaaa-7000-8000-eeeeeeeeeeee",
    "from": "system@messaging.example.com",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "timestamp": "2026-03-28T14:00:00.000Z",
    "type": "message",
    "content": [
      {
        "type": "text",
        "version": "1.0",
        "source": "system",
        "text": "Archival memory approaching quota (95%). 3 oldest facts were evicted to make room.",
        "fallback": "Archival memory approaching quota."
      }
    ],
    "signature": "..."
  }
}
```

**Quota warning threshold:** Servers SHOULD notify the participant when their archival memory reaches 80% of the quota. This notification is a system message delivered to the thread where the most recent store operation occurred.

### 7.4 Eviction by Server Policy

Beyond participant-initiated eviction (`ainm.memory.evict`), the server MAY evict archival facts in the following circumstances:

- **Quota enforcement** (Section 7.3): LRU eviction when storage limits are exceeded.
- **Account deletion**: When a participant's identity is revoked or deleted, the server SHOULD delete their archival memory.
- **Compliance**: The server MAY delete archival facts to comply with legal requirements (e.g., data retention regulations, right-to-deletion requests).

Server-initiated evictions are not communicated via `ainm.memory.evict` (which is a client-to-server method). Instead, the server logs the eviction internally. Participants discover evicted facts by their absence from query results.

## 8. Error Handling

Memory-related errors use JSON-RPC 2.0 error codes in the range `-32050` to `-32059`. This range is distinct from identity errors (`-32001` to `-32007`), security errors (`-32010` to `-32019`), transport errors (`-32020` to `-32029`), and threading errors (`-32030` to `-32039`).

### 8.1 Error Code Table

| Code | Name | Description | When Returned |
|------|------|-------------|---------------|
| `-32050` | Memory tier unavailable | The server does not support the requested memory tier, or the capability was not negotiated for this session. | `ainm.memory.store` or `ainm.memory.query` or `ainm.memory.evict` when the required `memory.conversation` or `memory.archival` capability is not available. |
| `-32051` | Invalid tier for operation | The requested tier is not valid for this operation. | `ainm.memory.store` with `tier: "conversation"` or `tier: "active"` (only `"archival"` is writable). `ainm.memory.query` with `tier: "active"` (not queryable). `ainm.memory.evict` with `tier: "conversation"` or `tier: "active"` (only `"archival"` is evictable). |
| `-32052` | Storage quota exceeded | The participant's archival memory has reached its storage limit (fact count or byte size). | `ainm.memory.store` when the participant's archival memory is full and the server is configured to reject rather than LRU-evict. |
| `-32053` | Invalid query | The query parameters are malformed or invalid. | `ainm.memory.query` with invalid `time_range`, `limit` < 0, or `offset` < 0. `ainm.memory.store` with invalid fact schema (key too long, value too large, too many tags). `ainm.memory.evict` with neither `fact_id` nor `key` provided. |
| `-32054` | Fact not found | The specified fact does not exist in the participant's archival memory. | `ainm.memory.evict` with a `fact_id` or `key` that does not match any stored fact. |
| `-32055` | Permission denied | The participant does not have permission to perform this operation on the specified fact. | `ainm.memory.evict` when the fact's `stored_by` does not match the authenticated participant. This is a defense-in-depth check -- under normal per-participant scoping, participants should only see their own facts. |

### 8.2 Error Response Format

Memory errors follow the standard JSON-RPC 2.0 error format used throughout AINM:

```json
{
  "jsonrpc": "2.0",
  "id": 100,
  "error": {
    "code": -32051,
    "message": "Invalid tier for operation: 'conversation' tier cannot be written to via ainm.memory.store",
    "data": {
      "tier_requested": "conversation",
      "operation": "store",
      "valid_tiers": ["archival"]
    }
  }
}
```

The `data` field is OPTIONAL and MAY contain additional diagnostic information. Implementations SHOULD include contextual information to help agents understand and recover from errors.

## 9. Security Considerations

### 9.1 Memory Operations Are Private

Memory operations (`ainm.memory.store`, `ainm.memory.query`, `ainm.memory.evict`) are private exchanges between a participant and the server. They are NOT thread messages and are NOT visible to other thread participants. This privacy is essential for several reasons:

- **Agent strategy confidentiality:** An agent querying "what do I know about this user's preferences?" should not broadcast this retrieval to the thread. The query itself reveals the agent's reasoning strategy.
- **User privacy:** Archival facts may contain user preferences or personal information that the user shared with a specific agent. These facts should not be inadvertently exposed to other participants.
- **No side channels:** Memory operations MUST NOT generate thread messages, presence updates, or any other observable side effects. The only observable result is the JSON-RPC response to the requesting participant.

### 9.2 Archival Memory Isolation

Archival memory is strictly per-participant (decision D-16). The server MUST enforce the following isolation guarantees:

- A participant's `ainm.memory.query` with `tier: "archival"` MUST return only facts where `stored_by` matches the authenticated session identity.
- A participant's `ainm.memory.evict` MUST only operate on facts where `stored_by` matches the authenticated session identity.
- Context frames MUST only include archival facts belonging to the receiving participant. The server MUST NOT include facts stored by other participants in a context frame, even if those facts might be "relevant" to the conversation.

**Sharing archival knowledge:** If an agent wants to share long-term knowledge with other participants, it MUST post a message to the thread. That message becomes part of shared conversation memory, accessible to all participants. This explicit sharing model ensures that participants always know when information is being shared and can verify the message signature.

### 9.3 Context Frame Security

Context frames contain potentially sensitive information: recent messages, conversation summaries, user preferences (via archival facts), and participant lists. Servers MUST observe the following security requirements:

- Context frames MUST only be delivered to the authenticated participant over the WebSocket session established during `ainm.initialize`. Servers MUST NOT deliver context frames to uninitialized connections.
- Context frames MUST NOT contain other participants' archival facts.
- Context frames SHOULD NOT include messages from threads the participant is not a member of.
- The `conversation_summary` field MAY contain information derived from messages by all participants. This is acceptable because conversation memory is shared by design (Section 3.2).

### 9.4 Replay and Injection

Memory operations are transmitted over the authenticated WebSocket session and do not carry individual signatures (unlike thread messages, which are individually signed per `security.md`). The security of memory operations relies on:

- **Transport authentication:** The WebSocket session is authenticated during `ainm.initialize` with an Ed25519 signature. All operations on this session are attributable to the authenticated identity.
- **TLS 1.3 encryption:** All WebSocket connections use TLS 1.3, preventing eavesdropping and tampering on memory operations in transit.
- **Server authority:** The server validates all memory operations and enforces quotas and permissions. A compromised client cannot escalate privileges beyond their own archival memory.

An attacker who compromises a participant's WebSocket session could issue memory operations on their behalf (store malicious facts, evict valid facts, query archival memory). Mitigation: the server SHOULD implement rate limiting on memory operations per session, and participants SHOULD rotate keys promptly if session compromise is suspected (see `identity.md` Section 6).

### 9.5 Fact Content Validation

Servers SHOULD validate archival fact content to prevent abuse:

- **Key validation:** Keys MUST conform to the format in Section 3.3 (1-256 characters, dot-namespaced, alphanumeric with dots/underscores/hyphens).
- **Value size limits:** Values MUST NOT exceed 4KB when serialized as JSON. This prevents using archival memory as general-purpose storage.
- **Tag validation:** Each tag MUST be 1-64 characters. Maximum 10 tags per fact.
- **No executable content:** Servers SHOULD reject values that contain executable code patterns (e.g., `<script>` tags, shell commands). This is a defense-in-depth measure; archival facts are private and not rendered as HTML, but preventing stored executable content is good practice.

## 10. Examples

### 10.1 Agent Receives Context Frame on Thread Join

An agent (`code-bot@agents.example.com`) joins a thread where Alice and Bob have been discussing authentication patterns. The server delivers a context frame to bootstrap the agent.

**Step 1: Agent joins the thread**

```json
{
  "jsonrpc": "2.0",
  "id": 50,
  "method": "ainm.thread.join",
  "params": {
    "thread_id": "019580a0-1234-7000-8000-000000000001"
  }
}
```

**Step 2: Server confirms the join**

```json
{
  "jsonrpc": "2.0",
  "id": 50,
  "result": {
    "joined": true,
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "joined_at": "2026-03-28T12:40:00.000Z"
  }
}
```

**Step 3: Server delivers context frame to the agent**

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.context.frame",
  "params": {
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "frame": {
      "active_context": {
        "recent_messages": [
          {
            "id": "019580a3-4444-7000-8000-cccccccccccc",
            "from": "alice@messaging.example.com",
            "timestamp": "2026-03-28T12:35:00.000Z",
            "summary": "What authentication pattern should we use for the API middleware?"
          },
          {
            "id": "019580a3-5555-7000-8000-dddddddddddd",
            "from": "bob@messaging.example.com",
            "timestamp": "2026-03-28T12:37:00.000Z",
            "summary": "JWT with refresh token rotation. Stateless, works with microservices."
          },
          {
            "id": "019580a5-1111-7000-8000-aaaaaaaaaaaa",
            "from": "alice@messaging.example.com",
            "timestamp": "2026-03-28T12:39:00.000Z",
            "summary": "Good idea. @code-bot can you review our current auth middleware?"
          }
        ],
        "window_start": "2026-03-28T12:30:00.000Z",
        "message_count": 8
      },
      "conversation_summary": "Alice and Bob are discussing refactoring the authentication middleware for the API. They are considering JWT with refresh token rotation. Alice has invited code-bot to review the current implementation.",
      "archival_facts": [
        {
          "fact_id": "019580b0-aaaa-7000-8000-111111111111",
          "key": "project.auth_framework",
          "value": "Currently using express-jwt v8 with RS256 signing",
          "tags": ["project", "auth", "dependencies"],
          "stored_by": "code-bot@agents.example.com",
          "stored_at": "2026-03-20T09:00:00.000Z"
        },
        {
          "fact_id": "019580b0-bbbb-7000-8000-222222222222",
          "key": "user_preference.code_style",
          "value": "Prefer functional patterns over OOP",
          "tags": ["preference", "coding"],
          "stored_by": "code-bot@agents.example.com",
          "stored_at": "2026-03-25T10:00:00.000Z"
        }
      ],
      "participants": [
        {
          "identity": "alice@messaging.example.com",
          "type": "human",
          "presence": "online"
        },
        {
          "identity": "bob@messaging.example.com",
          "type": "human",
          "presence": "online"
        },
        {
          "identity": "code-bot@agents.example.com",
          "type": "agent",
          "presence": "online"
        }
      ]
    }
  }
}
```

The agent now has structured context: it knows the recent conversation (active context), the broader discussion topic (conversation summary), its own relevant knowledge (archival facts), and who is in the thread (participants). It can begin participating immediately.

### 10.2 Agent Stores a User Preference as Archival Fact

During a conversation, code-bot learns that Alice prefers TypeScript over JavaScript. The agent stores this as an archival fact for future reference.

**Request:**

```json
{
  "jsonrpc": "2.0",
  "id": 200,
  "method": "ainm.memory.store",
  "params": {
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "tier": "archival",
    "fact": {
      "key": "user_preference.language",
      "value": {
        "preferred": "TypeScript",
        "reason": "Alice mentioned preferring TypeScript for type safety in API development"
      },
      "tags": ["preference", "language", "alice"]
    }
  }
}
```

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 200,
  "result": {
    "stored": true,
    "fact_id": "019580c0-cccc-7000-8000-333333333333"
  }
}
```

This fact is now stored in code-bot's archival memory. It will be available in future context frames when code-bot joins threads where language preferences are relevant. Other participants (Alice, Bob) cannot see this fact unless code-bot explicitly shares it by posting to the thread.

### 10.3 Agent Queries Conversation Memory for Earlier Discussion

Later in the conversation, code-bot needs to recall what Bob said about token rotation. Instead of asking Bob to repeat himself, the agent queries conversation memory.

**Request:**

```json
{
  "jsonrpc": "2.0",
  "id": 201,
  "method": "ainm.memory.query",
  "params": {
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "tier": "conversation",
    "query": "refresh token rotation",
    "from_identity": "bob@messaging.example.com",
    "limit": 5
  }
}
```

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 201,
  "result": {
    "results": [
      {
        "id": "019580a3-5555-7000-8000-dddddddddddd",
        "from": "bob@messaging.example.com",
        "timestamp": "2026-03-28T12:37:00.000Z",
        "summary": "JWT with refresh token rotation. Stateless, works with microservices."
      },
      {
        "id": "019580a4-6666-7000-8000-eeeeeeeeeeee",
        "from": "bob@messaging.example.com",
        "timestamp": "2026-03-28T12:38:00.000Z",
        "summary": "Refresh tokens should have a 7-day TTL with rotation on each use. Revocation via token family tracking."
      }
    ],
    "total": 2,
    "has_more": false
  }
}
```

This query is private. Alice and Bob do not see that code-bot queried the conversation history. The agent can now reference Bob's recommendation in its response.

### 10.4 Agent Queries Archival Memory for User Preferences

Before generating a code review, code-bot queries its own archival memory for relevant user preferences.

**Request:**

```json
{
  "jsonrpc": "2.0",
  "id": 202,
  "method": "ainm.memory.query",
  "params": {
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "tier": "archival",
    "query": "user_preference",
    "limit": 10
  }
}
```

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 202,
  "result": {
    "results": [
      {
        "fact_id": "019580b0-aaaa-7000-8000-111111111111",
        "key": "user_preference.code_style",
        "value": "Prefer functional patterns over OOP",
        "tags": ["preference", "coding"],
        "stored_by": "code-bot@agents.example.com",
        "stored_at": "2026-03-25T10:00:00.000Z"
      },
      {
        "fact_id": "019580c0-cccc-7000-8000-333333333333",
        "key": "user_preference.language",
        "value": {
          "preferred": "TypeScript",
          "reason": "Alice mentioned preferring TypeScript for type safety in API development"
        },
        "tags": ["preference", "language", "alice"],
        "stored_by": "code-bot@agents.example.com",
        "stored_at": "2026-03-28T12:42:00.000Z"
      },
      {
        "fact_id": "019580b0-bbbb-7000-8000-222222222222",
        "key": "user_preference.timezone",
        "value": "Europe/Madrid",
        "tags": ["preference", "locale"],
        "stored_by": "code-bot@agents.example.com",
        "stored_at": "2026-03-27T14:00:00.000Z"
      }
    ],
    "total": 3,
    "has_more": false
  }
}
```

The agent now knows: the team prefers functional patterns, Alice likes TypeScript, and their timezone is Europe/Madrid. This context informs the code review without the agent needing to ask these questions in the thread.

### 10.5 Complete Flow: Thread Join with Memory-Driven Response

This example shows the complete flow from thread join through context-driven response, combining all memory tiers.

```
Agent                                Server
  |                                    |
  |--- ainm.thread.join ------------->|  (1) Agent joins thread
  |<-- result: { joined: true } ------|  (2) Server confirms
  |<-- ainm.context.frame ------------|  (3) Server delivers context
  |                                    |
  |  [Agent processes context frame]   |
  |  [Active context: recent msgs]     |
  |  [Summary: conversation topic]     |
  |  [Archival: known preferences]     |
  |                                    |
  |--- ainm.memory.query ------------>|  (4) Agent queries for details
  |    tier: "conversation"            |      (private operation)
  |    query: "token rotation"         |
  |<-- result: { results: [...] } ----|  (5) Server returns matches
  |                                    |
  |--- ainm.message.send ------------>|  (6) Agent posts informed response
  |    (uses context + query results)  |      to the thread
  |                                    |
  |--- ainm.memory.store ------------>|  (7) Agent stores new fact
  |    tier: "archival"                |      for future reference
  |    key: "project.auth_decision"    |
  |<-- result: { stored: true } ------|  (8) Server confirms storage
  |                                    |
```

## References

- **AINM Threading Specification** (`threading.md`) -- Thread model, thread.join event, participant lists, thread-scoped presence. Memory is conversation-scoped; context frames are delivered on `ainm.thread.join`.
- **AINM Transport Specification** (`transport.md`) -- JSON-RPC framing, `ainm.{domain}.{action}` method naming convention, capability negotiation during `ainm.initialize`, WebSocket session lifecycle. Memory operations use JSON-RPC methods over the WebSocket connection.
- **AINM Identity Specification** (`identity.md`) -- Participant identity (`name@provider`), identity resolution, participant type (`human` or `agent`). Archival memory is scoped per-participant identity.
- **AINM Security Specification** (`security.md`) -- Message signing, trust annotations, session authentication. Memory operations are authenticated by the WebSocket session, not individually signed.
- **RFC 8785** -- JSON Canonicalization Scheme (JCS). Referenced in context of message signing; memory operations are not individually signed.
- **RFC 9562** -- Universally Unique Identifiers (UUIDs). UUIDv7 is used for `fact_id` and `thread_id`.
- **RFC 3339** -- Date and Time on the Internet: Timestamps. Used for `stored_at`, `window_start`, and all temporal fields.
- **RFC 2119 + RFC 8174** -- Key words for use in RFCs to Indicate Requirement Levels (MUST, SHOULD, MAY, etc.).
- **Letta/MemGPT** -- Hierarchical memory architecture (core memory, recall memory, archival memory) that inspired the three-tier model. Adapted for multi-participant protocol context.
