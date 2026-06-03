# Close the Loop: App ↔ Hosted Runtime — Design (v1, minimal)

**Date:** 2026-06-02
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Context:** The iOS app (~12k LOC) and the hosted server runtime (accounts A, hosted agent B,
credentials/quota C, sandboxing D, auth-sessions) have **never run together live** — IOS-11 is
still pending. A gap analysis showed the loop is ~95% wired; the missing 5% is glue. This closes
it with the **smallest path to one real, felt conversation** with the built-in `assistant@<domain>`
agent, deferring agent-discovery UX and deployment (E).

## 1. Goal

A human, on the iOS simulator, completes onboarding and has a real streamed conversation with the
hosted `assistant@<domain>` agent — exercising the binding handshake, the agent loop, memory
briefing, the LLM call, and streaming delivery — against a documented local DB-backed server.

**Non-goals (deferred):** agent-discovery picker / multi-agent UX; the account session-token client
wiring (REST account-management, not needed for the WS conversation loop); production deployment (E);
physical-device networking (simulator-only for v1).

## 2. The gap (from analysis, verified)

Working already (no change): the app handles `ainm.stream.*` deltas + `ainm.message.deliver` and
renders streaming replies; onboarding signs the handshake with the bound key; the server enforces
the binding (`verify_init`, DB-gated) and triggers the agent when a message targets it; human-mode
with `AINM_DATABASE__URL` auto-provisions `assistant@<server_domain>` and wires a real LLM when
`AINM_LLM__API_KEY` is set (`main.rs:511-525`).

Missing:
1. The app never **addresses a message to the agent** — the server trigger
   (`server.rs:2455`) fires on `thread_id.contains("assistant@")` OR `params.to == agent.address`
   OR content mentioning the address; the app sets none.
2. No **UI entry point** to start an assistant conversation.
3. A documented **run config** so the simulator can reach a DB-backed human-mode server.
4. The **live run** itself (IOS-11).

## 3. Design

### 3.1 App — `to` field on the envelope
Add optional `to: String?` to `MessageEnvelope` (Swift Codable) and include it in the outbound
`ainm.message.send` JSON-RPC params **only when set** (so existing human↔human sends are unchanged).
The server already reads `params.to` and compares it to the agent address — no server change needed
for addressing.

### 3.2 App — agent address on the thread
Persist the target agent address on the thread so every message in that thread carries `to`. Add an
optional `agentAddress: String?` to `ThreadRecord` (GRDB) + its in-memory thread model. `sendMessage`
sets `to: <thread.agentAddress>` when present. A thread with no `agentAddress` behaves exactly as today.

### 3.3 App — "Chat with Assistant" entry point
A button in the conversation-list toolbar (e.g. a "robot"/sparkle action) that:
- derives the agent address as `assistant@<host>` where `<host>` is the host component of
  `AppState.serverURL` (so it matches the server's `server_domain`);
- creates (or reuses) a thread with `agentAddress = assistant@<host>` and a friendly title
  ("Assistant"), then navigates into it.
No discovery, no picker — the one built-in agent. (Future: replace with a discovery list → F.)

### 3.4 App — local connection compatibility (verify + fix if needed)
Two connect-time details the implementation must verify against the real app code and fix only if
they block a plain-`ws`, localhost dogfood:
- **Scheme:** the local human-mode server serves plain `ws` on `:8080` (TLS lives on the separate
  federation port). If the WS client forces `wss://`, allow `ws://` when the configured `serverURL`
  uses it. If it already honors the URL scheme, no change.
- **REST base URL:** `AccountService` (signup/verify/bind-key) must target the SAME host/port as the
  WS connection (`http://<host>:8080`), derived from `serverURL`. Verify; fix only if it diverges.
These are "verify, fix iff broken" — not speculative rework.

### 3.5 Server — dogfood run helper
No server logic changes. Add a documented one-command launch (a `Makefile` target `dogfood` and/or a
`scripts/dogfood.sh`) that runs the existing binary in human mode with the right env/flags:
- `AINM_DATABASE__URL=postgres://…/ainm` (binding enforced + agent provisioned),
- `--identity localhost` (so `server_domain = localhost` and handshake identity `username@localhost`
  matches the simulator's `serverURL` host),
- `--mode human --port 8080`,
- `AINM_LLM__API_KEY` passed through from the operator's env (real replies; absent → the agent still
  runs but the LLM call no-ops/errors — documented),
- no Resend key → MockEmailSender logs the OTP code.
Postgres comes from the existing `server/docker-compose.yml` (or a local PG). The migration runner
(`db.rs`) applies 001–006 on connect.

### 3.6 Live validation — IOS-11 runbook (human-run)
A step-by-step `docs/` runbook: start Postgres → `make dogfood` → point the app's `serverURL` at
`ws://localhost:8080` → signup → read OTP from server logs → verify → bind key → tap "Chat with
Assistant" → send "What's 2+2?" → observe the streamed reply render. Record success + any errors.
This is the gate that proves the loop; it is run by the user, not automated.

## 4. Module boundaries / files

- **App:** `Models/MessageEnvelope.swift` (`to` field), `Persistence/ThreadRecord.swift` +
  `ThreadRepository` (`agentAddress` column/migration), the thread model + `AppState.createThread`/
  `sendMessage` (set `to`), `UI/ConversationList/ConversationListView.swift` (the button), and a
  host-derivation helper (`assistant@<host>` from `serverURL`). Verify `AINMClient` URL scheme +
  `AccountService` base URL (§3.4).
- **Server:** none beyond the run helper (`Makefile`/`scripts/dogfood.sh`) + the runbook doc.

## 5. Testing

- **App unit tests:** `MessageEnvelope` round-trips `to` (present and absent → omitted, not null, if
  the wire format requires omission); the `assistant@<host>` derivation from representative
  `serverURL`s (`ws://localhost:8080`, `wss://host:443`); `sendMessage` includes `to` iff the thread
  has an `agentAddress`. Follow the app's existing XCTest patterns.
- **Server:** no new logic → no new server tests (the agent-trigger path is covered by B's tests).
- **Manual gate:** the IOS-11 runbook (§3.6) — the real proof.

## 6. Definition of done

1. `MessageEnvelope` carries an optional `to`, serialized into `ainm.message.send` only when set;
   human↔human sends are byte-unchanged when `to` is absent.
2. Threads can carry an `agentAddress`; messages in such a thread set `to` to it.
3. A "Chat with Assistant" entry point opens an agent thread addressed to `assistant@<serverURL-host>`.
4. The app connects to a plain-`ws` localhost server and runs onboarding REST against the same host
   (verified; fixed only if it was blocking).
5. `make dogfood` (or `scripts/dogfood.sh`) launches a DB-backed human-mode server the simulator can
   reach, with the OTP readable in logs; a runbook documents the full IOS-11 sequence.
6. App unit tests pass (Xcode build green); the existing server suite is unaffected.
7. The human-run IOS-11 live conversation is documented as the closing validation (run by the user).
