# Runtime A2A — P3a (Federation Integration) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the inbound federation path route through the same `deliver_message` orchestrator the WS handlers use — so a relayed message gets the P2 consent gate, the A2A hop guard, and **real delivery to a connected connector agent on the receiving server** (today it only re-injects into a broadcast consumed by a demo echo loop). Plus: the first automated two-server integration test.

**Architecture (keystone decision — DECIDED):** Refactor `deliver_message(&ServerState, ...)` to take an explicit dependency struct `DeliverDeps` (the specific pieces it needs), so BOTH the WS handlers and `relay_handler` can call the SAME orchestrator without `relay_handler` needing a whole `ServerState`. The shared registries/counters (`AgentRegistry`, `HumanRegistry`, the `a2a_hops` map + `a2a_hop_cap`, the hosted-agent handle) are constructed ONCE in `run_federation` and injected into both the WS listener (via `start_federation`, mirroring how `agent_registry` is already injected at `main.rs:606/626`) and `FederationState` — so the relay listener and the WS listener operate on the same live state.

**Tech Stack:** Rust (Axum/Tokio) server at `server/src`; cargo + nextest; mTLS federation (`tls.rs`, `relay.rs`, certs fixtures at `server/certs/`). New: two-process black-box federation test.

**Standing constraints:**
- All git via `rtk proxy git` (NEVER bare `git`). Commit directly to `main` (no branches).
- Pure-logic-first where possible. No iOS changes.
- Never call the product a "demo" in product-facing text (the `run_federation` echo loop is a reference harness — fine to describe internally).

**Scope (P3a only):** Make the seam real for the EXISTING federated flow (relayed message → `deliver_message` → consent/hop/agent delivery on the receiving server) + the two-server test. **NOT in P3a (→ P3b):** generalizing the relay to carry an agent `to` for *outbound* cross-server agent→agent addressing, the second-server hop/consent *re-accounting* semantics beyond what `deliver_message` already does, and reply-routing-home for agent-initiated peer chains. P3a delivers the *integration substrate*; P3b delivers cross-server A2A *semantics* (success criterion 3).

---

## Background: verified current state (cite before editing)

- `deliver_message(state: &ServerState, params, sender_type, verified)` — `server.rs` (~line 1473). Reads `state.human_registry`, `state.agent_registry`, `state.a2a_hops`, `state.a2a_hop_cap`, `state.hosted_agent`, `state.to_agent_tx`, and `state.shutdown.spawn_turn` (hosted path). Called by both WS handlers.
- `FederationState` — `federation.rs:150`. Has `server_id`, `server_keypair`, `server_identity`, `thread_registry`, `identity_store`, `remote_cache`, `relay_sender`, `local_delivery_tx`, `persistence`, `http_client`. **No `AgentRegistry`, no `HumanRegistry`, no hop state, no hosted-agent.**
- `relay_handler` — `federation.rs:185`. After replay + server-sig + per-message-sig checks (step 3, which ALREADY lazily resolves a foreign sender's pubkey via `/.well-known/ainm/participant/{sender}` and registers it with a `type_` of "human"/"agent" at `federation.rs:425-426`), step 4 (`federation.rs:454-515`) registers the thread and does `local_delivery_tx.send(ainm.message.deliver{message,verified,relayed_from})` then fans out if origin. This is the insertion point.
- `run_federation` construction — `main.rs:580-630`. `agent_registry = server::AgentRegistry::new()` (`main.rs:606`) is injected into `start_federation` (`main.rs:626`). `fed_state` is built at `main.rs:582-593`. The relay route binds `fed_state` (`main.rs:698`).
- `start_federation` signature — `server.rs` (search `pub async fn start_federation`); it currently takes `agent_registry: AgentRegistry` and `a2a_hop_cap: u32` (added in P1/FCP). It internally builds `ServerState` (with `human_registry`, `a2a_hops`, etc.). `start()` / `start_with_port()` are sibling constructors.
- `identity_store.get_public_key_bytes(addr)` works for any registered address; relayed foreign senders are registered (with their `type_`) by step 3.
- mTLS test fixtures: `server/certs/{ca,server-a,server-b,server-a-key,server-b-key}.pem`. No automated two-server test exists (`server/tests/` has none).

---

## File Structure

- `server/src/server.rs` — define `DeliverDeps<'a>` (or owned `DeliverDeps`); refactor `deliver_message` to take it; add a thin `ServerState::deliver_deps(&self) -> DeliverDeps` helper; update both WS call sites; thread shared `HumanRegistry`/`a2a_hops`/`a2a_hop_cap`/`hosted_agent` into `start_federation`/`start`/`start_with_port` as needed so they can be injected.
- `server/src/federation.rs` — add `agent_registry`, `human_registry`, `a2a_hops`, `a2a_hop_cap`, `hosted_agent` to `FederationState`; in `relay_handler` step 4, build a `DeliverDeps` and call `deliver_message`, deriving `sender_type` from the registered identity `type_`.
- `server/src/main.rs` — construct the shared `HumanRegistry`/`a2a_hops` in `run_federation`, inject into both `start_federation` and `fed_state` (mirror the `agent_registry` precedent).
- `server/tests/federation_a2a_e2e.rs` — NEW. Two `run_federation` processes over mTLS (certs fixtures); assert a message from server A is delivered on server B through `deliver_message` (to a connected agent / via the consent+hop path). DB-or-skip.

---

## Task 1: Introduce `DeliverDeps` and refactor `deliver_message` to use it

**Goal:** Decouple `deliver_message` from `&ServerState` with ZERO behavior change. This is a pure refactor; the existing `ws_conversation_e2e`/`a2a_*` tests (DB-gated) must still pass.

**Files:** `server/src/server.rs`.

- [ ] **Step 1: Define `DeliverDeps`.** Near `deliver_message`, add a struct carrying borrowed references to exactly what the function uses. Read the current `deliver_message` body first and enumerate every `state.X` it touches; the struct fields must match. Expected set (verify against the body):

```rust
/// The exact dependencies `deliver_message` needs, decoupled from `ServerState` so both the
/// WS handlers and the federation relay handler can drive the same delivery orchestrator.
pub struct DeliverDeps<'a> {
    pub human_registry: &'a HumanRegistry,
    pub agent_registry: &'a AgentRegistry,
    pub a2a_hops: &'a Arc<Mutex<HashMap<String, u32>>>,
    pub a2a_hop_cap: u32,
    pub hosted_agent: Option<&'a HostedAgent>,
    pub to_agent_tx: &'a broadcast::Sender<String>,
    /// Only needed for the hosted-assistant turn spawn. The hosted path uses
    /// `state.shutdown.spawn_turn(...)` + `runtime_server::handle_turn(&ServerState, ...)`,
    /// which REQUIRES a full `ServerState`. See Step 3 for how this is handled.
    pub state_for_hosted_turn: Option<&'a ServerState>,
}
```

> **Important nuance (read carefully):** the hosted-assistant branch of `deliver_message` calls `state.shutdown.spawn_turn(async move { runtime_server::handle_turn(&state2, ...) })`, which needs a full owned `ServerState` (`state.clone()`). `relay_handler` does NOT have a `ServerState`. Resolution for P3a: the hosted-turn spawn stays gated on `state_for_hosted_turn` being `Some` — the WS handlers pass `Some(state)`, the relay handler passes `None`. When `None`, a hosted-assistant target over federation is NOT spawned (logged + dropped-to-humans). This is acceptable for P3a because the hosted-assistant-as-federated-target is itself part of the deferred parity work; connector-agent delivery (the P3a goal) does NOT need `state_for_hosted_turn`. Document this in a code comment.

- [ ] **Step 2: Refactor `deliver_message` signature + body.** Change `async fn deliver_message(state: &ServerState, params, sender_type, verified)` to `async fn deliver_message(deps: &DeliverDeps<'_>, params: &serde_json::Value, sender_type: &str, verified: bool)`. Replace every `state.human_registry` → `deps.human_registry`, `state.agent_registry` → `deps.agent_registry`, `state.a2a_hops` → `deps.a2a_hops`, `state.a2a_hop_cap` → `deps.a2a_hop_cap`, `state.to_agent_tx` → `deps.to_agent_tx`. For the hosted block: gate on `if let Some(state) = deps.state_for_hosted_turn { ... existing spawn_turn ... }` and the `hosted_addr` computation uses `deps.hosted_agent`.

- [ ] **Step 3: Add a `ServerState::deliver_deps` helper** so WS call sites stay terse:

```rust
impl ServerState {
    fn deliver_deps(&self) -> DeliverDeps<'_> {
        DeliverDeps {
            human_registry: &self.human_registry,
            agent_registry: &self.agent_registry,
            a2a_hops: &self.a2a_hops,
            a2a_hop_cap: self.a2a_hop_cap,
            hosted_agent: self.hosted_agent.as_deref(),
            to_agent_tx: &self.to_agent_tx,
            state_for_hosted_turn: Some(self),
        }
    }
}
```

> Verify `self.hosted_agent` is `Option<Arc<HostedAgent>>` so `.as_deref()` yields `Option<&HostedAgent>` (matches the existing `state.hosted_agent.as_ref().filter(|a| a.enabled)` usage). Adjust the field type in `DeliverDeps` if the real type differs.

- [ ] **Step 4: Update both WS call sites.** Every `deliver_message(&state, params, "agent"|"human", verified).await` → `deliver_message(&state.deliver_deps(), params, "agent"|"human", verified).await`. (Search `deliver_message(&state` in `server.rs`.)

- [ ] **Step 5: Build + regression.**
Run: `cargo build -p ainm-server` → clean.
Run: `cargo test -p ainm-server routing` → 28 pass.
Run: `cargo test -p ainm-server --test ws_conversation_e2e --test a2a_routing_e2e --test a2a_consent_e2e --test a2a_spoof_e2e` → all pass or skip-no-db.

- [ ] **Step 6: Commit.**
```
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "refactor(a2a-p3a): deliver_message takes explicit DeliverDeps (no behavior change)"
```

---

## Task 2: Inject shared registries/hop-state so FederationState can build DeliverDeps

**Goal:** Make `run_federation` construct ONE `HumanRegistry` + ONE `a2a_hops` map and share them with both the WS listener (`start_federation` → its `ServerState`) and `FederationState`, mirroring the existing `agent_registry` sharing (`main.rs:606/626`).

**Files:** `server/src/server.rs` (`start_federation`, `start`, `start_with_port`, `ServerState` construction), `server/src/main.rs`, `server/src/federation.rs`.

- [ ] **Step 1: Add fields to `FederationState`** (`federation.rs:150`):
```rust
    /// Shared with the WS listener's ServerState so relayed messages reach connected agents
    /// and apply the same consent + hop guard (runtime A2A P3a).
    pub agent_registry: crate::server::AgentRegistry,
    pub human_registry: crate::server::HumanRegistry,
    pub a2a_hops: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, u32>>>,
    pub a2a_hop_cap: u32,
    pub hosted_agent: Option<std::sync::Arc<crate::server::HostedAgent>>,
```
(Match the real types of `AgentRegistry`/`HumanRegistry`/`HostedAgent` — they're `Clone`, Arc-backed.)

- [ ] **Step 2: Thread the shared instances through `start_federation`.** Today `start_federation` takes `agent_registry: AgentRegistry` and builds `human_registry`/`a2a_hops` internally inside its `ServerState`. Add parameters `human_registry: HumanRegistry` and `a2a_hops: Arc<Mutex<HashMap<String,u32>>>` (and confirm `a2a_hop_cap` + `hosted_agent` are already params — they are). Use the injected instances when constructing `ServerState` instead of creating new ones. For `start()` / `start_with_port()` (non-federation constructors) keep creating their own internally (no change), OR give them defaulted internal creation — do the minimal change so only `start_federation`'s `ServerState` uses injected instances.

> This mirrors exactly how `agent_registry` was injected (P1 Task 5 / FCP-2). Follow that precedent: find where `ServerState { agent_registry, ... }` is built inside `start_federation`'s path and replace the internally-created `human_registry`/`a2a_hops` with the injected ones.

- [ ] **Step 2b:** If `start()`/`start_with_port()` share a common `ServerState`-builder with `start_federation`, add the two new params there with internal-default fallback so the non-fed constructors are unaffected. The compiler will guide which constructors need updating.

- [ ] **Step 3: Construct + inject in `run_federation`** (`main.rs:580-630`). Before building `fed_state`, add:
```rust
    let human_registry = server::HumanRegistry::new();
    let a2a_hops: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, u32>>> =
        std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));
```
Pass `human_registry.clone()` and `a2a_hops.clone()` into `start_federation` (new args). Add the five new fields to the `fed_state` literal (`main.rs:582-593`): `agent_registry: agent_registry.clone()`, `human_registry: human_registry.clone()`, `a2a_hops: a2a_hops.clone()`, `a2a_hop_cap: app_config.a2a.hop_cap`, `hosted_agent: hosted_agent_for_fed.clone()`.

> `agent_registry` (`main.rs:606`) and `hosted_agent_for_fed` (`main.rs:608`) already exist in scope. `app_config.a2a.hop_cap` is already read (`main.rs:628`).

- [ ] **Step 4: Build.**
Run: `cargo build -p ainm-server` → clean once all constructor sites have the new params/fields. (No behavior change yet — `FederationState`'s new fields are unused until Task 3; they will warn — acceptable, do NOT `#[allow]`.)

- [ ] **Step 5: Commit.**
```
rtk proxy git add server/src/server.rs server/src/main.rs server/src/federation.rs
rtk proxy git commit -m "feat(a2a-p3a): share HumanRegistry/AgentRegistry/hop-state into FederationState"
```

---

## Task 3: Route inbound relays through `deliver_message`

**Goal:** In `relay_handler` step 4, deliver each relayed message via `deliver_message` (gaining consent + hop guard + real agent delivery), deriving `sender_type` from the registered sender identity. Keep `local_delivery_tx.send` (still consumed by the reference echo loop / backfill) and the origin fan-out unchanged.

**Files:** `server/src/federation.rs`.

- [ ] **Step 1: Build `DeliverDeps` from `FederationState` and call `deliver_message`.** In the per-message loop at `federation.rs:454-515`, AFTER the existing `local_delivery_tx.send(...)` (keep it) and BEFORE/around the origin fan-out, add:

```rust
        // Runtime A2A P3a: route the relayed message through the same delivery orchestrator the
        // WS handlers use, so consent + hop guard apply on THIS (receiving) server and a connected
        // connector agent actually receives it. sender_type is derived from the registered identity
        // (resolved/verified in step 3 above); default "human".
        let sender_type = {
            let sender = msg.get("from").and_then(|f| f.as_str()).unwrap_or("");
            match state.identity_store.participant_type(sender) {
                Some(t) if t == "agent" => "agent",
                _ => "human",
            }
        };
        let deps = crate::server::DeliverDeps {
            human_registry: &state.human_registry,
            agent_registry: &state.agent_registry,
            a2a_hops: &state.a2a_hops,
            a2a_hop_cap: state.a2a_hop_cap,
            hosted_agent: state.hosted_agent.as_deref(),
            to_agent_tx: None_equivalent, // see note
            state_for_hosted_turn: None, // relay has no ServerState; hosted-target-over-fed deferred to P3b
        };
        crate::server::deliver_message(&deps, msg, sender_type, true).await;
```

> **Two things to resolve while implementing:**
> 1. `to_agent_tx`: `DeliverDeps.to_agent_tx` is used only for the human-no-target legacy fallback. `FederationState` has `local_delivery_tx` (a `broadcast::Sender<String>`) which is the federation-mode equivalent broadcast. Use `&state.local_delivery_tx` for `to_agent_tx` so the fallback (human sender, unresolved target) still reaches the local broadcast. Confirm both are `broadcast::Sender<String>` (they are).
> 2. `identity_store.participant_type(sender)`: verify this accessor exists on `IdentityStore`. From the explore, `IdentityStore` stores an `IdentityRecord` with a `type_`/`participant_type` field (set via `register(sender, pk, type_, domain, None)` at `federation.rs:426`). If there is no public accessor returning the type, add a small one to `identity.rs` (e.g. `pub fn participant_type(&self, address: &str) -> Option<String>`), mirroring `get_public_key_bytes`. This is a tiny additive read accessor.

- [ ] **Step 2: Build.**
Run: `cargo build -p ainm-server` → clean. The `FederationState` fields from Task 2 are now used (warnings clear).

- [ ] **Step 3: Targeted regression.**
Run: `cargo test -p ainm-server routing` → 28 pass.
Run the existing DB-gated suites (skip-no-db here): `cargo test -p ainm-server --test ws_conversation_e2e --test a2a_routing_e2e --test a2a_consent_e2e` → pass or skip.

- [ ] **Step 4: Commit.**
```
rtk proxy git add server/src/federation.rs server/src/identity.rs
rtk proxy git commit -m "feat(a2a-p3a): inbound relays route through deliver_message (consent+hop+agent delivery)"
```

---

## Task 4: Two-server automated federation test

**Goal:** The first automated cross-server test. Stand up two `ainm-server` processes in `--mode` federation over mTLS using the `server/certs/` fixtures; assert a message originated on server A is delivered on server B *through `deliver_message`* — specifically that a connected agent on B receives it. DB-or-skip.

**Files:** `server/tests/federation_a2a_e2e.rs` (NEW).

> This is the largest new piece. Model the process-spawn + mTLS + WS-client mechanics on `a2a_routing_e2e.rs` (for the WS/handshake/`next_text` helpers) and on the MANUAL federation runbook / `main.rs` `run_federation` CLI flags (`--mode`, `--cert`, `--key`, `--ca-cert`, `--identity`, `--federation-port`, `--remote`, `--port`). Read `main.rs:262-300` for the exact required flags and `server/certs/generate.sh` for which cert belongs to which server identity.

- [ ] **Step 1: Write the test.** Structure:
  1. DB-or-skip guard (`pool_and_url_or_skip`, copied from `a2a_routing_e2e.rs`).
  2. Pick two server identities matching the certs (e.g. `server-a.local` / `server-b.local` per `certs/generate.sh`). Two free ports each (WS `--port` + `--federation-port`).
  3. Spawn server A (`--mode human`) and server B (`--mode agent`) with their respective `--cert/--key/--ca-cert/--identity/--federation-port/--remote <peer>` and `AINM_DATABASE__URL` + `AINM_A2A__HOP_CAP`. Wait for both `/health/ready`.
  4. Connect a WS **agent** to server B (participant_type=agent, bound account on B's domain) — this registers it in B's shared `AgentRegistry`.
  5. Connect a WS **human** to server A; create/seed a federated thread (thread_id `server-a.local:<uuid>`); send a message addressed (`to`) to the agent on B.
  6. Assert the agent on B receives an `ainm.message.deliver` for that message within a timeout — proving the relay → `deliver_message` → `send_to_agent` path works cross-server.
  7. Kill both; cleanup both domains.

> **Honest caveat to encode in the test + runbook:** standing up mTLS between two spawned processes in-harness is the hard part (the existing suite has NO such test). If the two-process mTLS handshake proves infeasible to automate reliably in CI, fall back to: (a) an in-process test that constructs a `FederationState` with shared registries + a connected mock agent and calls `relay_handler` directly with a signed relay payload, asserting the agent receives delivery (this exercises the P3a integration — relay→deliver_message→agent — WITHOUT the mTLS transport, which is already covered manually). Prefer the full two-process test; use the in-process `relay_handler` test as the guaranteed-automatable floor. Implement at least the in-process test; attempt the two-process one.

- [ ] **Step 2: Compile + run.**
Run: `cargo test -p ainm-server --test federation_a2a_e2e --no-run` → compiles.
Run: `cargo test -p ainm-server --test federation_a2a_e2e` → passes vs DB+certs, or prints skip. Report which path (two-process vs in-process floor) is asserting.

- [ ] **Step 3: Commit.**
```
rtk proxy git add server/tests/federation_a2a_e2e.rs
rtk proxy git commit -m "test(a2a-p3a): automated federation integration test (relay→deliver_message→agent)"
```

---

## Task 5: Runbook + full-suite verification + clippy + DoD

**Files:** `docs/runbooks/2026-06-09-federation-a2a-verify.md` (NEW), verification only otherwise.

- [ ] **Step 1: Write the manual two-server runbook** documenting how to stand up server A + server B with the certs fixtures and observe a cross-server delivery reaching a connected connector agent (the live proof, since CI may only run the in-process floor). Mirror the FCP-5 manual runbook style.

- [ ] **Step 2: Server build, clippy, tests.**
Run: `cargo build -p ainm-server` → clean.
Run: `cargo clippy -p ainm-server --all-targets 2>&1 | grep -E "server\.rs|federation\.rs|identity\.rs|federation_a2a_e2e"` — confirm NO new lints in P3a-touched files (pre-existing lints in other files are out of scope).
Run: `cargo test -p ainm-server` → PASS except the two known pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`).

- [ ] **Step 3: DoD audit.**
- [ ] `deliver_message` driven by `DeliverDeps`; both WS handlers + `relay_handler` call it (Tasks 1, 3).
- [ ] Shared `AgentRegistry`/`HumanRegistry`/`a2a_hops`/hop-cap/hosted-agent across the WS + relay listeners (Task 2).
- [ ] Inbound relay applies consent + hop guard and can deliver to a connected agent (Task 3).
- [ ] Foreign-sender `sender_type` derived from the (already-resolved) registered identity (Task 3).
- [ ] At least the in-process `relay_handler→deliver_message→agent` test automated; two-process mTLS test attempted; manual runbook written (Tasks 4, 5).
- [ ] No iOS changes; no new clippy lints in touched files; suite green except the 2 pre-existing.
- [ ] P3b scope (outbound agent `to` relay, reply-routing-home, second-server re-accounting, hosted-target-over-fed) explicitly deferred — confirm none was silently added.

- [ ] **Step 4: Commit (if fixes made).**
```
rtk proxy git add -A
rtk proxy git commit -m "docs+chore(a2a-p3a): federation runbook + full-suite green + DoD"
```

---

## Notes & Risks

- **Highest-risk subsystem, thin test net.** Until Task 4's in-process test lands, Tasks 1–3 are verified only by build + the same-server suites. Task 1 (pure refactor) is the safest; Task 2 (constructor threading) is the most error-prone — lean on the compiler and the `agent_registry` precedent.
- **`state_for_hosted_turn: None` on the relay path** means a hosted-assistant addressed OVER federation is not spawned in P3a (logged, humans still see the message). Connector-agent delivery — the P3a goal — does not need it. Hosted-target-over-fed is P3b/parity.
- **`local_delivery_tx` is retained** alongside the new `deliver_message` call so the reference echo loop and backfill consumers keep working; in a real (non-reference) deployment nothing consumes it, so no double-delivery to real clients. Confirm the echo loop isn't double-counting in the two-process test.
- **`sender_type` cross-server is derived, not carried.** P3a derives it from the registered identity `type_`. P3b may instead carry an explicit `sender_type` in the relay envelope for robustness.
- **mTLS-in-CI is the automation risk.** The in-process `relay_handler` test is the guaranteed floor; the two-process test is best-effort. Do NOT block P3a on green two-process CI — the manual runbook (Task 5) is the live proof, consistent with FCP-5.
- **`participant_type` accessor** may need adding to `IdentityStore` (`identity.rs`) — a tiny additive read mirroring `get_public_key_bytes`.
```
