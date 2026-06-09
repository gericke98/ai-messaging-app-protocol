# Runtime A2A (Agent-to-Agent Messaging) — Design

**Date:** 2026-06-09
**Status:** Approved (design); pending implementation plans per phase
**Scope:** Full runtime A2A including federation, delivered in three independently-shippable phases (P1 same-server core → P2 consent policy → P3 federation)

---

## 1. Problem & Goal

Today the AINM server is strictly **human ↔ single-agent**. A human's message is *blind-broadcast* to every connected agent over a single `to_agent_tx` channel (no recipient, no thread filtering); connector agents **always reply** and **cannot tell** whether a sender was a human or another agent; agent replies only ever flow back to humans via `HumanRegistry::send_to_thread`. There is **no agent→agent code path** (`AgentRegistry::send_to_agent` exists but is `#[allow(dead_code)]`), **no recipient field** honored by the router (the envelope's `to` slot is read in exactly one place, `server.rs:2580`, purely as a hosted-assistant trigger heuristic), and **no loop guard** on the delivery path (only the hosted agent's internal 8-tool-round cap).

Meanwhile the entire A2A *display* arc is already shipped and production-grade but exercises **only seeded data**: SP3 directional bylines (`MessageGroup.directedTo`, `isMultiParticipant`), SP4 digest capsule, the participant roster, and `ParticipantProvenance`. None of it sees live agent-to-agent traffic because the server cannot produce any.

**Goal:** Make multi-agent threads real. An agent can address another agent (same-server or, ultimately, cross-server) in a thread; the server routes the message to the target agent's runtime, which can reply, producing genuine A2A bursts that the existing UI already knows how to render — with the human always able to break the loop, and consent grounded in thread membership. This is the unlock that makes SP1–SP4 meaningful in production.

**Non-goals (YAGNI):** per-account allow-lists, a human first-contact approval queue, a shared cross-agent token budget, an "observer" agent mode that responds without being addressed (the data to build it is exposed, but the policy is not forced in v1).

---

## 2. Current-State Reference (verified 2026-06-09)

All file:line citations are against `server/src/` and `packages/connector/` at the time of writing; verify before relying on them in a plan.

- **Human inbound:** `handle_human_connection` (`server.rs:2285`); `ainm.message.send` branch at `server.rs:2442`; signature verify `server.rs:2451`; delivery wraps as `ainm.message.deliver` and **broadcasts** on `to_agent_tx` (`server.rs:2522-2533`); persist `store_and_maybe_compile(.., false)` (`server.rs:2560`).
- **Agent inbound:** `handle_agent_connection` (`server.rs:1692`) — a **separate** handler; `ainm.message.send` at `server.rs:1750`; verify `server.rs:1763`; agent reply delivered to humans via `human_registry.send_to_thread` (`server.rs:1786/1798`); persist with `trigger_compilation = true` (`server.rs:1828`).
- **Delivery is thread-scoped, never recipient-addressed.** `to_agent_tx` is `broadcast::Sender<String>` (`server.rs:245`, created `server.rs:767`) → every agent gets every delivered message. `HumanRegistry::send_to_thread` (`server.rs:113`) iterates `thread_participants[thread_id]` → `connections[user_addr]` → per-connection senders, falling back to broadcast-to-all humans if the thread has no tracked participants.
- **Hosted-agent trigger (only addressing that exists):** `server.rs:2574-2599` computes `targets_agent = thread.contains("assistant@") || params.to == agent.address || content_mentions(content, agent.address)`; if true spawns `runtime_server::handle_turn` (`runtime_server.rs:15`), which streams back via `send_to_thread`.
- **Envelope:** no typed Rust struct — handled as `serde_json::Value`. Fields on the wire: `id`, `from`, `thread_id`, `type:"message"`, `content[]`, `timestamp`, `signature`; deliver wrapper adds `verified`. **No `to`, no `sender_type`, no structured mentions** in the canonical envelope. Method strings: `ainm.message.send`, `ainm.message.deliver`, `ainm.stream.start/delta/end/cancel`, `ainm.memory.*`, `ainm.thread.create`, `ainm.initialize`/`ainm.initialized`.
- **Registries:** `HumanRegistry` (`server.rs:60`) = `connections: HashMap<user_addr, Vec<Sender>>` + `thread_participants: HashMap<thread_id, HashSet<user_addr>>`. `AgentRegistry` (`server.rs:148`) = `connections: HashMap<agent_identity, Vec<Sender>>` + `agent_cards`. **`AgentRegistry` is NOT used for inbound delivery today** — human→agent uses the blind broadcast; the registry only serves cards + cleanup. `send_to_agent` (`server.rs:218`) / `send_to_all_agents` (`server.rs:229`) are dead code.
- **Loop guards:** hosted-agent tool-round cap (`runtime.rs`, `max_tool_rounds = 8`), token quota (`MeteredLlmClient`, `runtime_server.rs:110`), connector RPC timeout 30s. **No per-thread rate limit / max-messages / recursion guard on delivery.**
- **Signing:** `signing::sign_envelope` / `verify_signature` (`server.rs` signing module), RFC 8785 canonicalization, Ed25519 via `ring`. Inbound verification is **advisory** (`verified` flag) — the hard gate is handshake `verify_init` (`server.rs:1178`) when a DB is configured. `messages.signature` column **exists but is never populated** by `store_and_maybe_compile`.
- **Connector SDK:** `MessageHandler = (IncomingMessage, MessageContext) => Promise<string|void>` (`types.ts:64`). `IncomingMessage` (`types.ts:14`) exposes `id/from/threadId/timestamp/text/contentBlocks/raw` — **no `senderType`, no `to`, no `addressed`**. Reply via returning a string or `ctx.sendMessage(content)` (`agent.ts:227`) — envelope carries `thread_id` + content, **no `to`**. Reference connectors (`openai-agent.ts`, `claude-code-agent.ts`) **always respond**, no gating.
- **Persistence:** `store_and_maybe_compile` (`server.rs:1256`) inserts `(thread_id, sequence, from_addr, content)` into `messages` (`migrations/001_base_tables.sql`); **no recipient/`to` column**; `signature` left unpopulated.

---

## 3. Architecture

### 3.1 Envelope changes (protocol)

Promote the dormant `to` slot to a first-class documented field and add server-stamped sender typing:

```
message envelope (ainm.message.send → ainm.message.deliver)
  id           : string            (exists)
  from         : address           (exists; sender-asserted, signature-bound)
  to           : address | null    (PROMOTED — canonical routing target)
  thread_id    : string            (exists)
  sender_type  : "human" | "agent" (NEW — SERVER-STAMPED on deliver, never trusted from payload)
  content      : [ContentBlock]    (exists)
  timestamp    : string            (exists)
  signature    : string            (exists)
```

- `to` is part of the **signed** envelope on send (canonicalization already covers all present fields, so adding `to` is signature-compatible — a sender that sets `to` signs over it).
- `sender_type` is **added by the server to the `deliver` wrapper**, derived from the connection's handshake classification (`is_agent`, `server.rs:1237`). It is NOT part of what the sender signs and is NOT read from the sender's payload — a connector cannot spoof being a human.
- The existing `verified` flag on the deliver wrapper is retained and additionally surfaced to connectors (see §3.4).

### 3.2 Recipient resolution (`resolve_recipient`, pure)

A new pure function computes the routing target for an inbound message, replacing the ad-hoc hosted-agent heuristic:

```
resolve_recipient(env, thread_participants) -> Option<address>:
  1. if env.to is set AND is a known participant of thread → Some(to)
  2. else if content has a leading @mention of a thread participant → Some(resolved_address)
  3. else → None   (no agent target; humans observe)
```

- Mention parsing reuses the same `@localpart` notion as `content_mentions` (`server.rs:22`) and mirrors the iOS `MentionTarget.firstMentionAddress` semantics so client and server agree on `directedTo`.
- The **hosted-assistant trigger is folded into this function** — `assistant@…` is resolved like any other agent target; the special-case `thread.contains("assistant@")` heuristic is removed.
- Pure and exhaustively unit-tested (to-set / mention / none, unknown target, non-participant target).

### 3.3 Delivery (`route_message`, replaces the broadcast)

A single dispatch path after persistence:

```
route_message(env):
  target = resolve_recipient(env, participants)
  persist(env, to_addr = target)          # to_addr column + populate signature

  # Humans in the thread ALWAYS get a display copy (UI renders A2A via SP3/SP4)
  human_registry.send_to_thread(thread_id, deliver(env))

  match target:
    Some(addr) if is_agent(addr):
        # SAFETY LAYER runs here — see §3.5
        if a2a_guard_and_consent_ok(env, addr, thread_id):
            agent_registry.send_to_agent(addr, deliver(env))   # ACTIVATE dead code
    Some(addr) if is_human(addr):
        # already delivered above (humans always get it)
        ()
    None:
        ()   # humans observe; no agent invoked
```

- **`AgentRegistry::send_to_agent`** (currently dead) becomes the agent-delivery primitive, keyed by agent identity → connection sender. The blind `to_agent_tx` broadcast is **removed**.
- Agents receive a message **only** if they are the resolved target AND have joined the thread. Humans always receive every thread message for display (unchanged human experience; that is what the capsule/byline render).
- **Agent thread-membership:** agents currently "join" implicitly by receiving the broadcast. With targeted delivery we add a symmetric agent-side membership set (an agent joins a thread when first addressed in it, or when explicitly added via the participant roster path). This set is the consent boundary in §3.5.

### 3.4 Connector SDK contract

`IncomingMessage` (`packages/connector/src/protocol/types.ts`) gains three fields; all existing fields and handlers remain valid:

```ts
export interface IncomingMessage {
  id: string;
  from: string;
  threadId: string;
  timestamp: string;
  text: string;
  contentBlocks: ContentBlock[];
  // NEW:
  senderType: "human" | "agent";   // server-stamped, read from the deliver wrapper
  to: string | null;               // canonical target the server resolved
  addressed: boolean;              // to === this agent's own address
  verified: boolean;               // surfaced from the deliver wrapper (was dropped)
  raw: unknown;
}
```

- `handleMessageDelivery` (`agent.ts:192`) reads `sender_type`/`to`/`verified` off the wrapper and populates these.
- **Default gating:** reference connectors adopt `if (!msg.addressed) return;` — reply only when addressed. This kills the "every agent answers everything" storm at the client edge, complementing server-side targeted delivery. Authors who want broader behavior can read every field and decide for themselves (the SDK exposes, it does not force).
- **Addressing a peer:** `ctx.sendMessage(content, { to?: string })` and the auto-send-on-return path gain an optional `to`, written into the envelope before signing (`agent.ts:227`). An agent can address a peer explicitly or @mention it and let the server resolve it.
- The connector **never sends `sender_type`** — it only reads it. It continues to assert only its own `from`.

### 3.5 Safety layer (inside `route_message`, only when target is an agent and `sender_type == "agent"`)

**Per-thread A2A hop counter** (`DashMap<thread_id, u32>` on `ServerState`, matching the existing dashmap pattern):

```
is_a2a = (env.sender_type == "agent") AND (target resolves to an agent)

if env.sender_type == "human":
    a2a_hops[thread_id] = 0                       # human posting resets the circuit

if is_a2a:
    hops = a2a_hops[thread_id]                     # consecutive agent→agent, no human between
    if hops >= A2A_HOP_CAP:                        # config AINM_A2A__HOP_CAP, default 6
        send_to_thread(thread_id, a2a_limit_notice)   # humans see "agents paused — reply to continue"
        return false                               # do NOT invoke target agent
    a2a_hops[thread_id] = hops + 1
```

- The cap counts **consecutive agent→agent messages with no human between**; any human message zeroes it. The human is the circuit-breaker.
- On trip: humans still receive the message for display, plus a one-line **system notice**; the target agent is simply not invoked. **No silent drop.**
- Cap is config with a sane floor/ceiling (default 6).

**Consent** (membership = consent + card opt-out), checked when `is_a2a`:

```
consent_ok(from_agent, to_agent, thread_id):
  1. to_agent has joined thread_id           (membership = consent; human curates membership)
  2. from_agent has joined thread_id         (only participants may address peers)
  3. to_agent's Agent Card acceptsA2A != false   (global opt-out honored)
  → all three required; else deliver-to-humans-only + system notice
```

- `acceptsA2A` is a **new optional Agent Card capability**, default **true** for connectors (a connected agent is presumed cooperative; an author sets it false to refuse peer traffic). Card plumbing exists end-to-end (`build_connector_card`, handshake `card` object, `published_card_for`).
- Ordering: hop-guard first (cheap, in-memory), then consent (may touch card cache). Both produce a **visible** outcome to humans, never a silent failure.

### 3.6 Hosted/connector parity

The in-process hosted assistant (`runtime_server::handle_turn`) becomes just another routing target resolved by §3.2; its bespoke trigger at `server.rs:2580` is removed. Hosted and connector agents are addressed, guarded, and consent-checked identically. (The hosted agent's *internal* tool-round cap and quota remain — they bound a single turn, orthogonal to the thread-level A2A hop cap.)

### 3.7 Federation (cross-server A2A)

When `to` resolves to a **foreign-host** agent, generalize the existing federation relay (which already carries agent→human across servers) to carry an agent `to`:

```
Server A (origin)                          Server B (target's home)
─────────────────                          ────────────────────────
route_message(env), to=foreign agent
  hop-guard + consent checked on A     ──▶ fed POST /ainm/federation (mTLS)
                                           verify env signature (resolve sender's
                                             pubkey via .well-known/ainm-identity, cached)
                                           hop-guard + consent RE-checked on B ──▶
                                           route_message(env) locally on B
                                             → agent_registry.send_to_agent(target)
                                           reply env (to = origin sender)
  deliver reply into thread on A       ◀── fed relay back to A
  (humans on A see it; SP3/SP4 render)
```

- **Guard + consent enforced on BOTH servers.** A checks before spending a federation call; B re-checks because B owns the target and must not trust A's accounting. Hop counter is **per-server-per-thread**; both increment; the stricter bound wins. A malicious/buggy peer cannot bypass the circuit-breaker.
- **Signature verification is the trust anchor.** B verifies the foreign sender's Ed25519 signature by resolving the sender's public key from its home server's `.well-known/ainm-identity` (shipped in AIB-14), cached. Unverifiable → dropped + notice, never delivered.
- **Thread identity across servers:** a federated thread shares its `thread_id`; each server tracks its own participant/membership view. B routes to the target only if the target has joined the thread on B and A's sender is a relayed known participant.
- **`acceptsA2A` honored at B** from the target's own card (B is authoritative for its agents).
- **Reply routing home:** reply envelope's `to` is the original foreign sender; B relays back over the same fed channel; A delivers into the thread — identical to today's federated agent→human reply with an agent endpoint.
- **Reuse, not rebuild:** `published_card_for` already resolves across client+fed surfaces (FCP); the fed listener already shares one `AgentRegistry`; the remote-origin relay exists. P3 is mostly *generalizing the relay to carry an agent `to`* and *adding the second guard/consent checkpoint on B*.

---

## 4. Components & File Map (indicative; finalized per-phase plan)

**Server (Rust, `server/src/`):**
- `protocol.rs` / envelope handling — document `to`, add `sender_type` stamping on the deliver wrapper.
- New pure module (e.g. `routing.rs`) — `resolve_recipient`, the A2A hop-counter state machine, and `consent_ok` as testable pure/near-pure functions.
- `server.rs` — `route_message` replacing the `to_agent_tx` broadcast in both handlers; activate `AgentRegistry::send_to_agent`; agent thread-membership set; fold hosted-assistant trigger into `resolve_recipient`; populate `signature` + new `to_addr` on insert.
- New migration — `messages.to_addr` column.
- `agent_card.rs` — `acceptsA2A` capability in `build_connector_card` / `build_assistant_card`.
- `main.rs` / federation — generalize the relay `to`; second guard/consent checkpoint; foreign-sender pubkey resolution + verify (P3).
- Config — `AINM_A2A__HOP_CAP`.

**Connector SDK (TypeScript, `packages/connector/src/`):**
- `protocol/types.ts` — `senderType`/`to`/`addressed`/`verified` on `IncomingMessage`; `to?` on the send options.
- `agent.ts` — populate new fields in `handleMessageDelivery`; thread `to` through `sendMessage` + auto-send.
- `packages/connector-openai/`, `packages/connector-claude-code/` — adopt the `addressed` gate; echo peer `to` when replying to an agent sender.

**iOS:** **no changes** — SP3/SP4 already render directional bylines and the digest capsule. Live A2A traffic flows into the shipped UI unchanged.

---

## 5. Phasing

Each phase is independently shippable and gets its own implementation plan.

**P1 — Same-server core** (makes SP1–SP4 real)
Envelope `to` + server-stamped `sender_type`; `messages.to_addr` + populate `signature`; `resolve_recipient` + `route_message` (targeted delivery, broadcast removed); activate `send_to_agent`; agent thread-membership; hop counter + cap + notice; SDK `senderType`/`to`/`addressed`/`verified` + `addressed`-gate in reference connectors; fold hosted-assistant trigger.

**P2 — Consent policy**
Membership-as-consent enforcement in `route_message`; `acceptsA2A` card capability (default true) + opt-out honored; system-notice on refusal.

**P3 — Federation**
Generalize fed relay to carry agent `to`; second guard+consent checkpoint on receiving server; foreign-sender signature verification via `.well-known/ainm-identity`; per-server hop accounting.

---

## 6. Testing Strategy

- **Pure units first (carry the correctness weight):** `resolve_recipient` (to / mention / none / unknown / non-participant); hop-counter state machine (increment, reset-on-human, cap-trip); `consent_ok` (membership × card-flag matrix).
- **Server integration (black-box WS, like `ws_conversation_e2e`):** two mock agents in one thread — assert A→B delivery; cap trips after N hops; human message resets; non-member / opted-out target drops to humans-only with a notice. Reuse `ConstantMockLlmClient` / `AINM_LLM__MOCK`.
- **SDK:** connector unit tests for `addressed` gating and `to` round-trip (sign → send → server resolves).
- **iOS:** no app changes; live-verify becomes a real two-agent thread producing genuine A2A traffic the existing UI folds (the SP4 capsule, SP3 bylines). A seeded scenario is replaced by actual runtime traffic.
- **Federation (P3):** shared-resolver units in-harness; the mTLS two-server cross-fetch remains the documented MANUAL human step (no in-harness mTLS client), consistent with FCP-5.
- **Out of scope:** the two known pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`) — noted, not fixed here.

---

## 7. Success Criteria

1. An agent can address another same-server agent by `to` or @mention; **only** the addressed agent is invoked; it replies; the loop is bounded by the hop cap and reset by humans. *(P1)*
2. A non-member or opted-out agent is **never** invoked; humans see a system notice. *(P2)*
3. A foreign-host agent can be addressed; **both** servers enforce guard + consent; signatures verified; reply returns to the thread. *(P3)*
4. The shipped SP1–SP4 UI renders live A2A with **zero client changes**.
5. No regression in existing suites; **no silent truncation or drop** anywhere on the routing path — every guard/consent rejection is visible to humans.

---

## 8. Risks & Open Questions

- **Federation is the highest-risk phase** (mTLS, cross-server trust, two-sided accounting). Last for that reason; only attempted after P1+P2 prove the same-server model.
- **Agent thread-membership** is currently implicit (via broadcast). Making it explicit is load-bearing for consent; the P1 plan must define exactly when an agent is considered "joined" (first addressed vs roster-added) and ensure cleanup on disconnect mirrors the existing `AgentRegistry` cleanup.
- **Storm bound across many agents:** the hop cap bounds *consecutive* A2A messages but a thread with many agents could still produce wide fan-out per hop; if observed in practice, a per-thread token budget (reusing the quota ledger) is the noted future lever — explicitly deferred.
- **`acceptsA2A` default:** chosen as `true` for connectors (cooperative-by-default). Revisit if abuse appears; the opt-out is the v1 control.
