# Runtime A2A — P3b: Cross-Server A2A Semantics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On the production federation seam P3a built, make a cross-server agent↔agent conversation actually work end-to-end — an agent on server A can address an agent (or the hosted assistant) on server B, B enforces the same consent + hop guard, and the reply returns into the federated thread. This delivers success criterion 3.

**Architecture:** Three moving parts. **(1) Outbound** — when a message's `to` carries a foreign `@domain`, the sending server relays it to that domain (the agent branch had no such path; it only relayed by thread-origin). **(2) Inbound consent** — a signature-verified relayed *agent* sender is **auto-invited** into the thread on the receiving server (B trusts A's mTLS + Ed25519 attestation transitively), satisfying the P2 consent gate's "sender is human-invited" half for cross-server peers without a redundant human invite on B. **(3) Hosted-over-federation** — give the relay path a `ServerState` so a relayed message can spawn B's hosted assistant turn, and relay the hosted reply home to the thread origin.

**Tech Stack:** Rust 1.85 / Axum 0.8 server (`server/src/`); federation as the separate `run_federation` binary path (`main.rs`); redb persistence for the relay queue (no Postgres needed for the in-process federation test); Ed25519 via `ring`; mTLS relay over `POST /ainm/v1/relay`. No iOS, no connector-SDK changes — SP3/SP4 already render whatever lands in the thread.

**Standing constraints (carried from P1/P2/P3a):** commits go directly to `main` via `rtk proxy git` (never bare git); pushing to origin is user-gated separately at the end (do NOT push during execution); pure-logic-first testing; verbatim-code plan → skip the per-task two-stage review, run ONE holistic opus review at the end; the two pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`) stay out of scope; never log private-key bytes.

**Design decisions locked for this plan (user, 2026-06-09):**
1. **Cross-server consent = auto-invite the verified relayed agent sender.** B treats the mTLS+signature chain as the membership vouch; no manual invite on B required.
2. **Hosted-target-over-federation is IN SCOPE for P3b** (Tasks 5–6).

**Decision NOT taken — `sender_type` is derived, not carried in the relay.** Adding `sender_type` to a relayed message object would change its JSON canonicalization and break the per-message Ed25519 signature (the sender never signed `sender_type`). P3a already derives `sender_type` on B from the sender's registered identity `type_` (resolved via `/.well-known/ainm/participant/{sender}` during signature verification). That is the correct, signature-safe approach; this plan keeps it and does **not** carry `sender_type` on the wire.

---

## Current-State Anchors (verified 2026-06-09)

- `server/src/routing.rs` — pure helpers: `resolve_recipient`, `a2a_hop_decision`/`HopDecision`, `consent_ok`. Exhaustively unit-tested. (Task 1 adds `foreign_relay_target` here.)
- `server/src/server.rs`:
  - `deliver_message(deps, params, sender_type, verified)` at `:1572` — the shared orchestrator. Consent gate at `:1655` via `consent_ok(from_invited, to_invited, to_accepts)`.
  - `AgentRegistry::invite_to_thread` (`:276`) / `is_invited` (`:284`).
  - **Agent send branch** (federation-aware) at `:2118–2133` — routes by thread-origin domain only; a **local-origin** thread with a foreign `to` is delivered locally and **never relayed** (the outbound gap).
  - **Human send branch** at `:2827–2882` — local-origin path fans out to participating servers (`:2851`) + known remotes (`:2858`).
  - `FederationContext` struct at `:43` (`#[derive(Clone)]`): fields `server_id`, `thread_registry`, `relay_sender`, `local_delivery_tx`, `remote_cache`.
  - `ServerState` struct at `:315` (`#[derive(Clone)]`); `start_federation` at `:1023` builds `let state = ServerState {…}` at `:1048`, moves it into `.with_state(state)` at `:1097`, and returns `(SocketAddr, JoinHandle)`. **Only caller: `main.rs:620`.**
- `server/src/federation.rs`:
  - `FederationState` struct at `:150` — already carries `agent_registry`/`human_registry`/`a2a_hops`/`a2a_hop_cap`/`hosted_agent` (P3a). (Task 5 adds `hosted_turn_state`.)
  - `relay_handler` per-message delivery block at `:496–512` — derives `sender_type`, builds `DeliverDeps` with `state_for_hosted_turn: None`, calls `deliver_message`. (Tasks 4 + 5 edit here.)
  - In-process federation test `relay_delivers_to_connected_agent` at `:895` — builds a `FederationState` literal at `:961` and currently **pre-invites both** `alpha` and `beta` (`:947–953`). (Task 4 edits this test + adds a denied-case test.)
- `server/src/runtime_server.rs`:
  - `handle_turn(state, thread_id, requester, inbound_user_text)` at `:15`; calls `stream_reply(state, agent, thread_id, &outcome.final_text)` at `:127`.
  - `stream_reply(state, agent, thread_id, text)` at `:207`; builds the signed inner `message` (`:232`), wraps it in `ainm.message.deliver`, and `send_to_thread`s locally at `:258`. (Task 6 edits here.)
- `server/src/main.rs` — `run_federation`: `fed_state` literal at `:593`; `start_federation(...)` call at `:620`; mTLS relay router built from `fed_state` after `:644`. (Task 5 edits here.)
- `server/src/relay.rs` — `RelaySender::enqueue(destination, message)` (`:100`): `destination` is a server domain (must be in `remote_cache` to flush); `message` is the signed inner message envelope. The send loop wraps queued messages into the server-signed `ainm.relay` payload.

---

## File Structure

- **Modify** `server/src/routing.rs` — add pure `foreign_relay_target` + unit tests (Task 1).
- **Modify** `server/src/server.rs` — add `relay_to_foreign_target` helper; wire it into the agent send branch; register the foreign `to`-domain in the human send branch; change `start_federation` to return its `ServerState` (Tasks 2, 3, 5).
- **Modify** `server/src/federation.rs` — auto-invite the verified relayed agent sender; add `hosted_turn_state` to `FederationState`; pass it into `DeliverDeps`; extend the in-process test + add a denied-case test (Tasks 4, 5).
- **Modify** `server/src/main.rs` — capture `start_federation`'s returned `ServerState` and inject it into `fed_state.hosted_turn_state` (Task 5).
- **Modify** `server/src/runtime_server.rs` — `stream_reply` takes `requester`, stamps `to`, and relays the hosted reply home for foreign-origin threads (Task 6).
- **Modify** `docs/runbooks/2026-06-09-federation-a2a-verify.md` — add the two-server agent↔agent + hosted-over-fed verify steps (Task 7).

---

### Task 1: Pure `foreign_relay_target` routing decision

**Files:**
- Modify: `server/src/routing.rs` (append a function after `consent_ok`, ~`:118`; tests into the existing `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing tests**

Add inside `mod tests` in `server/src/routing.rs` (after `consent_denied_when_target_opts_out`, before the closing `}`):

```rust
    #[test]
    fn foreign_target_returns_other_domain() {
        assert_eq!(
            foreign_relay_target(Some("beta@server-b.local"), "server-a.local"),
            Some("server-b.local".to_string())
        );
    }

    #[test]
    fn foreign_target_none_for_local_domain() {
        assert_eq!(foreign_relay_target(Some("beta@server-a.local"), "server-a.local"), None);
    }

    #[test]
    fn foreign_target_none_without_domain() {
        // bare local-part (no @domain) is not a federation target
        assert_eq!(foreign_relay_target(Some("beta"), "server-a.local"), None);
    }

    #[test]
    fn foreign_target_none_for_absent_or_empty_to() {
        assert_eq!(foreign_relay_target(None, "server-a.local"), None);
        assert_eq!(foreign_relay_target(Some(""), "server-a.local"), None);
        assert_eq!(foreign_relay_target(Some("@server-b.local"), "server-a.local"), Some("server-b.local".to_string()));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p ainm-server --lib routing::tests::foreign_target 2>&1 | tail -20`
Expected: FAIL — `cannot find function foreign_relay_target in this scope`.

- [ ] **Step 3: Write the implementation**

Add to `server/src/routing.rs` immediately after the `consent_ok` function (after its closing `}` at ~`:118`):

```rust
/// The foreign home-domain to relay a message to, given its routing target `to` and this
/// server's own domain. Returns `Some(domain)` only when `to` carries an `@domain` that
/// differs from `server_id` (the target lives on another server). Returns `None` for a
/// local target, a domain-less `to`, or an absent/empty `to`.
///
/// Pure: the caller (`server.rs`) performs the actual relay enqueue / participant registration.
pub fn foreign_relay_target(to: Option<&str>, server_id: &str) -> Option<String> {
    let to = to?;
    let domain = to.split('@').nth(1)?;
    if domain.is_empty() || domain == server_id {
        return None;
    }
    Some(domain.to_string())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p ainm-server --lib routing::tests::foreign_target 2>&1 | tail -20`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
rtk proxy git add server/src/routing.rs
rtk proxy git commit -m "feat(a2a-p3b): pure foreign_relay_target routing decision"
```

---

### Task 2: Outbound — relay agent messages addressed to a foreign peer

**Files:**
- Modify: `server/src/server.rs` — add `relay_to_foreign_target` helper (near `deliver_message`, after its closing `}` at ~`:1690`); wire into the agent send branch at `:2126` (the local-origin `else`).

- [ ] **Step 1: Add the relay helper**

Insert into `server/src/server.rs` immediately after the closing `}` of `deliver_message` (~`:1690`):

```rust
/// Runtime A2A P3b: if `params.to` is addressed to an agent on another server, relay the
/// message to that agent's home domain and register the domain as a thread participant
/// (so later origin fan-out also reaches it). Returns `true` if a foreign relay was enqueued.
/// A local target (or no `to`) returns `false`; the caller delivers it locally as usual.
///
/// Used by the AGENT send branch, which (unlike the human branch) has no participating-server
/// fan-out of its own — this is the only outbound path for an agent addressing a foreign peer.
fn relay_to_foreign_target(
    fed: &FederationContext,
    thread_id: &str,
    params: &serde_json::Value,
) -> bool {
    let to = params.get("to").and_then(|t| t.as_str());
    match crate::routing::foreign_relay_target(to, &fed.server_id) {
        Some(domain) => {
            fed.thread_registry.add_server(thread_id, &domain);
            fed.relay_sender.enqueue(&domain, params.clone());
            tracing::info!(
                "[a2a-p3b] relayed agent message to foreign target domain {} (thread {})",
                domain, thread_id
            );
            true
        }
        None => false,
    }
}
```

- [ ] **Step 2: Wire it into the agent send branch**

In `server/src/server.rs`, the agent send branch currently reads (at `:2119–2133`):

```rust
                            if let Some(ref fed) = state.federation {
                                let origin = federation::parse_origin_domain(thread_id)
                                    .unwrap_or("");

                                if origin != fed.server_id && !origin.is_empty() {
                                    // Thread originated on remote server: relay response back (P3 generalizes this).
                                    fed.relay_sender.enqueue(origin, params.clone());
                                } else {
                                    // Local thread: targeted delivery (humans + addressed agent / hosted).
                                    deliver_message(&state.deliver_deps(), params, "agent", verified).await;
                                }
                            } else {
                                // Legacy mode: targeted local delivery.
                                deliver_message(&state.deliver_deps(), params, "agent", verified).await;
                            }
```

Replace the inner `else` block so a local-origin thread also relays a foreign-addressed peer **before** local delivery. The new version of the whole `if let Some(ref fed)` block:

```rust
                            if let Some(ref fed) = state.federation {
                                let origin = federation::parse_origin_domain(thread_id)
                                    .unwrap_or("");

                                if origin != fed.server_id && !origin.is_empty() {
                                    // Thread originated on remote server: relay response back to origin.
                                    fed.relay_sender.enqueue(origin, params.clone());
                                } else {
                                    // Local-origin thread. P3b: if addressed to a foreign-host agent,
                                    // relay to that agent's home domain, THEN deliver locally (humans
                                    // on this server still see the message; a local addressed agent is
                                    // also invoked). A purely-local target just delivers locally.
                                    relay_to_foreign_target(fed, thread_id, params);
                                    deliver_message(&state.deliver_deps(), params, "agent", verified).await;
                                }
                            } else {
                                // Legacy mode: targeted local delivery.
                                deliver_message(&state.deliver_deps(), params, "agent", verified).await;
                            }
```

(Only the `else` arm changed: added the `relay_to_foreign_target(fed, thread_id, params);` line and its comment. The remote-origin arm is unchanged — it must stay relay-only to avoid double-delivery, since the origin server fans the message back to this server.)

- [ ] **Step 3: Verify it builds**

Run: `cargo build -p ainm-server 2>&1 | tail -20`
Expected: builds clean (warnings OK; this repo does not enforce `-D warnings`).

- [ ] **Step 4: Commit**

```bash
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "feat(a2a-p3b): relay agent messages addressed to a foreign peer"
```

---

### Task 3: Outbound — register the foreign `to`-domain in the human send branch

**Why a different mechanism than Task 2:** the human branch already fans out to all participating servers + known remotes. Calling `relay_to_foreign_target` here would enqueue a *second* copy (the fan-out below would send the first), so B would receive the message twice (replay protection keys on `relay_id`, not message `id`, so both are accepted). Instead, only **register** the foreign domain as a participant *before* the fan-out, so the existing loop relays to it exactly once.

**Files:**
- Modify: `server/src/server.rs` — human send branch local-origin path, after `let origin = …` (~`:2833`), before the deliver/fan-out.

- [ ] **Step 1: Insert the participant registration**

In `server/src/server.rs`, the human branch currently reads (at `:2828–2837`):

```rust
                            if let Some(ref fed) = state.federation {
                                let thread_id = params
                                    .get("thread_id")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("");
                                let origin = federation::parse_origin_domain(thread_id)
                                    .unwrap_or("");

                                if origin == fed.server_id || origin.is_empty() {
                                    // We are origin: deliver locally AND relay to remote servers
```

Insert the registration as the first statement inside the `if origin == fed.server_id || origin.is_empty() {` block (i.e. immediately after that line, before the `let deliver = …`):

```rust
                                if origin == fed.server_id || origin.is_empty() {
                                    // P3b: a human addressing a foreign-host agent — register that
                                    // agent's home domain as a thread participant so the fan-out below
                                    // relays to it EXACTLY ONCE (no separate enqueue → no double send).
                                    // This is also how the remote agent gets human-invited on its own
                                    // server: the relayed human→agent message invites it there.
                                    if let Some(domain) = crate::routing::foreign_relay_target(
                                        params.get("to").and_then(|t| t.as_str()),
                                        &fed.server_id,
                                    ) {
                                        fed.thread_registry.add_server(thread_id, &domain);
                                    }
                                    // We are origin: deliver locally AND relay to remote servers
```

(The rest of the block — the `let deliver`, `deliver_message`, the participating-servers fan-out, and the known-remotes loop — is unchanged. Because `add_server` runs first, `thread_registry.get_info(thread_id)` in the fan-out now includes the new domain and relays to it once.)

- [ ] **Step 2: Verify it builds**

Run: `cargo build -p ainm-server 2>&1 | tail -20`
Expected: builds clean.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "feat(a2a-p3b): register foreign to-domain in human send fan-out"
```

---

### Task 4: Inbound — auto-invite the verified relayed agent sender + tests

**Files:**
- Modify: `server/src/federation.rs` — relay delivery block at `:496–512`; in-process test at `:895` + a new denied-case test.

- [ ] **Step 1: Update the existing in-process test to PROVE auto-invite (write the failing assertion first)**

In `server/src/federation.rs`, the test `relay_delivers_to_connected_agent` currently pre-invites both agents (at `:947–953`):

```rust
        // 7. P2 consent: simulate prior human invitation for both alpha and beta
        agent_registry
            .invite_to_thread("alpha@server-a.local", &thread_id)
            .await;
        agent_registry
            .invite_to_thread("beta@server-b.local", &thread_id)
            .await;
```

Replace with — invite ONLY the target `beta`; `alpha` (the foreign sender) is deliberately NOT invited, so the test now depends on the P3b auto-invite to pass:

```rust
        // 7. P2 consent: a human on B invited the TARGET (beta). The foreign SENDER (alpha) is
        //    deliberately NOT invited here — P3b must auto-invite the verified relayed agent
        //    sender, or consent_ok would deny this agent→agent message.
        agent_registry
            .invite_to_thread("beta@server-b.local", &thread_id)
            .await;
```

- [ ] **Step 2: Run the test to verify it now FAILS (auto-invite not yet implemented)**

Run: `cargo test -p ainm-server --lib federation::tests::relay_delivers_to_connected_agent 2>&1 | tail -30`
Expected: FAIL — `beta receiver closed` / timeout, because `consent_ok(from_invited=false, …)` denies delivery (alpha not invited).

- [ ] **Step 3: Implement the auto-invite in `relay_handler`**

In `server/src/federation.rs`, the per-message delivery block currently reads (at `:496–512`):

```rust
        {
            let sender = msg.get("from").and_then(|f| f.as_str()).unwrap_or("");
            let sender_type = match state.identity_store.participant_type(sender).as_deref() {
                Some("agent") => "agent",
                _ => "human",
            };
            let deps = crate::server::DeliverDeps {
                human_registry: &state.human_registry,
                agent_registry: &state.agent_registry,
                a2a_hops: &state.a2a_hops,
                a2a_hop_cap: state.a2a_hop_cap,
                hosted_agent: state.hosted_agent.as_deref(),
                to_agent_tx: &state.local_delivery_tx,
                state_for_hosted_turn: None,
            };
            crate::server::deliver_message(&deps, msg, sender_type, true).await;
        }
```

Insert the auto-invite between the `sender_type` computation and the `DeliverDeps` build:

```rust
        {
            let sender = msg.get("from").and_then(|f| f.as_str()).unwrap_or("");
            let sender_type = match state.identity_store.participant_type(sender).as_deref() {
                Some("agent") => "agent",
                _ => "human",
            };

            // Runtime A2A P3b: a signature-verified relayed AGENT sender is auto-invited into the
            // thread on THIS (receiving) server. The relay arrived over mTLS and the per-message
            // Ed25519 signature was verified above (step 3), so the origin server vouches for the
            // sender as a real thread participant. This satisfies the P2 consent gate's
            // "sender is human-invited" half for cross-server peers without a redundant manual
            // invite on the receiving side. (The TARGET's invitation remains B's own
            // human-curated membership — established when a human's relayed message invited it.)
            if sender_type == "agent" && !sender.is_empty() && !thread_id.is_empty() {
                state.agent_registry.invite_to_thread(sender, thread_id).await;
            }

            let deps = crate::server::DeliverDeps {
                human_registry: &state.human_registry,
                agent_registry: &state.agent_registry,
                a2a_hops: &state.a2a_hops,
                a2a_hop_cap: state.a2a_hop_cap,
                hosted_agent: state.hosted_agent.as_deref(),
                to_agent_tx: &state.local_delivery_tx,
                state_for_hosted_turn: None,
            };
            crate::server::deliver_message(&deps, msg, sender_type, true).await;
        }
```

(`thread_id` is the loop variable declared at `:463`; it is in scope here.)

- [ ] **Step 4: Run the updated test to verify it PASSES**

Run: `cargo test -p ainm-server --lib federation::tests::relay_delivers_to_connected_agent 2>&1 | tail -30`
Expected: PASS — beta receives the `ainm.message.deliver` even though only beta was pre-invited (auto-invite supplied alpha's consent).

- [ ] **Step 5: Add a denied-case test (target NOT invited → no delivery)**

Add a second test inside `mod tests` in `server/src/federation.rs`, after `relay_delivers_to_connected_agent`'s closing `}` (before the module's closing `}` at `:1034`). This reuses the helpers `make_persistence`, `build_inner_msg`, `build_relay_payload`:

```rust
    /// P3b: when the TARGET agent was never human-invited on B, the auto-invited foreign
    /// sender is not enough — consent_ok still denies (both endpoints must be invited).
    /// Asserts beta receives NOTHING (the message drops to humans-only with a notice).
    #[tokio::test]
    async fn relay_denied_when_target_not_invited() {
        let persistence = make_persistence();

        let server_b_kp =
            crate::identity::generate_keypair("server-b.local").expect("server-b keypair");
        let server_b_identity = crate::identity::build_server_identity(
            "server-b.local",
            &server_b_kp,
            8443,
            vec![],
        );

        let server_a_kp =
            crate::identity::generate_keypair("server-a.local").expect("server-a keypair");
        let remote_cache = crate::identity::RemoteServerCache::new();
        remote_cache.insert(crate::identity::ServerIdentity {
            server_id: "server-a.local".to_string(),
            public_key_b64: server_a_kp.public_key_b64(),
            relay_endpoint: "https://server-a.local:8443/ainm/v1/relay".to_string(),
            tls_fingerprints: vec![],
            supported_versions: vec!["0.1.0".to_string()],
        });

        let alpha_kp =
            crate::identity::generate_keypair("alpha@server-a.local").expect("alpha keypair");
        let identity_store = crate::identity::IdentityStore::new();
        identity_store.register(
            "alpha@server-a.local",
            &alpha_kp.public_key_b64(),
            "agent",
            "server-a.local",
            None,
        );

        let agent_registry = crate::server::AgentRegistry::new();
        let beta_card = serde_json::json!({ "name": "beta", "ainm": { "acceptsA2A": true } });
        let mut beta_rx = agent_registry
            .register("beta@server-b.local", &beta_card)
            .await;

        let thread_id = format!("server-a.local:{}", uuid::Uuid::now_v7());

        // NOTE: beta is NOT invited here. alpha will be auto-invited by relay_handler, but
        // consent requires BOTH — so delivery to beta must be denied.

        let (local_delivery_tx, _rx) = tokio::sync::broadcast::channel::<String>(64);
        let a2a_hops = std::sync::Arc::new(tokio::sync::Mutex::new(
            std::collections::HashMap::<String, u32>::new(),
        ));
        let fed_state = FederationState {
            server_id: "server-b.local".to_string(),
            server_keypair: std::sync::Arc::new(server_b_kp),
            server_identity: std::sync::Arc::new(server_b_identity),
            thread_registry: ThreadRegistry::new(),
            identity_store,
            remote_cache,
            relay_sender: None,
            local_delivery_tx,
            persistence,
            http_client: None,
            agent_registry: agent_registry.clone(),
            human_registry: crate::server::HumanRegistry::new(),
            a2a_hops,
            a2a_hop_cap: 6,
            hosted_agent: None,
            hosted_turn_state: None,
        };

        let inner_msg = build_inner_msg(&alpha_kp, &thread_id, "beta@server-b.local");
        let payload = build_relay_payload(&server_a_kp, vec![inner_msg]);

        let (status, body) = relay_handler(State(fed_state), Json(payload)).await;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert!(body.0.get("error").is_none(), "relay returned error: {:?}", body.0);

        // beta must NOT receive the message (consent denied — target not invited).
        let res = tokio::time::timeout(Duration::from_millis(300), beta_rx.recv()).await;
        assert!(
            res.is_err(),
            "expected NO delivery to beta (consent denied), but got: {:?}",
            res
        );
    }
```

(Note: this test's `FederationState` literal includes `hosted_turn_state: None` — Task 5 adds that field. If executing Task 4 strictly before Task 5, temporarily omit that line and add it in Task 5; the recommended order is to apply Task 5's struct-field addition first if the compiler complains. Implementer: if the field doesn't exist yet, add Task 5 Step 1 before compiling this test.)

- [ ] **Step 6: Run both federation tests**

Run: `cargo test -p ainm-server --lib federation::tests 2>&1 | tail -30`
Expected: PASS — `relay_delivers_to_connected_agent` (auto-invite) and `relay_denied_when_target_not_invited` (consent denial). Both run with NO Postgres (redb only).

- [ ] **Step 7: Commit**

```bash
rtk proxy git add server/src/federation.rs
rtk proxy git commit -m "feat(a2a-p3b): auto-invite verified relayed agent sender + consent tests"
```

---

### Task 5: Hosted-over-federation (inbound) — give the relay path a ServerState

**Goal:** a relayed message whose `to` resolves to B's hosted assistant must spawn `runtime_server::handle_turn`. Today the relay path passes `state_for_hosted_turn: None` because `FederationState` has no `ServerState`. We make `start_federation` return the `ServerState` it builds and inject it into `FederationState.hosted_turn_state`.

**Files:**
- Modify: `server/src/federation.rs` — add `hosted_turn_state` field to `FederationState`; pass into `DeliverDeps`; add the field to the in-process test literal(s).
- Modify: `server/src/server.rs` — `start_federation` returns its `ServerState`.
- Modify: `server/src/main.rs` — capture the returned state, inject into `fed_state`.

- [ ] **Step 1: Add the field to `FederationState`**

In `server/src/federation.rs`, the `FederationState` struct ends with (at `:177`):

```rust
    pub hosted_agent: Option<std::sync::Arc<crate::hosted_agent::HostedAgent>>,
}
```

Add the new field before the closing brace:

```rust
    pub hosted_agent: Option<std::sync::Arc<crate::hosted_agent::HostedAgent>>,
    /// P3b: the WS listener's `ServerState`, shared so the relay path can spawn a hosted
    /// assistant turn for a relayed message addressed to this server's hosted agent.
    /// `None` in tests / when no hosted agent is provisioned.
    pub hosted_turn_state: Option<std::sync::Arc<crate::server::ServerState>>,
}
```

- [ ] **Step 2: Pass it into the relay's `DeliverDeps`**

In `server/src/federation.rs`, the relay delivery block (edited in Task 4) sets `state_for_hosted_turn: None`. Change that line to:

```rust
                state_for_hosted_turn: state.hosted_turn_state.as_deref(),
```

(`Option<Arc<ServerState>>::as_deref()` → `Option<&ServerState>`, matching `DeliverDeps.state_for_hosted_turn`.)

- [ ] **Step 3: Add the field to the in-process test literal(s)**

In both test `FederationState` literals in `server/src/federation.rs` (`relay_delivers_to_connected_agent` at `:961` and the new `relay_denied_when_target_not_invited`), add after `hosted_agent: None,`:

```rust
            hosted_agent: None,
            hosted_turn_state: None,
```

- [ ] **Step 4: Make `start_federation` return its `ServerState`**

In `server/src/server.rs`, change the `start_federation` return type (`:1044`):

```rust
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
```

to:

```rust
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>, ServerState)> {
```

The body builds `let state = ServerState {…}` at `:1048` and currently consumes it at `.with_state(state)` (`:1097`). Clone for axum and keep the original to return. Change `:1097`:

```rust
        .with_state(state);
```

to:

```rust
        .with_state(state.clone());
```

And change the final `Ok((addr, handle))` (`:1109`) to:

```rust
    Ok((addr, handle, state))
```

(`ServerState` is `#[derive(Clone)]`; all its fields are `Arc`/handles, so the clone shares the same underlying registries, db pool, hosted agent, etc.)

- [ ] **Step 5: Capture + inject the state in `run_federation`**

In `server/src/main.rs`:

(a) Make `fed_state` mutable and initialize the new field. The `fed_state` literal at `:593` ends with:

```rust
        hosted_agent: hosted_agent_for_fed.clone(),
    };
```

Change the `let fed_state = …` opener (at `:593`) to `let mut fed_state = …`, and add the field before the closing brace:

```rust
        hosted_agent: hosted_agent_for_fed.clone(),
        hosted_turn_state: None,
    };
```

(b) Capture the returned state from the `start_federation` call at `:620`. It currently reads:

```rust
    let (server_addr, server_handle) = server::start_federation(
```

Change to:

```rust
    let (server_addr, server_handle, ws_listener_state) = server::start_federation(
```

(c) Immediately after that call returns (after the `.await?;` at `:642`), inject the state:

```rust
    .await?;

    // P3b: share the WS listener's ServerState with the relay path so a relayed message
    // addressed to the hosted assistant can spawn its turn (same db / hosted_agent / llm).
    fed_state.hosted_turn_state = Some(std::sync::Arc::new(ws_listener_state));
```

(The mTLS relay router that consumes `fed_state` is built afterwards, so the field is set before use.)

- [ ] **Step 6: Verify it builds + federation tests still pass**

Run: `cargo build -p ainm-server 2>&1 | tail -20 && cargo test -p ainm-server --lib federation::tests 2>&1 | tail -15`
Expected: builds clean; both federation tests pass.

- [ ] **Step 7: Commit**

```bash
rtk proxy git add server/src/federation.rs server/src/server.rs server/src/main.rs
rtk proxy git commit -m "feat(a2a-p3b): share ServerState into relay path for hosted-over-federation"
```

---

### Task 6: Hosted reply routing home

**Goal:** when B's hosted assistant replies in a thread that originated on another server, relay the reply back to the origin so it lands in the federated thread. Also stamp `to = requester` on the hosted reply so it is directionally addressed (enables the origin to route to the addressing peer and feeds SP3 bylines).

**Files:**
- Modify: `server/src/runtime_server.rs` — `handle_turn` call site (`:127`); `stream_reply` signature (`:207`), the message envelope (`:232`), and the local-send tail (`:258`).

- [ ] **Step 1: Pass `requester` into `stream_reply`**

In `server/src/runtime_server.rs`, the call at `:127`:

```rust
    stream_reply(state, agent, thread_id, &outcome.final_text).await;
```

Change to:

```rust
    stream_reply(state, agent, thread_id, requester, &outcome.final_text).await;
```

- [ ] **Step 2: Update the `stream_reply` signature**

At `:207`:

```rust
async fn stream_reply(state: &ServerState, agent: &crate::hosted_agent::HostedAgent, thread_id: &str, text: &str) {
```

Change to:

```rust
async fn stream_reply(
    state: &ServerState,
    agent: &crate::hosted_agent::HostedAgent,
    thread_id: &str,
    requester: &str,
    text: &str,
) {
```

- [ ] **Step 3: Stamp `to` on the signed message**

At `:232`, the message is built as:

```rust
    let mut message = serde_json::json!({
        "id": uuid::Uuid::now_v7().to_string(),
        "from": agent.address,
        "thread_id": thread_id,
        "type": "message",
```

Add a `to` line after `from`:

```rust
    let mut message = serde_json::json!({
        "id": uuid::Uuid::now_v7().to_string(),
        "from": agent.address,
        "to": requester,
        "thread_id": thread_id,
        "type": "message",
```

(The hosted reply is addressed back to whoever triggered the turn — a human, or, in an A2A chain, the addressing agent. `to` is part of the signed envelope, which is correct: the agent asserts it. Same-server behavior is unaffected because `stream_reply` delivers via `send_to_thread`, not the routing path; the added `to` only enriches rendering and enables the relayed copy below to be routed on the origin.)

- [ ] **Step 4: Relay the reply home for foreign-origin threads**

At `:258`, the tail of `stream_reply`:

```rust
    state.human_registry.send_to_thread(thread_id, &envelope.to_string()).await;
}
```

Change to (keep the local send, then add the relay-home):

```rust
    state.human_registry.send_to_thread(thread_id, &envelope.to_string()).await;

    // Runtime A2A P3b: if this thread originated on another server, relay the hosted agent's
    // reply back to the origin so it reaches the federated thread (humans + the addressing peer
    // on the origin side). `message` is the signed inner envelope the relay queue expects.
    if let Some(fed) = &state.federation {
        if let Some(origin) = crate::federation::parse_origin_domain(thread_id) {
            if !origin.is_empty() && origin != fed.server_id {
                fed.relay_sender.enqueue(origin, message.clone());
                tracing::info!(
                    "[a2a-p3b] relayed hosted reply home to origin {} (thread {})",
                    origin, thread_id
                );
            }
        }
    }
}
```

- [ ] **Step 5: Verify it builds + the hosted-runtime tests still pass**

Run: `cargo build -p ainm-server 2>&1 | tail -20 && cargo test -p ainm-server --lib runtime 2>&1 | tail -20`
Expected: builds clean; existing runtime tests pass (any test calling `stream_reply` indirectly via `handle_turn` is unaffected; the `to`/relay additions are additive).

- [ ] **Step 6: Commit**

```bash
rtk proxy git add server/src/runtime_server.rs
rtk proxy git commit -m "feat(a2a-p3b): hosted reply carries to + relays home for foreign threads"
```

---

### Task 7: Runbook, full verification, memory, holistic review

**Files:**
- Modify: `docs/runbooks/2026-06-09-federation-a2a-verify.md`
- Update memory: `~/.claude/projects/-Users-santiagogerickeparga-Proyectos-ai-native-messaging/memory/project_runtime_a2a.md`

- [ ] **Step 1: Extend the runbook with the P3b cross-server scenarios**

Append a `## P3b — Cross-server A2A (live verify)` section to `docs/runbooks/2026-06-09-federation-a2a-verify.md` documenting the manual two-process mTLS scenarios (no in-harness mTLS client exists, so these stay MANUAL, consistent with FCP-5):

```markdown
## P3b — Cross-server A2A (live verify)

**Automated coverage (runs in CI, no Postgres):** `cargo test -p ainm-server --lib federation::tests`
— `relay_delivers_to_connected_agent` proves the auto-invite (foreign agent sender reaches a
connected target it never manually invited); `relay_denied_when_target_not_invited` proves the
consent gate still denies when the target was not human-invited. Plus `routing::tests::foreign_target*`
for the outbound `to`-domain decision. The transport-level (real mTLS, two processes) checks below
are the MANUAL complement CI does not automate.

### Scenario A — connector↔connector across servers
1. Start A and B per the P3a topology above; connect connector agent `beta@server-b.local` to B,
   and connector agent `alpha@server-a.local` to A.
2. On A, as a human, create a federated thread and address BOTH `alpha@server-a.local` and
   `beta@server-b.local` (this human-invites alpha on A and — via the relayed human message —
   beta on B).
3. Have `alpha` send a message `to: beta@server-b.local`.
   - **Expected:** A relays to `server-b.local` (Task 2). On B, the relay auto-invites `alpha`
     (Task 4); consent passes (beta was human-invited on B in step 2); `beta` receives it.
   - `beta` replies `to: alpha@server-a.local`; B relays to origin `server-a.local`; on A the relay
     auto-invites `beta`, consent passes (alpha invited on A), `alpha` receives it.
   - The hop counter increments independently on each server; a human message on either side resets
     that server's counter (per-server accounting; the stricter bound wins).
4. **Pass:** the agent↔agent exchange round-trips; humans on both servers see the SP3/SP4-rendered
   bursts; no double-delivery; every consent/hop rejection (if forced) is a visible system notice.

### Scenario B — hosted assistant over federation
1. B runs with a provisioned hosted assistant (`assistant@server-b.local`).
2. On A, address `assistant@server-b.local` from a human (and/or from `alpha`).
   - **Expected:** A relays to B; B's relay path spawns `handle_turn` (Task 5); the hosted reply is
     stamped `to` and relayed home to A (Task 6); humans on A see the assistant's reply in-thread.
3. **Pass:** the hosted assistant on B participates in the A-origin thread end-to-end.

### Known limits (P3b scope)
- Outbound agent relay is targeted to the `to`-domain only (no agent-initiated fan-out to ALL
  participating servers); v1 sufficient for addressed A2A.
- `sender_type` is DERIVED on the receiving server from the sender's registered identity type
  (resolved via `/.well-known/ainm/participant`), not carried in the relay — carrying it would
  break the per-message signature. Cross-server `sender_type` correctness therefore depends on the
  origin's participant endpoint returning the right `type`.
```

- [ ] **Step 2: Full server suite + clippy**

Run:
```bash
cargo test -p ainm-server 2>&1 | tail -30
cargo clippy -p ainm-server --all-targets 2>&1 | tail -30
```
Expected: all tests pass **except** the two known pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`). Clippy clean on touched files (repo does not enforce `-D warnings`; pre-existing lints elsewhere are out of scope). The DB-gated WS E2E tests (`a2a_*_e2e`, `ws_conversation_e2e`) self-skip without Postgres.

- [ ] **Step 3: Update the project memory**

Update `~/.claude/projects/-Users-santiagogerickeparga-Proyectos-ai-native-messaging/memory/project_runtime_a2a.md`: record P3b shipped — outbound `to`-domain relay (agent branch relay + human branch participant-register), inbound auto-invite of the verified relayed agent sender (the resolution of the foreign-sender invitation gap), hosted-over-federation (ServerState shared into the relay path + hosted reply stamps `to` and relays home), per-server hop accounting confirmed, `sender_type` derived-not-carried (signature-safe). Note the two new in-process federation tests run without Postgres. Mark criterion 3 met. Note remaining manual live verifies (Scenarios A/B) and that agent-initiated cross-server fan-out beyond the `to`-target is deferred.

- [ ] **Step 4: Commit docs + memory**

```bash
rtk proxy git add docs/runbooks/2026-06-09-federation-a2a-verify.md
rtk proxy git commit -m "docs(a2a-p3b): runbook cross-server A2A + hosted-over-fed verify scenarios"
```

- [ ] **Step 5: ONE holistic opus review of the whole P3b diff**

Dispatch a single opus code-review subagent over the full P3b commit range (`git diff` from the pre-Task-1 SHA to HEAD). Focus: (a) no double-delivery on any cross-server path (agent relay vs human fan-out vs origin round-trip); (b) the auto-invite is correctly gated on `sender_type == "agent"` and a verified signature (never widens consent for unverified/human senders); (c) `start_federation`'s returned `ServerState` shares the same Arc-backed registries the WS listener uses (no divergent second instance); (d) the hosted reply-home relay can't loop unbounded (hop guard applies on the origin when the relayed copy is delivered); (e) no private-key bytes logged. Address any blocking findings before declaring P3b done.

---

## Self-Review (writing-plans)

**Spec coverage (spec §3.7 + P3 revision → P3b scope):**
- "generalize relay to carry an agent `to`" → Tasks 2–3 (outbound `to`-domain relay). ✔
- "second-server consent re-check" → already applied via P3a's `deliver_message` call; the missing piece (foreign-sender invitation) is Task 4 auto-invite. ✔
- "reply routing home" → connector replies already relay to origin (server.rs:2123, unchanged); hosted replies → Task 6. ✔
- "per-server hop re-accounting" → already per-server (each `ServerState`/`FederationState` has its own `a2a_hops`); no code change, confirmed + documented in runbook. ✔
- "foreign-sender signature verification" → already in P3a relay_handler (lazy `/.well-known/ainm/participant`), reused unchanged. ✔
- "carry `sender_type`" → explicitly NOT carried (signature-safety); derived instead — documented as a deliberate decision. ✔
- "hosted-target-over-federation" (user: include) → Tasks 5–6. ✔
- Success criterion 3 → Tasks 2+4 (connector↔connector) + 5+6 (hosted); manual live Scenarios A/B in the runbook. ✔

**Placeholder scan:** every code step shows complete verbatim code; no TBD/TODO. ✔

**Type consistency:** `foreign_relay_target(Option<&str>, &str) -> Option<String>` used identically in Tasks 2 & 3. `relay_to_foreign_target(&FederationContext, &str, &serde_json::Value) -> bool`. `FederationState.hosted_turn_state: Option<Arc<ServerState>>` → `.as_deref()` → `DeliverDeps.state_for_hosted_turn: Option<&ServerState>`. `start_federation` return tuple extended to 3-arity, the single caller (main.rs:620) updated. `stream_reply` gains `requester: &str`, the single caller (runtime_server.rs:127) updated. ✔

**Cross-task ordering note:** Task 4's denied-case test literal includes `hosted_turn_state: None`, a field Task 5 adds. The implementer should apply Task 5 Step 1 (the struct field) before compiling Task 4's new test, OR omit that line in Task 4 and add it in Task 5. Flagged inline in Task 4 Step 5.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-09-runtime-a2a-p3b-cross-server-semantics.md`. Two execution options:

1. **Subagent-Driven (recommended)** — fresh subagent per task; per project precedent for verbatim-code plans, skip the per-task two-stage review, verify build/test + diff between tasks, run ONE holistic opus review at the end (Task 7 Step 5). Commits to `main` via `rtk proxy git`; push to origin is user-gated separately at the end.
2. **Inline Execution** — execute tasks in this session via executing-plans with checkpoints.

Which approach?
