# Phase 9: AI-Native Federation & Full Demo - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-29
**Phase:** 09-ai-native-federation-full-demo
**Areas discussed:** Cross-federation Agent Card resolution, Event relay strategy, Memory context on federated join, Demo scenario design

---

## Cross-federation Agent Card Resolution

| Option | Description | Selected |
|--------|-------------|----------|
| Public .well-known GET | Agent cards are public discovery info — resolve via HTTPS GET to @provider domain. Matches A2A pattern. | ✓ |
| Via mTLS relay endpoint | Resolve through authenticated federation relay (ainm.agent-card method). More secure but adds coupling. | |
| Hybrid: public with mTLS enrichment | Public GET for basic card, mTLS for enriched card with additional capabilities. | |

**User's choice:** Public .well-known GET
**Notes:** Consistent with existing server identity resolution pattern.

| Option | Description | Selected |
|--------|-------------|----------|
| In-memory with TTL | Cache in RemoteServerCache with TTL (e.g., 5 min). Simple, consistent with existing caching. | ✓ |
| Persistent in redb | Cache in redb alongside TOFU pins and offline queue. Survives restarts. | |
| No caching | Always fetch fresh. Simplest but adds latency. | |

**User's choice:** In-memory with TTL
**Notes:** None.

| Option | Description | Selected |
|--------|-------------|----------|
| Per-agent path | /.well-known/ainm/{agent-name}/agent-card.json — each agent has own URL. | ✓ |
| Agent directory endpoint | /.well-known/ainm/agents.json returns list of all agents. | |
| Both: directory + per-agent | Directory for listing, per-agent for fetching. | |

**User's choice:** Per-agent path
**Notes:** Clean and scalable.

---

## Event Relay Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse ainm.relay for all events | Each event wrapped individually in relay payload. Same path, same validation, same queue. | ✓ |
| New ainm.stream-relay method | Dedicated lightweight relay method for streaming. Skips full validation. | |
| Batch streaming relay | Buffer deltas and send in batches (e.g., every 100ms or 5 deltas). | |

**User's choice:** Reuse ainm.relay for all events
**Notes:** Uniform handling — simple and consistent.

| Option | Description | Selected |
|--------|-------------|----------|
| Same mechanism for both | Tool call and streaming events both relayed via ainm.relay. Uniform handling. | ✓ |
| Different handling | Standard relay for tool calls, optimized path for streaming. | |

**User's choice:** Same mechanism for both
**Notes:** None.

| Option | Description | Selected |
|--------|-------------|----------|
| Same broadcast channel | Streaming/tool events through same broadcast::channel as regular messages. REPL filters by type. | ✓ |
| Dedicated streaming channel | Separate broadcast::channel for streaming events. | |
| You decide | Claude's discretion. | |

**User's choice:** Same broadcast channel
**Notes:** Established pattern, no new plumbing needed.

---

## Memory Context on Federated Join

| Option | Description | Selected |
|--------|-------------|----------|
| On join — origin pushes context | Origin pushes active context frames immediately when remote agent joins. | ✓ |
| On demand — agent requests via backfill | Agent joins with no context, requests memory on demand. | |
| Hybrid: push summary + pull details | Push lightweight summary, agent pulls full entries on demand. | |

**User's choice:** On join — origin pushes context
**Notes:** Agent is ready from first message, no cold start.

| Option | Description | Selected |
|--------|-------------|----------|
| Active context frames only | Push only current active context frames. Lightweight, immediately useful. | ✓ |
| Active context + recent conversation | Push active context plus last N messages. | |
| All three tiers | Push active context, conversation summary, and archival. | |

**User's choice:** Active context frames only
**Notes:** Conversation history available via existing ainm.backfill if needed.

| Option | Description | Selected |
|--------|-------------|----------|
| New ainm.context-push relay method | Dedicated method on relay endpoint. Payload: thread_id + context frames array. | ✓ |
| Embed in participant-added event | Include context frames in participant notification payload. | |
| You decide | Claude's discretion. | |

**User's choice:** New ainm.context-push relay method
**Notes:** Sent once on join, through existing mTLS relay infrastructure.

---

## Demo Scenario Design

| Option | Description | Selected |
|--------|-------------|----------|
| Single comprehensive scenario | One demo script: discover card → stream → tool call → memory. Cohesive story. | ✓ |
| Separate per-feature scripts | Individual scripts per feature. Modular but less compelling. | |
| Both: comprehensive + individual | Comprehensive for showcase, individual for testing. | |

**User's choice:** Single comprehensive scenario
**Notes:** Tells a cohesive story of AINM's unique value proposition.

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse existing slash commands | Human types /stream, /weather, /memory — same commands, now cross federation. | ✓ |
| Natural conversation flow | Demo has human ask natural questions triggering features. | |
| You decide | Claude's discretion. | |

**User's choice:** Reuse existing slash commands
**Notes:** Consistent with Phase 7/8 demo style.

| Option | Description | Selected |
|--------|-------------|----------|
| New /discover command | /discover helper@server-b.local resolves and displays remote agent card. | ✓ |
| Automatic on thread join | Server auto-resolves and displays card when remote agent joins. | |
| You decide | Claude's discretion. | |

**User's choice:** New /discover command
**Notes:** Explicit demonstration of AINF-01.

---

## Claude's Discretion

- Internal relay payload format for streaming/tool events
- TTL duration for agent card cache
- Active context frame selection logic on origin server
- Demo script structure and output formatting
- Order of relay_handler match arms for ainm.context-push

## Deferred Ideas

None — discussion stayed within phase scope
