# WebSocket Conversation E2E Test — Design (v1)

**Date:** 2026-06-03
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Context:** The iOS app ↔ hosted-runtime loop is wired (close-the-loop merged) and the server
boots headless (TTY fix merged), but the full **conversation over WebSocket** — handshake → agent
trigger → runtime loop → streamed delivery — has no automated coverage. B-10 tests the pure
`run_loop` in-process only. IOS-11 (the live conversation) is human-run. This adds a server-side
E2E test that validates the entire server half of IOS-11 automatically, so the human run only has to
confirm the iOS UI.

## 1. Goal & constraints

Drive a real WebSocket conversation against the **compiled `ainm-server` binary** and assert the
hosted agent's reply is delivered back. Validates: signed `ainm.initialize` handshake against a bound
account key (`verify_init`), the agent trigger on `params.to == assistant@<domain>`, `handle_turn` →
`run_loop`, and the signed `ainm.message.deliver` round-trip.

**Hard constraint (drives the whole approach):** `ServerState`, `server::start_*`, and
`runtime_server::handle_turn` are **binary-only** (the B-8 lib/bin split) — NOT in the `ainm_server`
lib crate that integration tests link against. So the test cannot construct the server in-process; it
must **black-box the compiled binary** (spawn it, drive it over a socket), the same pattern `make uat`
uses. Reachable from the test (lib-exposed): `ainm_server::{signing, identity, llm, ...}`.

**Determinism constraint:** the keyless binary gives the *agent* `llm = None`, and `handle_turn`
early-returns when `state.llm_client` is None — so a key-free run produces NO reply. The test needs a
deterministic agent LLM (see §3).

**Non-goals:** re-testing onboarding REST (signup/verify/bind — covered by AIB-13); testing real LLM
providers; the iOS UI (the human IOS-11 gate); multi-turn / tool-calling conversations (one text reply
is enough to prove the loop).

## 2. Architecture

Black-box subprocess test in `server/tests/ws_conversation_e2e.rs`, gated on a reachable Postgres
(skips otherwise, like the other integration suites). It:
1. inserts a human account + bound key directly via sqlx,
2. spawns `env!("CARGO_BIN_EXE_ainm-server")` headless with a test DB + the mock-LLM affordance,
3. polls `/health/ready`,
4. opens a WebSocket client (`tokio-tungstenite`, already a dep), does the signed handshake,
5. sends a signed message addressed to the agent,
6. asserts the agent's reply is delivered, then kills the child.

## 3. Server affordance: `AINM_LLM__MOCK`

A small change to `llm.rs` + `main.rs`:

- **`llm.rs`** — add `pub struct ConstantMockLlmClient { pub reply: String }` implementing `LlmClient`:
  `chat_with_tools` always returns `Ok(ChatResponse { turn: AssistantTurn::Text(self.reply.clone()),
  usage: Some(TokenUsage::default-ish) })` (no tool calls, never exhausts); `extract` returns an empty
  `ExtractionResult` (it is not exercised on the agent path). Unlike `MockLlmClient` (queue that errors
  when drained), this is constant — safe for any number of turns.
- **`main.rs`** — in the hosted-agent LLM selection (currently `match AINM_LLM__API_KEY { Ok→OpenAi,
  Err→None }`), add: if `AINM_LLM__MOCK` is truthy (`"1"`/`"true"`), set the agent llm to
  `ConstantMockLlmClient` with a fixed reply (e.g. `"[mock-llm] Hello from the AINM assistant. (no
  LLM key configured)"`), taking precedence over the key branch. Log a clear `[llm] AINM_LLM__MOCK
  set — agent using constant mock reply (dev/test only)` warning.
- **Bonus:** documented in the dogfood runbook — `AINM_LLM__MOCK=1 make dogfood` lets IOS-11 run
  key-free and still show a streamed canned reply.

The canned reply string is a single source of truth the test asserts a substring of.

## 4. Test flow (detailed)

1. **Skip guard:** `pool_or_skip()` (mirror the existing helper). Unique `domain = format!("e2e-{:x}.local", uuid)`.
2. **Account:** `identity::generate_keypair("alice@<domain>")`; INSERT `accounts` row
   (`alice@<domain>`, status `active`, email_verified true) + `account_keys` row with
   `URL_SAFE_NO_PAD(public_key_bytes)` (matches how the handshake decodes the stored key). Direct sqlx
   inserts (mirror `make_account` in the existing suites).
3. **Free port:** bind `TcpListener::bind("127.0.0.1:0")`, read the port, drop the listener.
4. **Spawn binary:** `Command::new(env!("CARGO_BIN_EXE_ainm-server"))` args
   `--mode human --port <P> --identity <domain> --cert <CARGO_MANIFEST_DIR>/certs/server-a.pem
   --key .../server-a-key.pem --ca-cert .../ca.pem`; env `AINM_DATABASE__URL=<test url>`,
   `AINM_LLM__MOCK=1`, `RUST_LOG=warn`; `kill_on_drop(true)`. (Absolute cert paths so cwd is irrelevant;
   `config/default.toml` is optional — figment skips a missing file.) The binary provisions
   `assistant@<domain>` on the same DB.
5. **Readiness:** poll `GET http://127.0.0.1:<P>/health/ready` (reqwest, a dep) until 200, ~15s timeout.
6. **WS connect:** `tokio_tungstenite::connect_async("ws://127.0.0.1:<P>/ws")`.
7. **Handshake:** `(sig, ts) = signing::sign_init(&alice.key_pair, "alice@<domain>")`; send
   `{"jsonrpc":"2.0","id":1,"method":"ainm.initialize","params":{"identity":"alice@<domain>",
   "timestamp":ts,"signature":sig}}` (mirror `repl.rs`'s exact frame); await the `initialized`
   response (id 1 result).
8. **Send:** build the envelope object `{id, from:"alice@<domain>", thread_id:<uuid>, timestamp,
   type:"message", content:[{"type":"text","text":"hello assistant"}], to:"assistant@<domain>"}`;
   sign with `signing::sign_envelope(&alice.key_pair, &envelope)` → set the `signature` field; send
   `{"jsonrpc":"2.0","method":"ainm.message.send","params":<signed envelope>}`. (The server verifies
   the inbound envelope via `verify_signature`, so it MUST be JCS-signed.)
9. **Assert reply:** read WS frames in a loop (with an overall ~15s timeout) until a notification with
   `method == "ainm.message.deliver"` whose inner `params.message.from == "assistant@<domain>"` arrives;
   assert its text content contains the canned mock substring. (Streaming `ainm.stream.*` deltas may
   precede it; tolerate/skip them — the terminal `deliver` is the assertion target.)
10. **Teardown:** drop/kill the child (`kill_on_drop`).

## 5. Module boundaries

- `server/src/llm.rs` — add `ConstantMockLlmClient` (+ a unit test asserting it returns the constant
  text and never errors across repeated calls).
- `server/src/main.rs` — `AINM_LLM__MOCK` wiring in the hosted-agent LLM selection.
- `server/tests/ws_conversation_e2e.rs` (new) — the black-box E2E test + a local `pool_or_skip` + the
  WS framing helpers (mirroring `repl.rs`).
- `docs/runbooks/2026-06-02-ios11-close-the-loop.md` — one note: `AINM_LLM__MOCK=1` for a key-free reply.
- No changes to `server.rs`, `runtime_server.rs`, or `runtime.rs` (the production paths are exercised
  as-is through the binary).

## 6. Error handling / robustness

- DB unreachable → skip (eprintln + return), matching the suite convention.
- Binary fails to become ready in 15s → fail the test with the captured child stderr tail.
- Each WS step has a timeout (no hang); the overall reply wait is bounded (~15s).
- Unique per-test domain avoids cross-run collisions in a shared DB; `kill_on_drop` prevents orphans.
- The test does not assert on streaming deltas (implementation detail); only the terminal signed
  `deliver` — robust to streaming-format changes.

## 7. Testing & DoD

1. `ConstantMockLlmClient` exists, implements `LlmClient`, returns the constant `Text` turn on every
   call (unit test, all platforms — no DB).
2. `AINM_LLM__MOCK` truthy → the binary's agent uses the constant mock (precedence over the API-key
   branch), logged.
3. `server/tests/ws_conversation_e2e.rs` drives the full WS round-trip and asserts the agent's reply
   delivers with the canned text — passing live against Postgres.
4. The test skips cleanly with no DB.
5. Full server suite green except the two known pre-existing failures (`test_graph_traversal`,
   `test_compilation_dedup_updates_confidence`); clippy clean on touched files.
6. The dogfood runbook documents `AINM_LLM__MOCK=1` for a key-free IOS-11 reply.
