# Compound Memory Specification

**Status:** Draft
**Version:** 0.1.0
**Last Updated:** 2026-04-09

## 1. Overview

The base memory specification (`memory.md`) defines a three-tier model where agents maintain private archival facts, query shared conversation history, and receive curated context frames. This model is sound for isolated agent interactions -- but it blocks a more ambitious capability: **compound memory**, where the messaging system itself accumulates structured knowledge about a user across all conversations, and any agent can be instantly briefed on who that user is from day one.

Today, when a user adds a new agent to a thread, that agent starts with zero context about the user. It does not know the user's expertise, preferences, communication patterns, goals, or prior decisions. The user must re-explain themselves to every new agent. This is the "Memento problem" -- agents have anterograde amnesia, unable to build persistent understanding across interactions.

Compound memory solves this by introducing a **user-owned knowledge graph** that accumulates structured, temporal, interconnected knowledge from every conversation. The server compiles raw interactions into this graph -- extracting entities, relationships, and patterns -- much like a compiler transforms source code into executable knowledge. When a new agent joins, the server generates a **briefing** scoped to that agent's capabilities, instantly onboarding it with relevant user context.

This specification extends the base memory model with four new capabilities:

1. **User Memory Graph** -- A knowledge graph of entities (nodes) and relationships (edges) organized into three cognitive types: semantic (facts and preferences), episodic (events and decisions), and procedural (patterns and strategies). The graph belongs to the user, not any individual agent.

2. **Memory Compilation** -- An LLM-powered process that extracts structured knowledge from conversations and archival facts, consolidates it into the user memory graph, and maintains graph health through contradiction detection and pruning.

3. **Agent Briefing** -- A protocol for onboarding new agents with a scoped view of the user's compound memory, filtered by the agent's declared capabilities and the user's consent level.

4. **Memory Portability** -- An open export/import format (`ainm-memory-v1`) that enables users to carry their compound memory between providers, ensuring no vendor lock-in.

All four capabilities are OPTIONAL extensions to the base memory specification. Servers that do not support compound memory continue to operate with the three-tier model unchanged. Servers that support compound memory negotiate this capability during `ainm.initialize` (see Section 10).

### Design Principles

**User-owned, not agent-owned.** The fundamental shift from base memory: archival facts are per-agent, but compound memory is per-user. The user's knowledge graph is a single entity that agents contribute to but do not own. This enables knowledge to persist when agents are replaced.

**Compiled, not retrieved.** Inspired by Karpathy's LLM knowledge base architecture: raw conversations are "source code," the compilation engine is the "compiler," and the knowledge graph is the "executable." Knowledge is structured once and kept current -- not re-derived from raw history on every query. This creates compounding value: every conversation enriches the graph, and the graph becomes more valuable with each interaction.

**Open and portable.** The compound memory format is an open standard. Users can export their knowledge graph, import it on a different provider, or share it with third-party applications. The network effect depends on adoption, and adoption depends on openness.

**Capability-scoped access.** Agents do not receive the full knowledge graph. They receive a scoped view filtered by their declared capabilities (from their Agent Card) and the user's consent level. A coding agent sees technical preferences and expertise; a scheduling agent sees calendar patterns and availability. The server performs this scoping automatically.

**Temporal by default.** Every node and edge carries temporal validity metadata. Facts have a `valid_from` timestamp and an optional `valid_until`. When a fact is superseded (e.g., the user changes their preferred framework), the old node is marked with `valid_until` and a `superseded_by` pointer, preserving historical knowledge while surfacing current truth.

This specification builds on the base `memory.md` specification, the capability negotiation defined in `transport.md`, the Agent Card capability declarations in `agent-card.md`, and the identity model in `identity.md`.

## 2. Terminology

| Term | Definition |
|------|-----------|
| **Compound memory** | The user-owned knowledge graph plus the compilation and briefing infrastructure that maintains it. Compound memory is the fourth tier of the AINM memory model, extending the three tiers defined in `memory.md`. |
| **User memory graph** | A directed graph of nodes (entities) and edges (relationships) that represents accumulated knowledge about a user. Owned by the user, contributed to by agents, managed by the server. |
| **Memory node** | An entity in the user memory graph. Represents a discrete piece of knowledge: a fact, a preference, a skill, an event, a decision, or a behavioral pattern. Each node has a type (semantic, episodic, or procedural), metadata, temporal validity, and provenance. |
| **Memory edge** | A directed relationship between two memory nodes. Represents how entities relate: `uses`, `prefers_over`, `works_at`, `decided_on`, `led_to`, `requires`. Each edge has temporal validity and provenance. |
| **Semantic node** | A memory node of type `semantic`. Represents stable, factual knowledge: user preferences, expertise areas, biographical facts, technical stack choices. Semantic nodes have high stability and change infrequently. |
| **Episodic node** | A memory node of type `episodic`. Represents time-bound events: decisions made, problems solved, conversations had, outcomes observed. Episodic nodes are always anchored to a specific time and often to a specific thread. |
| **Procedural node** | A memory node of type `procedural`. Represents learned patterns and strategies: preferred workflows, problem-solving approaches, communication styles, recurring behaviors. Procedural nodes are inferred from repeated observations across multiple conversations. |
| **Memory compilation** | The LLM-powered process of extracting structured knowledge from conversations and consolidating it into the user memory graph. Analogous to compiling source code into an executable. |
| **Compilation pass** | A single execution of the memory compilation engine over a set of source material (conversation messages, archival facts). Produces new or updated nodes and edges. |
| **Agent briefing** | A structured payload delivered to an agent containing a scoped view of the user's compound memory, tailored to the agent's capabilities and the conversation context. |
| **Memory scope** | A named subset of the user memory graph that can be independently granted to or revoked from an agent. Scopes map to node categories: `identity`, `preferences`, `expertise`, `goals`, `history`, `patterns`. |
| **Consent level** | The user's global preference for how much compound memory is shared with agents. Three levels: `minimal`, `standard`, `full`. See Section 7.2. |
| **Temporal validity** | The time window during which a node or edge is considered current. Defined by `valid_from` and `valid_until` fields. Nodes without `valid_until` are considered currently valid. |
| **Provenance** | Metadata tracking where a node or edge came from: which conversation, which message, which agent contributed it, and when it was extracted. Essential for auditability and trust. |
| **Memory export** | A self-contained JSON document in the `ainm-memory-v1` format containing a user's complete knowledge graph with all nodes, edges, and metadata. Designed for portability between providers. |

## 3. Architecture

Compound memory extends the three-tier model from `memory.md` with a fourth tier. The existing tiers are unchanged -- compound memory is additive.

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
                         |
                         v (compilation extracts and structures)
+----------------------------------------------------------+
|                  COMPOUND MEMORY (new)                     |
|  User-owned knowledge graph                               |
|  Nodes: semantic + episodic + procedural                  |
|  Edges: typed relationships with temporal validity        |
|  Scope: per-user | Access: scoped views via briefing      |
|  Retention: user-controlled, server-managed               |
+----------------------------------------------------------+
```

### 3.1 Relationship to Base Memory

Compound memory is built FROM the lower three tiers, not as a replacement:

- **Conversation memory** is the primary source material. The compilation engine processes conversation messages to extract entities and relationships.
- **Archival memory** provides supplementary input. Agent-stored facts feed into compilation as high-confidence signals (they were explicitly stored, not inferred).
- **Active context** is not directly used by compilation -- it is ephemeral by nature. However, the context frame delivery mechanism is extended to optionally include briefing data (see Section 6.3).

Agents continue to use archival memory (`ainm.memory.store`, `.query`, `.evict`) for their own private state management. Compound memory does not replace archival memory -- it aggregates knowledge from across all agents' contributions into a unified, user-owned graph.

### 3.2 Data Flow

```
Conversations                    Archival Facts
(all threads)                    (all participants)
     |                                 |
     v                                 v
+----------------------------------------------------------+
|              MEMORY COMPILATION ENGINE                     |
|                                                            |
|  1. Entity extraction (LLM-powered)                       |
|  2. Relationship mapping                                  |
|  3. Temporal validity assignment                          |
|  4. Deduplication and conflict resolution                 |
|  5. Confidence scoring                                    |
|  6. Graph integration (merge into existing graph)         |
+----------------------------------------------------------+
                         |
                         v
+----------------------------------------------------------+
|               USER MEMORY GRAPH                            |
|                                                            |
|  [Semantic]     [Episodic]      [Procedural]              |
|  - facts        - events        - patterns                |
|  - preferences  - decisions     - workflows               |
|  - expertise    - outcomes      - strategies              |
|  - relationships- interactions  - habits                  |
|                                                            |
|  Connected by typed, temporal edges                       |
+----------------------------------------------------------+
                         |
              +----------+----------+
              |                     |
              v                     v
      +---------------+    +----------------+
      | Agent Briefing |    | Memory Export   |
      | (scoped view   |    | (ainm-memory-v1 |
      |  per agent)    |    |  portable JSON) |
      +---------------+    +----------------+
```

### 3.3 Ownership Model

Compound memory introduces a new ownership concept not present in base memory:

- **Archival memory** (base spec): owned by the storing participant. Agent A's facts are invisible to Agent B. The participant controls their own store.
- **Compound memory** (this spec): owned by the **user**. All agents contribute to a single graph, but the user is the owner. The user controls consent levels, can view all nodes, can delete nodes, and can export the entire graph.

The server acts as the custodian of compound memory, managing storage, compilation, access control, and export. The server MUST NOT use compound memory data for purposes other than serving the user's agents, unless the user has explicitly consented to additional uses.

## 4. Memory Graph Model

The user memory graph is a directed graph `G = (N, E)` where `N` is the set of memory nodes and `E` is the set of memory edges.

### 4.1 Memory Nodes

A memory node represents a discrete unit of knowledge about the user. Every node has a type, a category, properties, temporal validity, confidence, and provenance.

**Node schema:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `node_id` | string | REQUIRED | Server-assigned unique identifier (UUIDv7). |
| `type` | string | REQUIRED | One of `"semantic"`, `"episodic"`, or `"procedural"`. Determines the cognitive category of the knowledge. |
| `category` | string | REQUIRED | Finer classification within the type. See Section 4.1.1 for the category taxonomy. |
| `label` | string | REQUIRED | Human-readable label for the node. MUST be between 1 and 512 characters. Examples: `"Prefers TypeScript over JavaScript"`, `"Decided to use PostgreSQL for the auth service"`, `"Tends to refactor before adding features"`. |
| `properties` | object | OPTIONAL | Arbitrary key-value properties providing structured detail. Keys MUST be strings. Values MUST be strings, numbers, booleans, or arrays of strings. Maximum 4KB when serialized. |
| `confidence` | number | REQUIRED | Confidence score between 0.0 and 1.0. Explicitly stated facts receive 1.0. Inferred patterns start lower and increase with repeated observation. See Section 4.1.2. |
| `source` | object | REQUIRED | Provenance metadata. See Section 4.1.3. |
| `temporal` | object | REQUIRED | Temporal validity. See Section 4.1.4. |
| `tags` | array of strings | OPTIONAL | Categorization tags for filtering. Maximum 20 tags, each 1-64 characters. |

#### 4.1.1 Node Categories

Each node type has a defined set of categories:

**Semantic categories** (stable, factual knowledge):

| Category | Description | Example |
|----------|-------------|---------|
| `preference` | User preferences and choices | "Prefers dark mode", "Uses 2-space indentation" |
| `fact` | Biographical or contextual facts | "Lives in Madrid", "Works at Acme Corp" |
| `skill` | Areas of expertise or competence | "Expert in Rust", "Intermediate in SwiftUI" |
| `relationship` | Connections to other people or entities | "Works with Alice on the auth project" |
| `tool` | Tools, technologies, and services used | "Uses VS Code", "Deploys on Vercel" |

**Episodic categories** (time-bound events):

| Category | Description | Example |
|----------|-------------|---------|
| `event` | Notable occurrences | "Migrated database to PostgreSQL on 2026-03-15" |
| `decision` | Choices made with rationale | "Chose JWT over session tokens for stateless auth" |
| `outcome` | Results of actions or decisions | "PostgreSQL migration reduced query latency by 40%" |
| `interaction` | Significant conversations or exchanges | "Discussed API design with the team" |

**Procedural categories** (learned patterns and strategies):

| Category | Description | Example |
|----------|-------------|---------|
| `pattern` | Recurring behavioral patterns | "Always writes tests before implementation" |
| `workflow` | Preferred sequences of actions | "Reviews PR → runs tests locally → merges" |
| `strategy` | Problem-solving approaches | "When debugging, starts with log analysis before code inspection" |
| `communication` | Communication style preferences | "Prefers concise responses without summaries" |

Servers MAY define additional categories beyond this taxonomy. Clients MUST treat unrecognized categories as valid and display them using their `label` and `type`.

#### 4.1.2 Confidence Scoring

Confidence scores reflect how certain the system is about a piece of knowledge:

| Score Range | Interpretation | Source |
|-------------|---------------|--------|
| 0.9 - 1.0 | Explicitly stated by the user | User said "I prefer TypeScript" or agent stored with explicit confirmation |
| 0.7 - 0.89 | Strongly inferred from multiple observations | Observed across 3+ conversations |
| 0.5 - 0.69 | Moderately inferred | Observed in 1-2 conversations |
| 0.3 - 0.49 | Weakly inferred | Single observation, indirect signal |
| 0.0 - 0.29 | Speculative | Inferred from minimal evidence, may be wrong |

Confidence scores are set by the compilation engine during extraction. Scores MAY increase when the same knowledge is confirmed in subsequent conversations. Scores SHOULD decrease over time for episodic and procedural nodes that have not been recently reinforced (decay rate is implementation-defined).

Agents receiving a briefing MAY use confidence scores to weight their decisions -- high-confidence facts can be treated as reliable; low-confidence observations should be treated as hypotheses.

#### 4.1.3 Provenance

Every node carries provenance metadata tracking its origin:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `source.thread_id` | string | OPTIONAL | UUIDv7 of the thread where this knowledge was extracted. OPTIONAL because some nodes may be created from cross-thread synthesis or user import. |
| `source.message_id` | string | OPTIONAL | UUIDv7 of the specific message that triggered extraction. OPTIONAL for synthesized or imported nodes. |
| `source.contributed_by` | string | REQUIRED | AINM address of the agent that contributed or extracted this knowledge. For compilation-extracted nodes, this is the server's compilation identity. For agent-contributed nodes, this is the contributing agent's address. |
| `source.extracted_at` | string | REQUIRED | RFC 3339 timestamp (UTC) of when the node was created or last updated. |
| `source.compilation_pass` | string | OPTIONAL | Identifier of the compilation pass that produced this node. Enables auditing of which compilation run generated which knowledge. |

#### 4.1.4 Temporal Validity

Every node has a temporal validity window:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `temporal.valid_from` | string | REQUIRED | RFC 3339 timestamp (UTC) marking when this knowledge became valid. For explicitly stated facts, this is the timestamp of the statement. For inferred patterns, this is the timestamp of the earliest supporting observation. |
| `temporal.valid_until` | string or null | REQUIRED | RFC 3339 timestamp (UTC) marking when this knowledge ceased to be valid. `null` if the knowledge is currently valid. Set when a fact is superseded or explicitly invalidated. |
| `temporal.superseded_by` | string or null | OPTIONAL | `node_id` of the node that superseded this one. `null` if not superseded. Enables graph traversal from outdated knowledge to current knowledge. |

**Example -- temporal supersession:**

The user preferred React in 2024, then switched to Svelte in 2026:

```json
[
  {
    "node_id": "019580a0-aaaa-7000-8000-000000000001",
    "type": "semantic",
    "category": "preference",
    "label": "Prefers React for frontend development",
    "confidence": 1.0,
    "temporal": {
      "valid_from": "2024-01-15T00:00:00.000Z",
      "valid_until": "2026-02-01T00:00:00.000Z",
      "superseded_by": "019580a0-aaaa-7000-8000-000000000002"
    }
  },
  {
    "node_id": "019580a0-aaaa-7000-8000-000000000002",
    "type": "semantic",
    "category": "preference",
    "label": "Prefers Svelte for frontend development",
    "confidence": 1.0,
    "temporal": {
      "valid_from": "2026-02-01T00:00:00.000Z",
      "valid_until": null,
      "superseded_by": null
    }
  }
]
```

### 4.2 Memory Edges

A memory edge represents a directed relationship between two nodes. Edges carry the same temporal validity and provenance as nodes.

**Edge schema:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `edge_id` | string | REQUIRED | Server-assigned unique identifier (UUIDv7). |
| `source_node` | string | REQUIRED | `node_id` of the source node. |
| `target_node` | string | REQUIRED | `node_id` of the target node. |
| `relationship` | string | REQUIRED | The type of relationship. See Section 4.2.1 for the relationship taxonomy. |
| `properties` | object | OPTIONAL | Additional relationship properties. Same constraints as node properties. |
| `weight` | number | OPTIONAL | Relationship strength between 0.0 and 1.0. Higher values indicate stronger or more frequently observed relationships. Default: 1.0. |
| `confidence` | number | REQUIRED | Same semantics as node confidence (Section 4.1.2). |
| `source` | object | REQUIRED | Same schema as node provenance (Section 4.1.3). |
| `temporal` | object | REQUIRED | Same schema as node temporal validity (Section 4.1.4). |

#### 4.2.1 Relationship Types

The protocol defines a core set of relationship types. Servers MAY define additional types.

| Relationship | Direction | Example |
|-------------|-----------|---------|
| `uses` | subject → tool/technology | "User" → "TypeScript" |
| `prefers_over` | preferred → alternative | "PostgreSQL" → "MySQL" |
| `works_at` | person → organization | "User" → "Acme Corp" |
| `works_with` | person → person | "User" → "Alice" |
| `decided_on` | decision → subject | "Chose JWT" → "Auth System" |
| `led_to` | cause → effect | "DB Migration" → "40% Latency Reduction" |
| `requires` | dependency → dependant | "Deployment" → "CI/CD Pipeline" |
| `part_of` | component → whole | "Auth Module" → "Backend Service" |
| `learned_from` | knowledge → source event | "Prefers TDD" → "Bug in Production" |
| `contradicts` | fact → contradicting fact | Only for flagging unresolved contradictions |
| `similar_to` | entity → related entity | Semantic similarity, inferred |

### 4.3 Graph Constraints

Servers MUST enforce the following constraints on the user memory graph:

| Constraint | Recommended Limit | Description |
|-----------|-------------------|-------------|
| Maximum nodes per user | 10,000 | Prevents unbounded graph growth. Servers SHOULD evict low-confidence, temporally expired nodes when approaching the limit. |
| Maximum edges per user | 50,000 | Edge count typically scales with node count. 5:1 ratio is a reasonable default. |
| Maximum node label length | 512 characters | Human-readable labels should be concise. |
| Maximum node properties size | 4KB serialized | Same as archival fact value limit. |
| Maximum edges per node | 100 | Prevents "god nodes" that connect to everything. |
| Minimum confidence for persistence | 0.1 | Nodes below this threshold MAY be automatically pruned. |

These are recommended limits. Servers MUST document their actual limits in the `ainm.initialize` capability response (Section 10). Servers MUST return error code `-32060` (graph quota exceeded) when limits are reached.

## 5. Memory Compilation

Memory compilation is the process of transforming raw conversational data into structured knowledge in the user memory graph. The compilation engine is LLM-powered -- it uses a language model to extract entities, relationships, and patterns that a rule-based system would miss.

### 5.1 Compilation Lifecycle

Compilation follows a three-operation lifecycle inspired by Karpathy's LLM knowledge base architecture:

**Ingest** -- Process new conversational data, extract entities and relationships, and integrate them into the existing graph. This is the primary compilation operation, triggered after conversations conclude or at server-defined intervals.

**Query** -- The compilation engine does not query; agents and users do. However, the compilation engine MAY perform self-directed queries during ingestion to check for duplicate entities or contradicting facts before inserting new nodes.

**Lint** -- A health-check pass that scans the graph for contradictions, stale nodes, orphan nodes (no edges), and knowledge gaps. Lint results are reported to the user and MAY be surfaced to agents.

### 5.2 Compilation Triggers

Servers MUST support at least one compilation trigger. Servers MAY support multiple triggers.

| Trigger | Requirement | Description |
|---------|------------|-------------|
| End of conversation | MUST | The server MUST trigger compilation when a thread becomes inactive (no messages for a server-defined period, recommended: 30 minutes) or when the last participant leaves. |
| Real-time incremental | MAY | The server MAY trigger compilation after each message or batch of messages, updating the graph incrementally. This provides faster knowledge accumulation but requires more LLM invocations. |
| Periodic batch | MAY | The server MAY run compilation on a scheduled basis (e.g., daily), processing all conversations since the last compilation pass. Efficient for high-volume servers. |
| Manual trigger | MAY | The server MAY allow users to explicitly request compilation via `ainm.memory.compile`. |

### 5.3 Compilation Process

The compilation engine MUST perform the following steps for each compilation pass:

1. **Source assembly** -- Gather the source material: conversation messages from the target thread(s), plus the user's existing archival facts from all participating agents. The compilation engine accesses conversation memory as a privileged system component -- it is not subject to the per-participant archival scoping restrictions of base memory.

2. **Entity extraction** -- Use an LLM to identify entities (people, technologies, preferences, events, decisions, patterns) from the source material. Each extracted entity becomes a candidate memory node with a type, category, label, and confidence score.

3. **Relationship extraction** -- Use an LLM to identify relationships between extracted entities and between extracted entities and existing graph nodes. Each relationship becomes a candidate memory edge.

4. **Temporal assignment** -- Assign temporal validity to each candidate node and edge. For explicitly stated facts, `valid_from` is the timestamp of the statement. For inferred patterns, `valid_from` is the timestamp of the earliest supporting message.

5. **Deduplication** -- Check candidate nodes against existing graph nodes. If a candidate matches an existing node (same label, type, and overlapping temporal validity), the compilation engine MUST merge rather than duplicate. Merging updates the confidence score (higher of the two values) and refreshes the `source.extracted_at` timestamp.

6. **Contradiction resolution** -- Check candidate nodes against existing nodes for contradictions. If a new fact contradicts an existing fact (e.g., "prefers React" vs. "prefers Svelte"), the compilation engine MUST:
   - Set `valid_until` on the older node to the `valid_from` of the newer node.
   - Set `superseded_by` on the older node to the `node_id` of the newer node.
   - Create a new edge of type `contradicts` between the two nodes (for auditability).
   - Log the contradiction in the compilation report.

7. **Confidence scoring** -- Assign confidence scores to new nodes based on the extraction context:
   - Explicit user statements: 0.95 - 1.0
   - Agent-stored archival facts used as source: 0.85 - 0.94
   - Strongly implied by conversation context: 0.7 - 0.84
   - Weakly implied or inferred from single observation: 0.5 - 0.69
   - Cross-thread pattern (inferred from multiple conversations): 0.6 - 0.84 (boosted for repetition)

8. **Graph integration** -- Insert the deduplicated, validated candidate nodes and edges into the user memory graph. Update existing nodes with merged data where applicable.

### 5.4 Compilation Notification

After a compilation pass completes, the server MUST send an `ainm.memory.compile` notification to the user's connected clients. This enables the client UI to refresh the memory graph view.

**Notification:**

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.memory.compile",
  "params": {
    "user_identity": "alice@messaging.example.com",
    "compilation_id": "019580d0-aaaa-7000-8000-111111111111",
    "timestamp": "2026-04-09T15:30:00.000Z",
    "source_threads": ["019580a0-1234-7000-8000-000000000001"],
    "result": {
      "nodes_created": 5,
      "nodes_updated": 2,
      "nodes_superseded": 1,
      "edges_created": 8,
      "edges_updated": 0,
      "contradictions_resolved": 1,
      "lint_warnings": 0
    }
  }
}
```

The compilation notification is delivered to the user's clients only -- NOT to agents in the thread. Agents learn about compound memory through the briefing protocol (Section 6), not through compilation notifications.

### 5.5 Lint Operation

The lint operation scans the user memory graph for health issues. Servers MUST support lint as a request/response method.

**Request:**

```json
{
  "jsonrpc": "2.0",
  "id": 200,
  "method": "ainm.memory.lint",
  "params": {
    "scope": "full"
  }
}
```

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `scope` | string | OPTIONAL | `"full"` (scan entire graph) or `"recent"` (scan only nodes created since last lint). Default: `"recent"`. |

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 200,
  "result": {
    "lint_id": "019580d1-bbbb-7000-8000-222222222222",
    "timestamp": "2026-04-09T15:35:00.000Z",
    "findings": [
      {
        "type": "contradiction",
        "severity": "warning",
        "description": "Two active nodes assert different preferred databases",
        "node_ids": ["019580a0-1111-...", "019580a0-2222-..."]
      },
      {
        "type": "orphan",
        "severity": "info",
        "description": "Node has no edges connecting it to the rest of the graph",
        "node_ids": ["019580a0-3333-..."]
      },
      {
        "type": "stale",
        "severity": "info",
        "description": "Node has not been reinforced in over 6 months",
        "node_ids": ["019580a0-4444-..."]
      }
    ],
    "summary": {
      "total_nodes": 347,
      "total_edges": 891,
      "contradictions": 1,
      "orphan_nodes": 1,
      "stale_nodes": 1
    }
  }
}
```

**Finding types:**

| Type | Description |
|------|-------------|
| `contradiction` | Two or more currently valid nodes assert conflicting facts. |
| `orphan` | A node with no incoming or outgoing edges. May indicate incomplete extraction. |
| `stale` | A node that has not been referenced, reinforced, or updated within a server-defined freshness window. |
| `gap` | A pattern detected but not yet captured as a node (e.g., the user has discussed a topic repeatedly but no procedural node exists for it). |
| `overconnected` | A node approaching the maximum edges-per-node limit. May indicate a "god node" that should be decomposed. |

## 6. Agent Briefing Protocol

Agent briefing is the mechanism by which new agents receive relevant context from the user's compound memory. Briefings are scoped views of the memory graph, filtered by the agent's capabilities and the user's consent level.

### 6.1 Briefing Flow

When an agent joins a thread, the server performs the following briefing flow:

```
Agent joins thread          Server processes briefing
(ainm.thread.join)          request
     |                           |
     v                           v
+-------------+          +------------------+
| Agent Card  |--------->| Scope Resolution |
| capabilities|          | (match caps to   |
| declared    |          |  memory scopes)  |
+-------------+          +------------------+
                                |
                                v
                         +------------------+
                         | Consent Check    |
                         | (user's consent  |
                         |  level applied)  |
                         +------------------+
                                |
                                v
                         +------------------+
                         | Subgraph         |
                         | Extraction       |
                         | (relevant nodes  |
                         |  + edges)        |
                         +------------------+
                                |
                                v
                         +------------------+
                         | Budget Assembly  |
                         | (fit within      |
                         |  token budget)   |
                         +------------------+
                                |
                                v
                         +------------------+
                         | Briefing         |
                         | Delivery         |
                         | (ainm.memory.    |
                         |  briefing)       |
                         +------------------+
```

### 6.2 Briefing Triggers

The server MUST deliver a briefing in the following situations:

1. **Thread join** -- When an agent joins a thread and the server has compound memory enabled for the user, the server MUST include a briefing alongside the context frame. The briefing is delivered as a separate `ainm.memory.briefing` notification immediately after the `ainm.context.frame` notification.

2. **Explicit request** -- An agent MAY request a briefing at any time via `ainm.memory.briefing` as a JSON-RPC request. This is useful when an agent needs to refresh its understanding of the user after a long conversation.

The server SHOULD NOT proactively re-deliver briefings unless the user's compound memory has changed significantly since the last briefing (server-defined threshold).

### 6.3 Briefing Structure

**Notification (server-initiated):**

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.memory.briefing",
  "params": {
    "briefing_id": "019580e0-cccc-7000-8000-333333333333",
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "user_identity": "alice@messaging.example.com",
    "generated_at": "2026-04-09T16:00:00.000Z",
    "scope": "domain-scoped",
    "consent_level": "standard",
    "sections": {
      "identity": {
        "name": "Alice",
        "location": "Madrid, Spain",
        "timezone": "Europe/Madrid",
        "role": "Senior Software Engineer"
      },
      "preferences": [
        {
          "node_id": "019580a0-pref-0001-...",
          "label": "Prefers TypeScript over JavaScript",
          "confidence": 0.95,
          "valid_from": "2025-06-01T00:00:00.000Z"
        },
        {
          "node_id": "019580a0-pref-0002-...",
          "label": "Uses 2-space indentation",
          "confidence": 1.0,
          "valid_from": "2024-01-01T00:00:00.000Z"
        }
      ],
      "expertise": [
        {
          "node_id": "019580a0-skill-0001-...",
          "label": "Expert in Rust systems programming",
          "confidence": 0.9,
          "valid_from": "2023-03-01T00:00:00.000Z"
        }
      ],
      "active_goals": [
        {
          "node_id": "019580a0-goal-0001-...",
          "label": "Building an AI-native messaging protocol",
          "confidence": 1.0,
          "valid_from": "2026-02-15T00:00:00.000Z"
        }
      ],
      "interaction_patterns": [
        {
          "node_id": "019580a0-pat-0001-...",
          "label": "Prefers concise responses without trailing summaries",
          "confidence": 0.85,
          "valid_from": "2026-01-10T00:00:00.000Z"
        }
      ],
      "relevant_history": [
        {
          "node_id": "019580a0-ep-0001-...",
          "label": "Completed Phase 13 of iOS app implementation",
          "confidence": 1.0,
          "valid_from": "2026-03-30T00:00:00.000Z"
        }
      ]
    },
    "graph_snapshot": {
      "nodes": ["...relevant subgraph nodes (full MemoryNode objects)"],
      "edges": ["...relevant subgraph edges (full MemoryEdge objects)"]
    },
    "token_budget_used": 4096
  }
}
```

**Briefing fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `briefing_id` | string | REQUIRED | UUIDv7 identifier for this briefing instance. |
| `thread_id` | string | REQUIRED | The thread context for this briefing. |
| `user_identity` | string | REQUIRED | AINM address of the user whose compound memory is being briefed. |
| `generated_at` | string | REQUIRED | RFC 3339 timestamp of briefing generation. |
| `scope` | string | REQUIRED | `"full"`, `"domain-scoped"`, or `"minimal"`. Indicates the breadth of memory included. |
| `consent_level` | string | REQUIRED | The user's consent level applied to this briefing: `"minimal"`, `"standard"`, or `"full"`. |
| `sections` | object | REQUIRED | Structured briefing content organized by category. See Section 6.4. |
| `sections.identity` | object | OPTIONAL | Core identity facts (name, location, role). Included at all consent levels. |
| `sections.preferences` | array | OPTIONAL | Relevant preferences as node summaries. |
| `sections.expertise` | array | OPTIONAL | Skill and expertise areas. |
| `sections.active_goals` | array | OPTIONAL | Current goals and projects. |
| `sections.interaction_patterns` | array | OPTIONAL | Communication and behavioral patterns. |
| `sections.relevant_history` | array | OPTIONAL | Contextually relevant episodic nodes. |
| `graph_snapshot` | object | OPTIONAL | Raw graph data (nodes + edges) for the scoped subgraph. Included only when `scope` is `"full"` or `"domain-scoped"`. |
| `token_budget_used` | integer | OPTIONAL | Approximate token count of the briefing payload. |

### 6.4 Briefing Sections

Each section maps to specific node categories from the memory graph:

| Section | Node Categories Included | Consent Level Required |
|---------|-------------------------|----------------------|
| `identity` | `fact` (biographical) | `minimal` |
| `preferences` | `preference`, `tool` | `standard` |
| `expertise` | `skill` | `standard` |
| `active_goals` | `event`, `decision` (recent, goal-related) | `standard` |
| `interaction_patterns` | `pattern`, `communication`, `workflow` | `full` |
| `relevant_history` | `event`, `decision`, `outcome`, `interaction` | `full` |
| `graph_snapshot` | All scoped nodes + edges | `full` |

### 6.5 Capability-Based Scoping

The server determines which memory graph regions are relevant to an agent by matching the agent's declared capabilities (from its Agent Card, `agent-card.md`) against memory scope definitions.

**Scope resolution rules:**

1. The server reads the agent's Agent Card, specifically the `skills` array and the `ainm.contentTypes` array.
2. For each skill declared by the agent, the server identifies relevant node categories. For example, an agent with skill description containing "code review" or "software development" is matched to nodes with categories `preference` (coding-related), `skill` (technical), `tool`, and `decision` (technical decisions).
3. The server filters the memory graph to include only nodes matching the resolved categories, applying the user's consent level as an additional filter.
4. If the agent declares an `ainm.memory` section in its Agent Card requesting specific scopes (see Section 10.2), those scopes take precedence over skill-based inference.

Scope resolution is implementation-defined. The protocol defines the scopes and their semantics, but the matching algorithm between agent capabilities and memory scopes is left to server implementations. Servers SHOULD err on the side of including relevant context rather than being overly restrictive -- under-briefed agents perform poorly.

### 6.6 Briefing Acknowledgment

After receiving a briefing, agents SHOULD acknowledge receipt:

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.memory.briefing.ack",
  "params": {
    "briefing_id": "019580e0-cccc-7000-8000-333333333333"
  }
}
```

The acknowledgment is a notification (no response expected). It enables the server to track which agents have been successfully briefed and to detect agents that did not process their briefing (e.g., due to disconnection).

## 7. Access Control and Sharing

### 7.1 Memory Scopes

Memory scopes define named subsets of the user memory graph that can be independently controlled. Each scope maps to a set of node categories.

| Scope Name | Node Categories | Description |
|-----------|----------------|-------------|
| `identity` | `fact` (biographical) | Core identity: name, location, timezone, role |
| `preferences` | `preference`, `tool` | Choices, settings, tooling |
| `expertise` | `skill` | Technical and domain expertise |
| `goals` | `event` (goal-related), `decision` (recent) | Current objectives and active projects |
| `history` | `event`, `decision`, `outcome`, `interaction` | Past events and their outcomes |
| `patterns` | `pattern`, `workflow`, `strategy`, `communication` | Behavioral and communication patterns |

### 7.2 Consent Levels

Users set a global consent level that determines the baseline memory sharing behavior:

| Level | Scopes Included | Description |
|-------|----------------|-------------|
| `minimal` | `identity` only | Only basic identity facts are shared. Agents receive no preferences, expertise, or history. Suitable for untrusted or new agents. |
| `standard` | `identity` + `preferences` + `expertise` + `goals` | The recommended default. Agents receive enough context to personalize their behavior without exposing behavioral patterns or detailed history. |
| `full` | All scopes | Complete memory sharing. Agents receive the full scoped subgraph including behavioral patterns and detailed history. Suitable for trusted, long-term agents. |

The user MAY override the global consent level for specific agents. For example, a user with `standard` consent MAY grant `full` access to their primary coding assistant while keeping `minimal` for a newly added agent.

### 7.3 Grant and Revoke Operations

**Grant:**

```json
{
  "jsonrpc": "2.0",
  "id": 300,
  "method": "ainm.memory.grant",
  "params": {
    "agent_identity": "code-bot@agents.example.com",
    "scopes": ["identity", "preferences", "expertise", "goals", "patterns"],
    "consent_level": "full"
  }
}
```

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 300,
  "result": {
    "granted": true,
    "agent_identity": "code-bot@agents.example.com",
    "effective_scopes": ["identity", "preferences", "expertise", "goals", "patterns"],
    "effective_consent_level": "full"
  }
}
```

**Revoke:**

```json
{
  "jsonrpc": "2.0",
  "id": 301,
  "method": "ainm.memory.revoke",
  "params": {
    "agent_identity": "code-bot@agents.example.com",
    "scopes": ["patterns", "history"]
  }
}
```

Grant and revoke operations are user-initiated only. Agents MUST NOT call `ainm.memory.grant` or `ainm.memory.revoke` -- these operations require the user's authenticated session. The server MUST reject grant/revoke requests from agent sessions with error code `-32061` (unauthorized memory operation).

When a revoke takes effect, the server MUST NOT include the revoked scopes in future briefings for that agent. The server SHOULD NOT retroactively inform the agent that its access was revoked -- the revocation silently reduces future briefing scope.

### 7.4 Agent Contribution to Compound Memory

Agents contribute to the user's compound memory in two ways:

1. **Implicit contribution** -- Through normal conversation. The compilation engine processes all messages in the thread, including agent messages, and extracts knowledge about the user.

2. **Explicit contribution** -- Via the `ainm.memory.graph.contribute` method. Agents MAY contribute nodes directly when they learn something notable about the user:

```json
{
  "jsonrpc": "2.0",
  "id": 400,
  "method": "ainm.memory.graph.contribute",
  "params": {
    "thread_id": "019580a0-1234-7000-8000-000000000001",
    "node": {
      "type": "semantic",
      "category": "preference",
      "label": "Prefers functional programming patterns over OOP",
      "confidence": 0.85,
      "properties": {
        "context": "User consistently used map/filter/reduce instead of class-based patterns across 3 code reviews"
      },
      "tags": ["coding", "architecture"]
    },
    "edges": [
      {
        "target_label": "TypeScript",
        "relationship": "uses",
        "confidence": 0.9
      }
    ]
  }
}
```

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 400,
  "result": {
    "contributed": true,
    "node_id": "019580f0-aaaa-7000-8000-444444444444",
    "edge_ids": ["019580f0-bbbb-7000-8000-555555555555"]
  }
}
```

Contributed nodes are flagged with the contributing agent's identity in `source.contributed_by`. The server MAY apply a moderation step before integrating contributed nodes into the graph -- this is implementation-defined. Contributed nodes SHOULD start with confidence no higher than 0.85 (agent contributions are treated as observations, not ground truth, unless the user confirms them).

## 8. Portability

### 8.1 Export Format: ainm-memory-v1

The `ainm-memory-v1` format is a self-contained JSON document that captures a user's complete compound memory for transfer between providers.

```json
{
  "format": "ainm-memory-v1",
  "version": "1.0.0",
  "exported_at": "2026-04-09T18:00:00.000Z",
  "user_identity": "alice@messaging.example.com",
  "graph": {
    "nodes": [
      {
        "node_id": "019580a0-aaaa-7000-8000-000000000001",
        "type": "semantic",
        "category": "preference",
        "label": "Prefers TypeScript over JavaScript",
        "properties": { "strength": "strong" },
        "confidence": 0.95,
        "source": {
          "contributed_by": "compiler@messaging.example.com",
          "extracted_at": "2026-03-15T10:00:00.000Z"
        },
        "temporal": {
          "valid_from": "2025-06-01T00:00:00.000Z",
          "valid_until": null,
          "superseded_by": null
        },
        "tags": ["coding", "language"]
      }
    ],
    "edges": [
      {
        "edge_id": "019580a0-eeee-7000-8000-000000000001",
        "source_node": "019580a0-aaaa-7000-8000-000000000001",
        "target_node": "019580a0-aaaa-7000-8000-000000000002",
        "relationship": "prefers_over",
        "confidence": 0.95,
        "source": {
          "contributed_by": "compiler@messaging.example.com",
          "extracted_at": "2026-03-15T10:00:00.000Z"
        },
        "temporal": {
          "valid_from": "2025-06-01T00:00:00.000Z",
          "valid_until": null,
          "superseded_by": null
        }
      }
    ]
  },
  "metadata": {
    "node_count": 347,
    "edge_count": 891,
    "earliest_node": "2024-01-15T00:00:00.000Z",
    "latest_node": "2026-04-09T15:30:00.000Z",
    "export_source_server": "messaging.example.com",
    "export_protocol_version": "0.1.0"
  }
}
```

### 8.2 Export Operation

```json
{
  "jsonrpc": "2.0",
  "id": 500,
  "method": "ainm.memory.export",
  "params": {
    "format": "ainm-memory-v1",
    "include_superseded": false,
    "include_provenance": true
  }
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `format` | string | REQUIRED | MUST be `"ainm-memory-v1"`. Future versions may add additional formats. |
| `include_superseded` | boolean | OPTIONAL | Whether to include nodes/edges that have been superseded (i.e., `valid_until` is set). Default: `false`. |
| `include_provenance` | boolean | OPTIONAL | Whether to include full provenance metadata (thread_id, message_id, compilation_pass). Default: `true`. When `false`, only `contributed_by` and `extracted_at` are included. |

The server MUST return the complete export document in the response. For large graphs, the server MAY stream the response using the streaming mechanism defined in `streaming.md` with content type `application/json`.

Export is a user-only operation. Agents MUST NOT call `ainm.memory.export`. The server MUST reject export requests from agent sessions with error `-32061`.

### 8.3 Import Operation

```json
{
  "jsonrpc": "2.0",
  "id": 501,
  "method": "ainm.memory.import",
  "params": {
    "format": "ainm-memory-v1",
    "data": { "...the full ainm-memory-v1 document..." },
    "conflict_strategy": "prefer-newer"
  }
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `format` | string | REQUIRED | MUST be `"ainm-memory-v1"`. |
| `data` | object | REQUIRED | The complete `ainm-memory-v1` document to import. |
| `conflict_strategy` | string | OPTIONAL | How to handle conflicts with existing nodes. Default: `"prefer-newer"`. |

**Conflict strategies:**

| Strategy | Behavior |
|----------|----------|
| `prefer-newer` | When an imported node conflicts with an existing node, keep the one with the more recent `valid_from`. Supersede the older one. |
| `prefer-existing` | Keep existing nodes; only import nodes that have no conflict. |
| `keep-both` | Import all nodes, flagging conflicts as `contradicts` edges for manual resolution. |

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 501,
  "result": {
    "imported": true,
    "nodes_imported": 280,
    "nodes_skipped": 67,
    "nodes_merged": 12,
    "edges_imported": 650,
    "edges_skipped": 141,
    "conflicts_detected": 5,
    "conflicts_resolved": 5
  }
}
```

Import is a user-only operation. Servers MUST validate the imported document against the `ainm-memory-v1` schema before processing.

### 8.4 Federation Memory Sharing

In a federated deployment, a user's compound memory resides on their home server. When the user participates in threads on remote servers, the remote server MAY request a briefing from the home server to provide to agents on the remote server.

**Federation briefing flow:**

1. User `alice@server-a.com` joins a thread on `server-b.com`.
2. `server-b.com` recognizes that Alice's home server is `server-a.com` (from identity resolution, `identity.md` Section 4).
3. `server-b.com` sends a server-to-server briefing request to `server-a.com` via the federation relay (`transport.md` Section 7).
4. `server-a.com` checks Alice's federation memory sharing setting (see below).
5. If permitted, `server-a.com` generates a briefing scoped to Alice's federation consent level and returns it.
6. `server-b.com` delivers the briefing to agents in the thread alongside the context frame.

**Federation consent:** Users MUST explicitly opt in to federation memory sharing. By default, compound memory is NOT shared with federated servers. Users MAY set a federation consent level independently from their local consent level. The federation consent level MUST NOT exceed the local consent level.

## 9. Privacy and Governance

### 9.1 User Rights

The following user rights are non-negotiable protocol requirements:

| Right | Requirement |
|-------|------------|
| **Right to view** | Users MUST be able to view all nodes and edges in their compound memory graph. No hidden or system-only nodes. |
| **Right to delete** | Users MUST be able to delete any node or edge from their graph. Deletion MUST be permanent and MUST cascade to orphaned edges. The server MUST NOT retain deleted nodes for any purpose. |
| **Right to export** | Users MUST be able to export their complete compound memory at any time (Section 8.2). |
| **Right to disable** | Users MUST be able to disable compound memory entirely. When disabled, the server MUST stop compilation, delete the graph, and revert to base three-tier memory behavior. |
| **Right to control sharing** | Users MUST be able to set consent levels and per-agent overrides (Section 7.2, 7.3). |
| **Right to transparency** | Users MUST be able to see which agents have received briefings and what scope each briefing contained. The server MUST maintain a briefing audit log. |

### 9.2 Data Minimization

Servers MUST practice data minimization in compound memory:

- The compilation engine MUST NOT extract knowledge that is not relevant to understanding the user. Implementation-specific chatter, protocol metadata, and system messages MUST NOT become memory nodes.
- Servers MUST NOT store raw conversation transcripts in the compound memory graph. Only extracted, structured knowledge is stored.
- Servers MUST NOT use compound memory data for purposes other than serving the user's agents, unless the user has explicitly consented to additional uses (e.g., model training, analytics).

### 9.3 Memory Poisoning Prevention

Memory poisoning occurs when a malicious agent contributes false or misleading nodes to the user's compound memory. Servers MUST implement countermeasures:

- **Agent-contributed nodes** (via `ainm.memory.graph.contribute`) MUST be flagged with the contributing agent's identity and SHOULD be subject to moderation or user confirmation before full integration.
- **Confidence capping**: Agent-contributed nodes MUST NOT have confidence scores above 0.85. Only compilation-extracted nodes from explicit user statements may reach 1.0.
- **Rate limiting**: The server MUST limit the rate of `ainm.memory.graph.contribute` calls per agent per hour. Recommended limit: 50 contributions per agent per hour.
- **Contradiction alerts**: When a contributed node contradicts existing high-confidence nodes, the server SHOULD alert the user rather than silently superseding.

### 9.4 Prompt Injection Resistance

The compilation engine processes conversation messages that may contain adversarial content. The following safeguards apply:

- The compilation engine's LLM MUST use system prompts that clearly separate user conversation content from extraction instructions.
- Extracted labels and properties MUST be treated as untrusted text. Clients MUST sanitize node labels before display (preventing XSS or injection into UI rendering).
- The compilation engine MUST NOT execute instructions found within conversation messages. For example, a message saying "remember that my name is [DROP TABLE users]" MUST result in a node with a literal label containing that text, not a database operation.

## 10. Backward Compatibility

### 10.1 Capability Negotiation

Compound memory is negotiated during `ainm.initialize` as an extension to the existing memory capabilities.

**Server declares compound memory support in `ainm.initialized`:**

```json
{
  "jsonrpc": "2.0",
  "method": "ainm.initialized",
  "params": {
    "capabilities": {
      "memory": {
        "conversation": true,
        "archival": true,
        "graph": true,
        "compilation": true,
        "briefing": true,
        "export": true,
        "maxGraphNodes": 10000,
        "maxGraphEdges": 50000,
        "supportedExportFormats": ["ainm-memory-v1"]
      }
    }
  }
}
```

**New capability fields:**

| Field | Type | Description |
|-------|------|-------------|
| `memory.graph` | boolean | Server supports the user memory graph and graph query/contribute operations. |
| `memory.compilation` | boolean | Server runs LLM-powered memory compilation. |
| `memory.briefing` | boolean | Server generates agent briefings from compound memory. |
| `memory.export` | boolean | Server supports memory export/import. |
| `memory.maxGraphNodes` | integer | Maximum nodes per user memory graph. |
| `memory.maxGraphEdges` | integer | Maximum edges per user memory graph. |
| `memory.supportedExportFormats` | array of strings | Export formats supported. |

Servers that set `memory.graph` to `false` (or omit the field) operate with base three-tier memory only. Clients and agents MUST check this capability before invoking compound memory operations. Invoking a compound memory operation on a server that does not support it returns error `-32062` (compound memory not supported).

### 10.2 Agent Card Extension

Agents that are aware of compound memory MAY declare memory scope preferences in their Agent Card:

```json
{
  "ainm": {
    "memory": {
      "requestedScopes": ["identity", "preferences", "expertise"],
      "minimumScopes": ["identity"],
      "supportsBriefing": true
    }
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `ainm.memory.requestedScopes` | array of strings | The memory scopes the agent would ideally receive. The server uses these to scope briefings when capability-based inference is ambiguous. |
| `ainm.memory.minimumScopes` | array of strings | The minimum scopes required for the agent to function effectively. If the user's consent level does not cover these scopes, the server MAY warn the user. |
| `ainm.memory.supportsBriefing` | boolean | Whether the agent understands and can process `ainm.memory.briefing` notifications. Agents that do not support briefing still benefit from compound memory indirectly through enriched context frames. |

### 10.3 Interoperability

**Relationship to MCP:** The compound memory graph can be exposed as an MCP resource via the MCP bridge defined in `compatibility.md`. This allows MCP-compatible tools to query the user's memory graph through standard MCP `resources/read` operations, bridging the gap between AINM-native agents and MCP tool chains.

**Relationship to Mem0:** The `ainm-memory-v1` export format is designed to be transformable to and from Mem0's memory format. A bridge utility can convert between the two formats, enabling users to import memories from Mem0-based systems into AINM compound memory. The key mapping is: Mem0 `memory` records → AINM semantic nodes; Mem0 `entities` → AINM nodes with category `relationship` or `fact`; Mem0 `relations` → AINM edges.

**Relationship to Obsidian:** The compound memory graph can be exported as an Obsidian-compatible vault of markdown files. Each node becomes a markdown file with YAML frontmatter (properties, temporal, provenance) and the label as the file content. Edges become `[[wikilinks]]` between files. This enables users to browse and edit their compound memory using Obsidian's graph view, creating a bridge between the protocol and Karpathy's LLM knowledge base workflow.

## 11. Graph Operations (JSON-RPC Methods)

### 11.1 ainm.memory.graph.query

Queries the user's compound memory graph.

**Direction:** Client to server (request/response).

```json
{
  "jsonrpc": "2.0",
  "id": 600,
  "method": "ainm.memory.graph.query",
  "params": {
    "query": "typescript preferences",
    "type_filter": ["semantic"],
    "category_filter": ["preference", "tool"],
    "temporal_filter": "current",
    "min_confidence": 0.5,
    "limit": 20,
    "include_edges": true
  }
}
```

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | OPTIONAL | Text search across node labels and properties. If omitted, returns all nodes matching filters. |
| `type_filter` | array of strings | OPTIONAL | Filter by node type: `"semantic"`, `"episodic"`, `"procedural"`. If omitted, all types are included. |
| `category_filter` | array of strings | OPTIONAL | Filter by node category. If omitted, all categories are included. |
| `temporal_filter` | string | OPTIONAL | `"current"` (only nodes where `valid_until` is null), `"all"` (include superseded), or `"historical"` (only superseded). Default: `"current"`. |
| `min_confidence` | number | OPTIONAL | Minimum confidence score. Default: 0.0. |
| `limit` | integer | OPTIONAL | Maximum results. Default: 20. Maximum: 100. |
| `include_edges` | boolean | OPTIONAL | Include edges connecting the returned nodes. Default: `false`. |

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 600,
  "result": {
    "nodes": ["...array of MemoryNode objects"],
    "edges": ["...array of MemoryEdge objects (if include_edges=true)"],
    "total_count": 42,
    "returned_count": 20
  }
}
```

Graph queries from agents are subject to scoping. An agent can only query nodes within the scopes granted by the user's consent level and per-agent overrides. The server silently filters out nodes outside the agent's scope.

### 11.2 ainm.memory.graph.traverse

Traverses the graph starting from a specific node, following edges up to a specified depth.

**Direction:** Client to server (request/response).

```json
{
  "jsonrpc": "2.0",
  "id": 601,
  "method": "ainm.memory.graph.traverse",
  "params": {
    "start_node": "019580a0-aaaa-7000-8000-000000000001",
    "max_depth": 2,
    "relationship_filter": ["uses", "prefers_over"],
    "direction": "outgoing",
    "temporal_filter": "current"
  }
}
```

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `start_node` | string | REQUIRED | `node_id` to start traversal from. |
| `max_depth` | integer | OPTIONAL | Maximum traversal depth. Default: 2. Maximum: 5. |
| `relationship_filter` | array of strings | OPTIONAL | Only follow edges with these relationship types. If omitted, all types are followed. |
| `direction` | string | OPTIONAL | `"outgoing"`, `"incoming"`, or `"both"`. Default: `"both"`. |
| `temporal_filter` | string | OPTIONAL | Same as graph.query. Default: `"current"`. |

**Response:**

```json
{
  "jsonrpc": "2.0",
  "id": 601,
  "result": {
    "subgraph": {
      "nodes": ["...array of MemoryNode objects in the traversed subgraph"],
      "edges": ["...array of MemoryEdge objects connecting them"]
    },
    "traversal_depth_reached": 2
  }
}
```

### 11.3 ainm.memory.graph.contribute

Allows agents to explicitly contribute knowledge to the user's compound memory. See Section 7.4 for the full specification and examples.

### 11.4 Error Codes

Compound memory operations use error codes in the `-32060` to `-32069` range:

| Code | Name | Description |
|------|------|-------------|
| `-32060` | Graph quota exceeded | The user's memory graph has reached the server's node or edge limit. |
| `-32061` | Unauthorized memory operation | An agent attempted an operation restricted to user sessions (export, import, grant, revoke). |
| `-32062` | Compound memory not supported | The server does not support compound memory. Check the `memory.graph` capability. |
| `-32063` | Invalid export format | The requested export format is not supported by the server. |
| `-32064` | Import validation failed | The imported document does not conform to the specified format schema. |
| `-32065` | Briefing not available | No compound memory exists for the user, or the user has disabled compound memory. |
| `-32066` | Node not found | The specified `node_id` does not exist in the user's memory graph. |
| `-32067` | Contribution rejected | The server rejected an agent's contribution (moderation, rate limiting, or policy). |
