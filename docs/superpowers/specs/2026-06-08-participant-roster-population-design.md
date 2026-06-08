# Live Participant-Roster Population — Design (v1)

**Date:** 2026-06-08
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)

## Context

SP3 (A2A & Multi-Participant Expression) shipped multi-agent name-tinting gated on
`MessageThreadViewModel.isMultiParticipant`, which derives from the `participant` table. The SP3
holistic review found that **nothing on the live path ever writes `participant` rows** — only
`ScenarioFixture` (tests/UAT) does. `AINMClient.handleMessageDelivery` (`Networking/AINMClient.swift:691`)
auto-creates the *thread* on inbound messages but never records participants. As a result
`isMultiParticipant` is always `false` in production and multi-agent tinting is dormant outside
fixtures.

This feature populates the `participant` table on the live path so the roster becomes **durable
first-class data** — lighting up SP3's multi-agent features and enabling future work (participant
list / roster UI, `@`-autocomplete from real members, roles). It is purely local DB state: **no
protocol, server, or network change** (consistent with how SP3 stayed display/data-only).

**Protocol reality (why classification is a heuristic, established during design):** there is no
authoritative sender-type on the live wire. `MessageEnvelope` carries only `from` (an address). The
`Participant` model has a `type: human|agent` field (`threading.schema.json`), but the iOS client
never decodes a participant list from the wire, and the server's `thread_participants` tracks only
identity strings — the one place the server emits per-participant `type` is a hardcoded demo payload
inside `ainm.context.frame`. Presence type is itself a heuristic (`handlePresence` infers
`isAgent: status == "processing"`). The only real client-side signals are: **self** (`== connectedIdentity`),
**`thread.agentAddress`**, **`savedAgents`** (card-resolved agents), and the content block **`source`**
(`"user"|"agent"`, advisory, per-message).

## Decisions (locked during brainstorming)

1. **Goal:** persist a durable participant roster (first-class data), not merely derive behavior in
   the view model.
2. **Type classification:** layer the real signals — `agent` if (`== thread.agentAddress`) ∨
   (in `savedAgents`) ∨ (content `source == "agent"`); `human` if self; else `human`.
3. **Backfill:** lazy on thread open, derived from the messages already loaded.

## 1. Goal & scope

Record a participant row for every identity that appears in a thread — at thread creation, on live
message delivery, and (for pre-existing threads) on first open — classified by the best available
signal, so `isMultiParticipant` and per-sender type are correct in production.

**In scope:**
- A pure `ParticipantClassifier`.
- An idempotent `ParticipantRepository.upsert` with earliest-joinedAt-wins + human→agent upgrade.
- Three write points: `startThread(withAgent:)` seed, `handleMessageDelivery` upsert, and
  `MessageThreadViewModel.loadInitial` lazy backfill.
- Unit + integration tests; full suites green.

**Out of scope:** any server/protocol/network change; a participant-list/roster UI surface (the data
this enables is for *future* UI); presence-driven type upgrades (the current presence type is itself a
heuristic and not needed for correctness); editing/removing participants; `thread.join`/`thread.leave`
handling beyond what message delivery already implies.

## 2. Architecture & components

The `participant` table already exists (composite PK `(threadId, identity)`), so **no schema
migration** is needed — every write is an idempotent upsert.

```
Persistence/
  ParticipantRepository.swift   (NEW) upsert(threadId, identity, type, role, joinedAt) on a DatabasePool;
                                       earliest-joinedAt-wins; role creator-sticky; type human→agent
                                       upgrade only (never downgrade). Plus a write-in-existing-`db`
                                       variant for callers already inside a transaction.
Models/  (or UI/Shared/)
  ParticipantClassifier.swift   (NEW, PURE) classify(address, isSelf, agentAddress,
                                       savedAgentAddresses, contentSource) -> String ("human"|"agent")
Networking/AINMClient.swift     (modify) handleMessageDelivery upserts envelope.from inside the
                                       existing write transaction (DB-queryable signals + envelope source)
App/AppState.swift              (modify) startThread(withAgent:) seeds user(human/creator) +
                                       agent(agent/member) in the same write as the ThreadRecord
UI/MessageThread/
  MessageThreadViewModel.swift  (modify) loadInitial backfills the roster from loaded messages when the
                                       thread has zero participant rows (authoritative in-memory savedAgents)
```

Each unit has one responsibility: the classifier decides type (pure, testable), the repository
persists idempotently, the three call sites supply the identities + signals they have. They cooperate
without coordination because the upsert merge is order-independent (Section 4).

## 3. The classifier (pure)

`ParticipantClassifier.classify(address:isSelf:agentAddress:savedAgentAddresses:contentSource:) -> String`:

```
if isSelf                                 -> "human"   // authoritative (== currentUserIdentity)
if address == agentAddress                -> "agent"   // thread's primary agent
if savedAgentAddresses.contains(address)  -> "agent"   // card-resolved agents
if contentSource == "agent"               -> "agent"   // per-message protocol hint
else                                      -> "human"   // safe default
```

- `isSelf` = `address == currentUserIdentity`.
- `agentAddress` is the thread's `agentAddress` (may be nil → that clause never matches).
- `contentSource` is the `source` of the first text block of the message being recorded; nil when
  unavailable (e.g. seeding at thread creation).
- Precedence: `isSelf` is checked first, so the current user is always `human` even if their address
  somehow matched another signal.

## 4. Idempotent upsert semantics

`ParticipantRepository.upsert(threadId, identity, type, role, joinedAt)` — idempotent on the
`(threadId, identity)` primary key:

- **Insert** when absent.
- **On conflict (existing row):**
  - `joinedAt` → keep the **earliest** of existing vs incoming.
  - `role` → remains `creator` if it was ever `creator` (creator-sticky); otherwise takes incoming.
  - `type` → **upgrade `human` → `agent`** when the incoming value is `agent`; **never downgrade**
    `agent` → `human`. So a confirmed agent stays an agent, and an early coarse `human` guess is
    corrected by a later authoritative agent signal.

**`role`:** `creator` when `identity == thread.createdBy`, else `member`.
**`joinedAt`:** delivery → `envelope.timestamp`; backfill → the participant's earliest message
timestamp; seed → `thread.createdAt`.

This merge is order-independent: thread-creation, delivery, and backfill can run in any order/any
number of times and the roster converges to the most-informed value.

## 5. Write points & data flow

**1. Thread creation** — `AppState.startThread(withAgent: address)`. After saving the `ThreadRecord`,
in the **same write**, seed two rows:
- `(connectedIdentity, type: "human", role: "creator", joinedAt: thread.createdAt)`
- `(address,           type: "agent", role: "member",  joinedAt: thread.createdAt)`

**2. Live delivery** — `AINMClient.handleMessageDelivery`, inside the existing write transaction
(right after `record.save(db)`), upsert `envelope.from`:
- `isSelf` = `envelope.from == currentUserIdentity` (the client's connected identity, threaded into
  `AINMClient`; nil-safe when not yet known).
- `agentAddress` = read from the thread row in-txn.
- `savedAgentAddresses` = `SELECT address FROM savedAgent` in-txn.
- `contentSource` = first text block's `source` from the decoded envelope.
- `role` = `creator` if `from == thread.createdBy` else `member`; `joinedAt` = `envelope.timestamp`.
- The participant upsert is **best-effort and must not drop the message**: isolate it so a participant
  write error cannot fail the message-save (message persistence remains the priority).

**3. Lazy backfill** — `MessageThreadViewModel.loadInitial`, after messages load: if the thread has
**zero** participant rows, derive the distinct identity set
`{ all message.fromAddress } ∪ { connectedIdentity } ∪ { thread.agentAddress (if non-nil) }`, classify
each with the **full in-memory `savedAgents`** (richest signal) + per-identity earliest message
`source`, and upsert all (earliest message timestamp as `joinedAt`). Then the existing `participants`
load + `isMultiParticipant` proceed unchanged.

**Data flow:**
```
startThread ─┐
delivery ────┼─→ ParticipantClassifier.classify ─→ ParticipantRepository.upsert ─→ participant table
loadInitial ─┘                                                                          │
                                          MessageThreadViewModel.participants ←──────────┘
                                          → isMultiParticipant / senderType → ParticipantByline (SP3)
```

## 6. Error handling / edge cases

- **Not connected (`connectedIdentity == nil`):** delivery skips the `isSelf` refinement (unknown →
  other signals decide); backfill still records every `fromAddress` + `agentAddress`. No crash.
- **Human thread (`agentAddress == nil`):** the agent-address clause never matches; senders classify
  via savedAgents/source/self. Correct.
- **Self-echo of the user's own message via delivery:** `isSelf` → `human`; role reconciled to
  `creator` if they created the thread; idempotent PK → no duplicate.
- **Type churn:** human→agent upgrade only; a confirmed agent never regresses.
- **Malformed/empty content:** `contentSource` nil → one fewer signal; classification still resolves.
- **Delivery-path failure isolation:** the participant upsert must not regress message delivery; a
  participant write error is swallowed (logged) without failing the message save.
- **`isMultiParticipant`:** counts distinct non-self identities — a single-agent thread stays `false`
  (calm), exactly as SP3 expects; a user + 2 agents → `true`.

## 7. Testing

- **Pure unit — `ParticipantClassifier`:** self→human; agentAddress→agent; savedAgent→agent;
  source=="agent"→agent; none→human; precedence (self beats an agent-looking address).
- **`ParticipantRepository` (in-memory DB):** insert; idempotent re-upsert (no dup, PK holds);
  earliest-joinedAt-wins; human→agent upgrade; agent **not** downgraded; creator role sticky.
- **Delivery integration:** deliver a message into a fresh thread → a participant row appears with the
  right type/role; a second distinct sender → two rows; `isMultiParticipant` becomes `true`.
- **Lazy backfill:** seed a thread with messages + **zero** participant rows (pre-feature data) →
  `loadInitial` populates the roster; second open is idempotent (no dupes, no churn).
- **Regression:** full iOS unit suite + UITests stay green; clean build. Existing
  `ScenarioFixture`-seeded participant tests must remain green (the upsert must not conflict with
  fixture-seeded rows).

## 8. Definition of Done

1. `ParticipantClassifier` (pure) implemented + unit-tested across the signal matrix and precedence.
2. `ParticipantRepository.upsert` implemented with earliest-joinedAt-wins, creator-sticky role, and
   human→agent-only type upgrade; unit-tested (incl. no-downgrade).
3. `startThread(withAgent:)` seeds user + agent in the same write as the thread.
4. `handleMessageDelivery` upserts the sender in-txn using DB-queryable signals + envelope source,
   without ever regressing message persistence.
5. `MessageThreadViewModel.loadInitial` lazily backfills the roster for threads with no participant
   rows; idempotent on re-open.
6. `isMultiParticipant` is demonstrably `true` for a real user + 2-agent thread and `false` for a 1:1
   agent thread (integration test).
7. Full iOS unit suite + UITests green; clean build; committed to `main` with `rtk proxy git`.

## 9. Build order

1. `ParticipantClassifier` (pure) + unit tests — the decision core.
2. `ParticipantRepository.upsert` (+ in-txn variant) + repository tests — the persistence core.
3. Wire `startThread(withAgent:)` seed (+ test).
4. Wire `handleMessageDelivery` upsert (+ delivery integration test); ensure failure isolation.
5. Wire `MessageThreadViewModel.loadInitial` lazy backfill (+ backfill test); verify
   `isMultiParticipant` flips on a multi-agent thread.
6. Full unit + UITest suites green; DoD audit; commit.

The decision core lands first (everything calls it), then persistence, then the three write points in
increasing integration scope, then verification.
