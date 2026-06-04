# Agent Discovery / Add-Agent UX — Design (v1)

**Date:** 2026-06-04
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Context:** The iOS reference client can today talk to exactly one agent — the hosted
`assistant@<host>` — reachable only through a hardcoded `AppState.startAssistantThread()` + a single
"Chat with Assistant" button. Yet a home server can host many agents: the provisioned assistant **plus
any runtime that connects via the connector SDK** (Claude Code, OpenAI, etc.), each registered as
`name@host` with a published agent-card. The plumbing to reach them already exists on the client
(`AgentCardService` fetches an agent-card by URL or by address; `AgentCard` model; `ThreadRecord`
carries a target `agentAddress`; the send path stamps `to`). What's missing is the **user-facing path**
to discover, add, and converse with agents beyond the one hardcoded assistant. This sub-project adds
that path — the keystone of the "universal agent client: one app, any agent runtime, no lock-in" vision.

## 1. Goal & scope

A user can add an agent by **address (`name@host`) or agent-card URL**, see a verified preview of the
agent's card, save it as a per-account contact, and start/continue threads with it. The hosted
assistant becomes one entry in a saved-agents list rather than a hardcoded special case.

**Decisions (locked in brainstorming):**
- **Discovery = manual add** by address or card URL (no browsable server directory in v1).
- **Reach = home-server agents only** (`name@<your-host>`); delivery uses the proven local path.
  Remote/federated agents are a fast-follow.
- **Persistence = saved per-account "agent contacts"** (address + cached card) in the active account's
  local DB, shown in an Agents list; reusable across threads; removable.
- **Trust (v1) = "the agent's card resolves on your home server."** Message authenticity is already
  enforced server-side (canonical signed envelopes on delivery). Client-side cryptographic
  **key-pinning of agents is deferred** — no agent-key endpoint exists yet (agents are not rows in the
  accounts table that `.well-known/ainm-identity` serves), and home-server message signing is enforced
  on the wire regardless. This is a stated v1 limitation + fast-follow (see §10).

**Out of scope (v1):** remote/federated agents; a browsable server directory; client-side agent
key-pinning; editing agent metadata; agent search/recommendations.

## 2. Architecture & components

Mostly iOS, on existing primitives; one small server-side seam fix.

```
iOS (operates on the ACTIVE account's GRDB DB — multi-account already isolates DBs per account):
  Persistence/SavedAgentRecord.swift   (new)    GRDB row: a saved agent contact
  Persistence/SavedAgentStore.swift    (new)    CRUD over the active account's DB
  Persistence/AppDatabase.swift        (modify) migration adding the saved_agents table
  Networking/AgentCardService.swift    (modify) canonical card-URL path + scheme derived from serverURL
  App/AppState.swift                   (modify) resolveAgent / saveAgent / removeAgent / savedAgents;
                                                generalize startAssistantThread → startThread(withAgent:);
                                                seed hosted assistant as a default saved agent on bootstrap
  UI/Agents/AgentsListView.swift       (new)    saved-agents list + Add/Remove + tap-to-thread
  UI/Agents/AddAgentView.swift         (new)    enter address/URL → look up → preview card → confirm
  UI/ConversationList/ConversationListView.swift (modify) toolbar entry point to AgentsListView

Server:
  src/server.rs                        (modify, small) reconcile the agent-card route so the client
                                                resolves home-server agents (incl. the hosted assistant)
  tests/...                            (new)    regression test locking the agent-card route contract
```

## 3. Data model (per-account, GRDB)

**`SavedAgentRecord`** (new table `saved_agents` in the per-account DB):
- `address: String` — primary key, canonical `name@host`.
- `displayName: String` — from the card's `name` (for list display).
- `cardJSON: String` — serialized `AgentCard` snapshot, so the list renders offline without re-fetching.
- `addedAt: Date`.

Stored in the **active account's** database (multi-account isolation gives each account its own DB
file), so each account has its own agent list. Distinct from the existing ETag-keyed `AgentCardRecord`
HTTP cache: `SavedAgent` is the user's chosen contact; its card can be refreshed via `AgentCardService`.

A GRDB migration registers the table (additive; follows the existing migration pattern in
`AppDatabase.swift`).

## 4. Add → resolve → verify flow

**`AppState.resolveAgent(_ input: String) async -> Result<ResolvedAgent, AgentResolveError>`**
where `ResolvedAgent = (address: String, card: AgentCard)`:

1. **Parse** `input` as either:
   - a card URL (`http(s)://…/agent-card.json`), or
   - an address: `name@host`, or a bare `name` (→ default host to the active account's server host).
2. **Home-server guard:** if the parsed address's host ≠ the active account's server host →
   `AgentResolveError.remoteUnsupported` → "Only agents on your server (@<host>) are supported yet."
   (A pasted card URL is accepted only if its host matches the server host.)
3. **Fetch** the agent-card via `AgentCardService` (canonical path + scheme from `serverURL`, §5). A 200
   with a decodable card means the agent resolves on the home server. Non-200/timeout/undecodable →
   `AgentResolveError.unreachable` → "Couldn't reach that agent — make sure it's connected to your
   server and the address is correct."
4. Return `(canonicalAddress, card)` for the preview.

**`AppState.saveAgent(address:card:)`** → `SavedAgentStore.add(SavedAgentRecord(...))` → refresh
`savedAgents`. If the address is already saved, the caller jumps to the existing entry rather than
erroring (no duplicate row).

**`AppState.removeAgent(address:)`** → `SavedAgentStore.remove(address)` → refresh. Removing a saved
agent deletes only the contact; its existing threads remain in the DB.

`savedAgents: [SavedAgentRecord]` on AppState mirrors the store (refreshed on add/remove/bootstrap).

## 5. Server seam fix (agent-card contract)

Two concrete client/server mismatches block resolving home-server agents today; reconcile both:

1. **Path:** the server serves cards at `/.well-known/ainm/{agent_name}/agent-card.json`
   (`server.rs` `agent_card_handler`), but iOS `AgentCardService.fetchCard(forAddress:)` builds
   `/.well-known/ainm/**agents/**{local}/agent-card.json` (extra `agents/` segment). Fix: **align iOS to
   the server's existing route** (drop `agents/`). No server route change, no new endpoint.
2. **Scheme:** `fetchCard(forAddress:)` hardcodes `https://`, which fails against the `ws://localhost`
   dev server. Fix: derive the scheme from the active account's `serverURL` (`ws→http`, `wss→https`).

`agent_card_handler` already checks the live `AgentRegistry` (connector-connected agents publish their
cards there) and falls back to the static hosted-assistant card, so both the hosted assistant and
connector agents resolve at the canonical route once the client path/scheme match.

**Server regression test:** assert the canonical route serves the hosted agent's card (locks the
client/server contract so it can't silently drift again — mirrors the RC envelope-decode regression
tests). If a connector agent isn't connected, its card 404s; that's the expected "unreachable" case.

## 6. Thread start (generalize)

Replace the hardcoded assistant entry with an address-parameterized one:
- `AppState.startThread(withAgent address: String) async -> String?` creates a `ThreadRecord` with
  `agentAddress = address` (the existing send path stamps `to` from `agentAddress`).
- `startAssistantThread()` becomes a thin caller of `startThread(withAgent:)` with the hosted assistant
  address (kept for the existing entry point / tests, or removed if all call sites migrate — the plan
  will decide per call site).
- **Default seed:** on `bootstrap()`, if the active account has **no** saved agents, seed the hosted
  `assistant@host` (derived via the existing `assistantAddress(forServerURL:)`) as a default saved
  agent, so today's "Chat with Assistant" affordance maps onto the new list with no regression.

## 7. UI surfaces (iOS)

- **Entry point:** a toolbar button in `ConversationListView` (alongside the account-switcher
  `person.crop.circle`) opening **AgentsListView** as a sheet.
- **AgentsListView:** lists saved agents (display name, address, a short capability hint from the card);
  an **Add Agent** row → AddAgentView; tapping an agent starts/continues a thread with it
  (`startThread(withAgent:)`); swipe-to-remove with a confirmation.
- **AddAgentView:** a text field (address or card URL) + a **Look up** action that calls
  `resolveAgent`; on success shows the fetched card preview (name, description, key capabilities) and an
  **Add** button; on failure shows the inline error from §4.

Rough shape:
```
Agents                              [＋]
────────────────────────────────────────
 🤖  Assistant            assistant@host
     streaming · memory
 🔧  Claude Code         claude-code@host
     tools · streaming
────────────────────────────────────────
 ＋  Add an agent…
```

## 8. Error handling

All surfaced inline in AddAgentView: unreachable/offline agent (`unreachable`), non-home-server host
(`remoteUnsupported`), malformed address/URL (parse error), duplicate (jump to existing, no error).
Card-refresh failures in the list fall back to the cached `cardJSON` snapshot. Removing an agent never
touches its threads.

## 9. Testing

- **iOS unit:** address parse/normalize (`name`→`name@host`; `name@host`; host guard rejects a
  foreign host; URL-vs-address detection); `SavedAgentStore` CRUD; per-account isolation (an agent
  saved under account A is absent under account B); migration applies on a fresh DB.
- **iOS contract:** the card URL derived by `AgentCardService.fetchCard(forAddress:)` equals the
  canonical server route, and scheme maps correctly from `ws`/`wss`.
- **Server:** regression test — the canonical agent-card route returns the hosted agent's card with the
  expected shape.
- **Manual live (human):** run a connector agent (or use the hosted assistant), add it by address in
  the app, confirm the preview renders, start a thread, and receive a reply. (Consistent with prior
  live-validation phases; the only step the dev session can't self-verify.)

## 10. Definition of Done

1. A user can add an agent by `name@host`/bare-name/card-URL; a foreign host is rejected with a clear
   message; an unreachable agent shows a clear error.
2. Added agents persist per-account, appear in AgentsListView, and survive relaunch; removing one
   deletes only the contact.
3. Tapping a saved agent starts/continues a thread targeting its address, and messages deliver to it
   (verified live in §9).
4. The hosted assistant appears as a default saved agent (seeded) with no regression to the existing
   chat flow.
5. The client/server agent-card contract is reconciled (path + scheme) and locked by a server
   regression test; iOS unit + contract tests green; full iOS suite green; server suite green except
   the two known pre-existing failures.
6. v1 limitations documented: home-server-only, no client key-pinning, no directory.

## 11. Build order

Persistence (SavedAgentRecord + store + migration) → AgentCardService path/scheme fix + server route
regression test → AppState resolve/save/remove + startThread generalization + default seed → UI
(AgentsListView, AddAgentView, ConversationList entry) → tests → manual live. The card-contract fix is
load-bearing (nothing resolves without it) and should land early.
