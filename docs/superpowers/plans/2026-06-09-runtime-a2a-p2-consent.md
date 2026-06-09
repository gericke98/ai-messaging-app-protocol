# Runtime A2A — P2 (Consent Policy) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Gate agent→agent delivery on human-established consent: a peer message is delivered only when a human has invited BOTH agents into the thread and the target agent has not opted out via its Agent Card (`acceptsA2A`). Every denial is visible to humans (no silent drop).

**Architecture:** Builds on P1 (same-server core, shipped). A new pure `consent_ok` predicate (in `routing.rs`) carries the policy logic under unit tests. `AgentRegistry` gains a per-thread "human-invited" membership set (distinct from P1's connection-membership) plus an `accepts_a2a` card accessor. `deliver_message` records a human→agent message as an invitation and, for the agent→agent case, runs `consent_ok` after the hop guard — emitting a visible system notice on denial. Connectors may opt out via a new `acceptsA2A` config flag carried in the handshake card object and read by `build_connector_card`.

**Tech Stack:** Rust (Axum/Tokio) server at `server/src`; cargo + nextest; black-box WS integration tests (DB-or-skip). TypeScript connector SDK at `packages/connector`.

**Standing constraints:**
- All git via `rtk proxy git` (NEVER bare `git`). Commit directly to `main` (no branches).
- Pure-logic-first testing. No iOS changes (humans curate membership through existing human→agent addressing — already works; no new UI).
- Never call the product a "demo".

**Consent semantics (decided):** *Human-established membership.* An agent is a consenting thread member only because a HUMAN brought it in — i.e. a human message in that thread targeted it (`to`/@mention resolving to that agent). Agent→agent delivery requires BOTH the sender and the target to be human-invited in the thread, AND the target's card `acceptsA2A != false`. Human→agent is never consent-gated (that is how invitation happens). First peer-contact with a never-human-invited agent → denied + visible notice.

**Scope boundary:** This is the same-server consent layer. Federation (P3) — cross-server consent re-checks, foreign-sender signature verification — is NOT in this plan. The hosted-assistant-as-initiator parity gap (a P1 follow-up) is also out of scope; the hosted assistant is consent-checked correctly as a *target* here.

---

## Background: current state after P1 (verified)

- `server/src/routing.rs` — pure `resolve_recipient`, `a2a_hop_decision`/`HopDecision` (+ tests). P2 adds `consent_ok` here.
- `server/src/server.rs`:
  - `AgentRegistry` (`~line 150`): `connections`, `agent_cards`, `thread_agents` (P1 connection-membership) + methods `register`/`cleanup`/`get_agent_card`/`send_to_agent`/`is_connected`/`join_thread`/`participants_of`/`leave_all_threads`.
  - `deliver_message` (`~line 1473`): resolves target; for an agent target runs the hop guard then delivers via `send_to_agent` (connected) or `spawn handle_turn` (hosted); human-no-target falls back to `to_agent_tx`. `a2a_limit_notice` (`~line 1450`) builds the hop-cap system notice.
- `server/src/agent_card.rs`: `build_assistant_card(address)` (line 14) and `build_connector_card(address, init_params)` (line 59); both emit an `ainm` extension object (with `memory`, `tools`, etc.). `build_connector_card` reads fields from `init_params.card`.
- Connector SDK: handshake sends a `card` object (name/description/version/skills/tools) in `ainm.initialize` params (`packages/connector/src/protocol/handshake.ts`); config is `AINMAgentConfig` (`packages/connector/src/protocol/types.ts`).

---

## File Structure

- `server/src/routing.rs` — add pure `consent_ok` + tests.
- `server/src/agent_card.rs` — add `"acceptsA2A"` to both card builders (assistant: `true`; connector: read `init_params.card.acceptsA2A`, default `true`) + tests.
- `server/src/server.rs` — `AgentRegistry`: add `thread_invited` map + `invite_to_thread`/`is_invited`/`accepts_a2a`; purge invited on disconnect. `deliver_message`: record human invitation; consent-gate agent→agent; add `a2a_consent_notice`.
- `packages/connector/src/protocol/types.ts` — `AINMAgentConfig.acceptsA2A?: boolean`.
- `packages/connector/src/protocol/handshake.ts` (+ `agent.ts` if the card object is assembled there) — carry `acceptsA2A` in the handshake card object.
- `server/tests/a2a_consent_e2e.rs` — black-box: peer denied when target not human-invited; allowed after human invites both; `acceptsA2A:false` blocks. DB-or-skip.

---

## Task 1: Pure `consent_ok` predicate

**Files:**
- Modify: `server/src/routing.rs`

- [ ] **Step 1: Append `consent_ok` + tests**

In `server/src/routing.rs`, add this function ABOVE the `#[cfg(test)] mod tests` block (top-level item):

```rust
/// Pure A2A consent predicate (P2). An agent→agent message may be delivered only when:
/// 1. the SENDER agent was human-invited into the thread,
/// 2. the TARGET agent was human-invited into the thread, AND
/// 3. the target has not opted out of peer messages (`acceptsA2A != false`).
///
/// Human-invited membership is established elsewhere (a human message targeting the agent);
/// this predicate is pure over the three resolved booleans.
pub fn consent_ok(
    from_is_human_invited: bool,
    to_is_human_invited: bool,
    to_accepts_a2a: bool,
) -> bool {
    from_is_human_invited && to_is_human_invited && to_accepts_a2a
}
```

Add inside `mod tests`:

```rust
    #[test]
    fn consent_requires_both_invited_and_accept() {
        assert!(consent_ok(true, true, true));
    }

    #[test]
    fn consent_denied_when_sender_not_invited() {
        assert!(!consent_ok(false, true, true));
    }

    #[test]
    fn consent_denied_when_target_not_invited() {
        assert!(!consent_ok(true, false, true));
    }

    #[test]
    fn consent_denied_when_target_opts_out() {
        assert!(!consent_ok(true, true, false));
    }
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ainm-server routing::tests`
Expected: PASS (P1's 10 + 4 new = 14).

- [ ] **Step 3: Commit**

```bash
rtk proxy git add server/src/routing.rs
rtk proxy git commit -m "feat(a2a-p2): pure consent_ok predicate (P2 Task 1)"
```

---

## Task 2: `acceptsA2A` in Agent Card builders

**Files:**
- Modify: `server/src/agent_card.rs`

- [ ] **Step 1: Add `acceptsA2A: true` to the assistant card**

In `build_assistant_card` (`agent_card.rs:14`), find this EXACT block inside the `ainm` object:

```rust
            "memory": {
                "archivalSupported": true,
                "contextFramePreferred": true
            },
            "tools": []
```

Replace it with (adds `acceptsA2A` sibling — hosted assistant accepts peer messages by default):

```rust
            "memory": {
                "archivalSupported": true,
                "contextFramePreferred": true
            },
            "acceptsA2A": true,
            "tools": []
```

- [ ] **Step 2: Add `acceptsA2A` to the connector card (read from `init_params.card`, default true)**

In `build_connector_card` (`agent_card.rs:59`), find this EXACT block (after `let version = ...;`, before `let streaming = ...;`):

```rust
    let streaming = caps
        .and_then(|c| c.get("streaming"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
```

Insert immediately ABOVE it:

```rust
    // P2: a connector may opt out of peer (agent→agent) messages via `card.acceptsA2A: false`.
    // Absent/non-bool defaults to true (cooperative-by-default).
    let accepts_a2a = card
        .and_then(|c| c.get("acceptsA2A"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
```

Then find this EXACT block (the connector card's `ainm` object tail):

```rust
            "memory": { "archivalSupported": false, "contextFramePreferred": false },
            "tools": tools
```

Replace it with:

```rust
            "memory": { "archivalSupported": false, "contextFramePreferred": false },
            "acceptsA2A": accepts_a2a,
            "tools": tools
```

- [ ] **Step 3: Add tests**

In `agent_card.rs` `mod tests`, add:

```rust
    #[test]
    fn assistant_card_accepts_a2a_by_default() {
        let card = build_assistant_card("assistant@localhost");
        assert_eq!(card["ainm"]["acceptsA2A"], true);
    }

    #[test]
    fn connector_card_accepts_a2a_defaults_true_when_absent() {
        let params = json!({ "card": { "name": "Alpha" }, "capabilities": {} });
        let card = build_connector_card("alpha@localhost", &params);
        assert_eq!(card["ainm"]["acceptsA2A"], true);
    }

    #[test]
    fn connector_card_honors_accepts_a2a_false() {
        let params = json!({ "card": { "name": "Beta", "acceptsA2A": false }, "capabilities": {} });
        let card = build_connector_card("beta@localhost", &params);
        assert_eq!(card["ainm"]["acceptsA2A"], false);
    }
```

(If `json!` is not already imported in the test module, the file already does `use serde_json::{json, Value};` at the top — confirm tests can call `json!`; existing connector-card tests already use it, so it is in scope.)

- [ ] **Step 4: Run tests**

Run: `cargo test -p ainm-server agent_card`
Expected: PASS (existing card tests + 3 new).

- [ ] **Step 5: Commit**

```bash
rtk proxy git add server/src/agent_card.rs
rtk proxy git commit -m "feat(a2a-p2): acceptsA2A on assistant + connector cards (P2 Task 2)"
```

---

## Task 3: `AgentRegistry` human-invited membership + `accepts_a2a` accessor

**Files:**
- Modify: `server/src/server.rs` — `AgentRegistry` struct, `new`, `cleanup`, and the methods block.

- [ ] **Step 1: Add `thread_invited` to the struct**

Find this EXACT block (the `AgentRegistry` struct, post-P1):

```rust
#[derive(Clone)]
pub struct AgentRegistry {
    connections: Arc<Mutex<HashMap<String, Vec<tokio::sync::mpsc::UnboundedSender<String>>>>>,
    agent_cards: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    /// thread_id → set of agent identities participating in that thread (runtime A2A).
    thread_agents: Arc<Mutex<HashMap<String, std::collections::HashSet<String>>>>,
}
```

Replace with:

```rust
#[derive(Clone)]
pub struct AgentRegistry {
    connections: Arc<Mutex<HashMap<String, Vec<tokio::sync::mpsc::UnboundedSender<String>>>>>,
    agent_cards: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    /// thread_id → set of agent identities participating in that thread (runtime A2A).
    thread_agents: Arc<Mutex<HashMap<String, std::collections::HashSet<String>>>>,
    /// thread_id → set of agent identities a HUMAN has invited into that thread (P2 consent).
    /// Distinct from `thread_agents`: invitation requires a human message to have targeted the
    /// agent, so consent is meaningful (an agent cannot self-admit by being peer-addressed).
    thread_invited: Arc<Mutex<HashMap<String, std::collections::HashSet<String>>>>,
}
```

- [ ] **Step 2: Initialize it in `new`**

Find the `AgentRegistry::new` body (post-P1):

```rust
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            agent_cards: Arc::new(Mutex::new(HashMap::new())),
            thread_agents: Arc::new(Mutex::new(HashMap::new())),
        }
    }
```

Replace with:

```rust
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            agent_cards: Arc::new(Mutex::new(HashMap::new())),
            thread_agents: Arc::new(Mutex::new(HashMap::new())),
            thread_invited: Arc::new(Mutex::new(HashMap::new())),
        }
    }
```

- [ ] **Step 3: Add `invite_to_thread`, `is_invited`, `accepts_a2a`**

Find the P1 method `leave_all_threads` (it ends the `thread_agents` accessor group):

```rust
    /// Remove an agent identity from every thread's membership set (disconnect cleanup).
    pub async fn leave_all_threads(&self, identity: &str) {
        let mut map = self.thread_agents.lock().await;
        map.retain(|_, members| {
            members.remove(identity);
            !members.is_empty()
        });
    }
```

Insert immediately AFTER it:

```rust
    /// Record that a HUMAN invited an agent into a thread (P2 consent membership).
    pub async fn invite_to_thread(&self, identity: &str, thread_id: &str) {
        let mut map = self.thread_invited.lock().await;
        map.entry(thread_id.to_string())
            .or_default()
            .insert(identity.to_string());
    }

    /// True when `identity` was human-invited into `thread_id` (P2 consent membership).
    pub async fn is_invited(&self, identity: &str, thread_id: &str) -> bool {
        let map = self.thread_invited.lock().await;
        map.get(thread_id).map(|s| s.contains(identity)).unwrap_or(false)
    }

    /// Whether a connected agent's published card permits peer (A2A) messages.
    /// Reads `ainm.acceptsA2A` from the cached card; absent/non-bool defaults to true.
    pub async fn accepts_a2a(&self, identity: &str) -> bool {
        let cards = self.agent_cards.lock().await;
        cards
            .get(identity)
            .and_then(|c| c.get("ainm"))
            .and_then(|a| a.get("acceptsA2A"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }
```

- [ ] **Step 4: Purge invited membership on disconnect**

Find this EXACT block in `AgentRegistry::cleanup` (post-P1):

```rust
        if now_empty {
            // No connections remain: drop the cached card so the well-known route 404s,
            // and remove this agent from every thread membership set (runtime A2A).
            self.agent_cards.lock().await.remove(identity);
            self.leave_all_threads(identity).await;
        }
    }
```

Replace with:

```rust
        if now_empty {
            // No connections remain: drop the cached card so the well-known route 404s,
            // and remove this agent from every thread membership/invitation set (runtime A2A).
            self.agent_cards.lock().await.remove(identity);
            self.leave_all_threads(identity).await;
            let mut invited = self.thread_invited.lock().await;
            invited.retain(|_, members| {
                members.remove(identity);
                !members.is_empty()
            });
        }
    }
```

- [ ] **Step 5: Build**

Run: `cargo build -p ainm-server`
Expected: builds. `invite_to_thread`/`is_invited`/`accepts_a2a` will warn as unused until Task 4 — expected; do NOT `#[allow]`.

- [ ] **Step 6: Commit**

```bash
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "feat(a2a-p2): AgentRegistry human-invited membership + accepts_a2a (P2 Task 3)"
```

---

## Task 4: Wire consent into `deliver_message`

**Files:**
- Modify: `server/src/server.rs` — `a2a_limit_notice` neighborhood (add `a2a_consent_notice`) + `deliver_message`.

- [ ] **Step 1: Add the consent-denied notice helper**

Find the END of `a2a_limit_notice` (it returns `serde_json::to_string(&deliver).unwrap()` and closes with `}`). Immediately AFTER `a2a_limit_notice`'s closing brace, insert:

```rust
/// Build the system notice delivered to humans when an agent→agent message is blocked by consent
/// (target not human-invited, sender not human-invited, or the target opted out via `acceptsA2A`).
/// Visible, never a silent drop (P2 success criterion).
fn a2a_consent_notice(thread_id: &str, target: &str) -> String {
    let text = format!(
        "An agent tried to message {target}, but that agent isn't part of this conversation yet. Add it to allow agent-to-agent messages."
    );
    let deliver = protocol::new_notification(
        "ainm.message.deliver",
        serde_json::json!({
            "message": {
                "id": protocol::new_message_id(),
                "from": "system@local",
                "thread_id": thread_id,
                "timestamp": protocol::now_rfc3339(),
                "type": "message",
                "content": [{
                    "type": "text",
                    "version": "1.0",
                    "source": "system",
                    "text": text,
                    "fallback": text,
                }],
            },
            "verified": false,
            "sender_type": "system",
        }),
    );
    serde_json::to_string(&deliver).unwrap()
}
```

- [ ] **Step 2: Record human invitation in `deliver_message`**

In `deliver_message`, find this EXACT block (post-P1):

```rust
    let target_is_hosted = hosted_addr.as_deref() == Some(t.as_str());
    let target_is_connected_agent = state.agent_registry.is_connected(&t).await;
    let target_is_agent = target_is_hosted || target_is_connected_agent;
```

Replace it with (adds the human-invitation recording):

```rust
    let target_is_hosted = hosted_addr.as_deref() == Some(t.as_str());
    let target_is_connected_agent = state.agent_registry.is_connected(&t).await;
    let target_is_agent = target_is_hosted || target_is_connected_agent;

    // P2 consent: a human addressing an agent INVITES it into the thread (human curates membership).
    if !sender_is_agent && target_is_agent && !thread_id.is_empty() {
        state.agent_registry.invite_to_thread(&t, thread_id).await;
    }
```

- [ ] **Step 3: Gate agent→agent on consent after the hop guard**

In `deliver_message`, find this EXACT block (post-P1 — the hop-guard tail just before the delivery match):

```rust
    if decision.tripped {
        let notice = a2a_limit_notice(thread_id, state.a2a_hop_cap);
        state.human_registry.send_to_thread(thread_id, &notice).await;
        return;
    }
    if !decision.deliver_to_target {
        return;
    }

    if target_is_hosted {
```

Replace it with (inserts the consent gate between the hop guard and delivery):

```rust
    if decision.tripped {
        let notice = a2a_limit_notice(thread_id, state.a2a_hop_cap);
        state.human_registry.send_to_thread(thread_id, &notice).await;
        return;
    }
    if !decision.deliver_to_target {
        return;
    }

    // P2 consent gate (agent→agent only): both endpoints must be human-invited, and the
    // target must not have opted out via `acceptsA2A`. Denials are visible to humans.
    if sender_is_agent && target_is_agent {
        let from_invited = state.agent_registry.is_invited(from, thread_id).await;
        let to_invited = state.agent_registry.is_invited(&t, thread_id).await;
        let to_accepts = if target_is_hosted {
            true
        } else {
            state.agent_registry.accepts_a2a(&t).await
        };
        if !crate::routing::consent_ok(from_invited, to_invited, to_accepts) {
            let notice = a2a_consent_notice(thread_id, &t);
            state.human_registry.send_to_thread(thread_id, &notice).await;
            return;
        }
    }

    if target_is_hosted {
```

- [ ] **Step 4: Build**

Run: `cargo build -p ainm-server`
Expected: clean build; the Task 3 methods are now used (no more unused warnings for them).

- [ ] **Step 5: Run the P1 regression + routing tests**

Run: `cargo test -p ainm-server routing`
Expected: PASS (14 routing tests).
Run: `cargo test -p ainm-server --test ws_conversation_e2e`
Expected: PASS or skip-no-db. (Human→hosted-assistant is NOT consent-gated — sender is human — so the hosted flow is unaffected.)

- [ ] **Step 6: Commit**

```bash
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "feat(a2a-p2): consent-gate agent→agent in deliver_message + notice (P2 Task 4)"
```

---

## Task 5: Connector SDK — `acceptsA2A` config + handshake

**Files:**
- Modify: `packages/connector/src/protocol/types.ts` (`AINMAgentConfig`).
- Modify: `packages/connector/src/protocol/handshake.ts` (and/or `packages/connector/src/agent.ts` where the handshake `card` object is assembled — locate first).

- [ ] **Step 1: Add the config flag**

In `packages/connector/src/protocol/types.ts`, find the `AINMAgentConfig` interface. After the `version?: string;` line, add:

```ts
  /** Opt out of agent-to-agent (peer) messages. Defaults to true (cooperative). */
  acceptsA2A?: boolean;
```

- [ ] **Step 2: Carry it in the handshake card object**

Locate where the handshake `card` object is built. Run `rtk proxy grep -rn "card" packages/connector/src/protocol/handshake.ts packages/connector/src/agent.ts | grep -i "name\|description\|skills\|card:"`. The connector sends a `card` object inside `ainm.initialize` params containing `name`/`description`/`version`/`skills`/`tools`. Add `acceptsA2A` to that object.

In `packages/connector/src/agent.ts`, the handshake is invoked via `performHandshake(this.transport, { identity, name, description, version, capabilities, skills, tools, keypair })` (around `agent.ts:89`). Find the object literal passed to `performHandshake` and add a field:

```ts
      acceptsA2A: this.config.acceptsA2A,
```

Then in `packages/connector/src/protocol/handshake.ts`, find where the `card` object is assembled in the initialize params (it already includes `name`, `description`, `version`, `skills`, `tools`). Add to that card object:

```ts
      acceptsA2A: opts.acceptsA2A ?? true,
```

and add `acceptsA2A?: boolean;` to the handshake options type (the `performHandshake` params interface in `handshake.ts`). Match the exact existing field names/shape — read the file first and mirror how `description`/`version` flow from `agent.ts` → `performHandshake` → the card object. The server (`build_connector_card`, Task 2) reads `card.acceptsA2A`, defaulting true if absent, so a connector that doesn't set it is unaffected.

- [ ] **Step 3: Build + test**

Run (from `packages/connector`): `npm run build && npm test`
Expected: PASS. If a handshake test asserts the exact card object shape, update it to include `acceptsA2A`. Report any test touched.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add packages/connector/src/protocol/types.ts packages/connector/src/protocol/handshake.ts packages/connector/src/agent.ts
rtk proxy git commit -m "feat(a2a-p2): connector acceptsA2A config + handshake (P2 Task 5)"
```

---

## Task 6: Black-box WS integration test — consent

**Files:**
- Create: `server/tests/a2a_consent_e2e.rs`

Reuses the helper shapes from `server/tests/a2a_routing_e2e.rs` (copy them — integration tests are standalone). DB-or-skip. Asserts: (a) agent→agent is DENIED when the target was not human-invited (human gets a consent notice; target gets nothing); (b) after the human invites BOTH agents, agent→agent is DELIVERED; (c) a target with `acceptsA2A:false` is DENIED even when both are invited.

- [ ] **Step 1: Write the test file**

Create `server/tests/a2a_consent_e2e.rs`. Copy the helpers (`pool_and_url_or_skip`, `make_bound_account`, `cleanup_domain`, `free_port`, `next_text`, `WsSink`/`WsStream` aliases, `connect_and_handshake`, `send_addressed`, `await_deliver_from`) VERBATIM from `server/tests/a2a_routing_e2e.rs`, then add a second variant of `connect_and_handshake` that includes a `card` with `acceptsA2A` so an agent can opt out. Concretely, after copying the helpers, add:

```rust
/// Handshake as an agent whose handshake card sets `acceptsA2A`.
async fn connect_agent_with_a2a(
    ws_url: &str,
    id: &identity::Identity,
    accepts_a2a: bool,
) -> (WsSink, WsStream) {
    let (ws_stream, _) = tokio::time::timeout(Duration::from_secs(5),
        tokio_tungstenite::connect_async(ws_url))
        .await.expect("ws connect timeout").expect("ws connect");
    let (mut tx, mut rx) = ws_stream.split();
    let (sig, ts) = signing::sign_init(&id.key_pair, &id.address).unwrap();
    let init_req = protocol::new_request(
        "ainm.initialize",
        serde_json::json!({
            "identity": id.address,
            "protocolVersion": "0.1.0",
            "signature": sig,
            "timestamp": ts,
            "capabilities": { "streaming": true, "contentTypes": ["text"], "participant_type": "agent" },
            "card": { "name": id.address.split('@').next().unwrap(), "acceptsA2A": accepts_a2a },
        }),
        serde_json::json!(2),
    );
    tx.send(Message::Text(serde_json::to_string(&init_req).unwrap().into())).await.unwrap();
    let resp = next_text(&mut rx, Duration::from_secs(10)).await.expect("init response");
    let resp_json: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert!(resp_json.get("result").is_some(), "handshake rejected: {resp}");
    let initialized = protocol::new_notification("ainm.initialized", serde_json::json!({}));
    tx.send(Message::Text(serde_json::to_string(&initialized).unwrap().into())).await.unwrap();
    (tx, rx)
}
```

Then add the test:

```rust
#[tokio::test]
async fn agent_to_agent_requires_human_invitation_and_card_consent() {
    let Some((pool, db_url)) = pool_and_url_or_skip().await else {
        eprintln!("no DB; skipping agent_to_agent_requires_human_invitation_and_card_consent");
        return;
    };

    let run = format!("{:x}", uuid::Uuid::new_v4().as_u128());
    let domain = format!("a2ac-{run}.local");
    let alice = make_bound_account(&pool, &format!("alice@{domain}"), &domain).await;
    let agent_a = make_bound_account(&pool, &format!("alpha@{domain}"), &domain).await;
    let agent_b = make_bound_account(&pool, &format!("beta@{domain}"), &domain).await;
    let agent_c = make_bound_account(&pool, &format!("gamma@{domain}"), &domain).await;

    let port = free_port().await;
    let certs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("certs");
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_ainm-server"))
        .args([
            "--mode", "human", "--port", &port.to_string(), "--identity", &domain,
            "--cert", certs.join("server-a.pem").to_str().unwrap(),
            "--key", certs.join("server-a-key.pem").to_str().unwrap(),
            "--ca-cert", certs.join("ca.pem").to_str().unwrap(),
        ])
        .env("AINM_DATABASE__URL", &db_url)
        .env("AINM_A2A__HOP_CAP", "10")
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null()).stderr(Stdio::piped()).kill_on_drop(true)
        .spawn().expect("spawn ainm-server");

    let http = reqwest::Client::new();
    let ready_url = format!("http://127.0.0.1:{port}/health/ready");
    let mut ready = false;
    for _ in 0..150 {
        if let Ok(r) = http.get(&ready_url).send().await { if r.status().is_success() { ready = true; break; } }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "server did not become ready");

    let ws_url = format!("ws://127.0.0.1:{port}/ws");
    let (mut human_tx, mut human_rx) = connect_and_handshake(&ws_url, &alice, "human").await;
    let (mut a_tx, mut a_rx) = connect_agent_with_a2a(&ws_url, &agent_a, true).await;
    let (mut _b_tx, mut b_rx) = connect_agent_with_a2a(&ws_url, &agent_b, true).await;
    let (mut _c_tx, mut c_rx) = connect_agent_with_a2a(&ws_url, &agent_c, false).await; // opts out

    let thread_id = protocol::new_message_id();

    // Human invites only A (human→A). B and C are NOT yet invited.
    send_addressed(&mut human_tx, &alice, &thread_id, &agent_a.address, "start @alpha").await;
    await_deliver_from(&mut a_rx, &alice.address, Duration::from_secs(10)).await
        .expect("A receives the human invite");

    // (1) A→B is DENIED — B was never human-invited. B must NOT receive it; human gets a notice.
    send_addressed(&mut a_tx, &agent_a, &thread_id, &agent_b.address, "hi @beta").await;
    let b_blocked = await_deliver_from(&mut b_rx, &agent_a.address, Duration::from_secs(3)).await;
    assert!(b_blocked.is_none(), "B should NOT receive a peer message before being human-invited, got: {b_blocked:?}");
    let notice1 = await_deliver_from(&mut human_rx, "system@local", Duration::from_secs(5)).await;
    assert!(notice1.is_some(), "human should get a consent notice for the blocked A→B");

    // (2) Human invites B (human→B). Now A→B is ALLOWED.
    send_addressed(&mut human_tx, &alice, &thread_id, &agent_b.address, "join us @beta").await;
    await_deliver_from(&mut b_rx, &alice.address, Duration::from_secs(10)).await
        .expect("B receives the human invite");
    send_addressed(&mut a_tx, &agent_a, &thread_id, &agent_b.address, "now hi @beta").await;
    let b_got = await_deliver_from(&mut b_rx, &agent_a.address, Duration::from_secs(10)).await
        .expect("B should receive A's peer message after being human-invited");
    assert_eq!(b_got.1, "agent");

    // (3) Human invites C, but C opted out (acceptsA2A:false) → A→C still DENIED.
    send_addressed(&mut human_tx, &alice, &thread_id, &agent_c.address, "hello @gamma").await;
    await_deliver_from(&mut c_rx, &alice.address, Duration::from_secs(10)).await
        .expect("C receives the human invite");
    send_addressed(&mut a_tx, &agent_a, &thread_id, &agent_c.address, "hi @gamma").await;
    let c_blocked = await_deliver_from(&mut c_rx, &agent_a.address, Duration::from_secs(3)).await;
    assert!(c_blocked.is_none(), "C opted out of A2A; must NOT receive a peer message, got: {c_blocked:?}");

    let _ = child.kill().await;
    cleanup_domain(&pool, &domain).await;
}
```

- [ ] **Step 2: Verify compile (and run if DB present)**

Run: `cargo test -p ainm-server --test a2a_consent_e2e --no-run`
Expected: compiles clean. Fix only compile issues by comparing to `a2a_routing_e2e.rs`. Then run `cargo test -p ainm-server --test a2a_consent_e2e` — passes against a DB, or prints the skip line + passes without one. Report which.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add server/tests/a2a_consent_e2e.rs
rtk proxy git commit -m "test(a2a-p2): consent — human-invitation gating + card opt-out (P2 Task 6)"
```

---

## Task 7: Full-suite verification + clippy + DoD

**Files:** none (verification only).

- [ ] **Step 1: Server build, clippy (P1-parity scope), tests**

Run: `cargo build -p ainm-server` → clean.
Run: `cargo clippy -p ainm-server --all-targets 2>&1 | grep -E "routing\.rs|agent_card\.rs|a2a_consent_e2e|server\.rs"` and confirm NO new lints in P2-touched files (the codebase has pre-existing lints in other files — `compiler.rs`/`graph.rs`/`repository.rs`/`briefing.rs`/`persistence.rs` — which are NOT P2's concern; do not "fix" them).
Run: `cargo test -p ainm-server` → PASS except the two long-standing pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`). Confirm those are the ONLY failures.

- [ ] **Step 2: Connector SDK**

Run (from `packages/connector`): `npm run build && npm test` → PASS.

- [ ] **Step 3: DoD audit (P2 scope)**

- [ ] `consent_ok` pure + unit-tested (Task 1).
- [ ] `acceptsA2A` on both card builders, default true, connector-overridable (Task 2).
- [ ] Human-invited membership tracked separately from connection-membership; purged on disconnect (Task 3).
- [ ] `deliver_message`: human→agent records invitation; agent→agent gated on `consent_ok(from_invited, to_invited, to_accepts)`; denial emits a VISIBLE system notice, never a silent drop (Task 4).
- [ ] Connector can opt out via config → handshake → card (Task 5).
- [ ] Integration test covers: denied-before-invite, allowed-after-both-invited, card-opt-out-blocks (Task 6).
- [ ] Human→agent and human→hosted-assistant are NOT consent-gated (regression-free; `ws_conversation_e2e` green/skip).
- [ ] No iOS changes.
- [ ] Spec criterion 2 holds: a non-member or opted-out agent is never invoked; the human sees a notice.

- [ ] **Step 4: Final commit (if fixes were made)**

```bash
rtk proxy git add -A
rtk proxy git commit -m "chore(a2a-p2): full-suite green + DoD audit (P2 Task 7)"
```

---

## Notes & Risks

- **Invitation provenance is the crux:** `thread_invited` is populated ONLY on a human→agent message (`deliver_message`, sender_type=="human", target is an agent). An agent being peer-addressed does NOT invite it (that is the whole point). Verify in Task 4 that the invitation write is guarded by `!sender_is_agent`.
- **Ordering:** hop guard runs before consent (per spec §3.5). A consent-denied A2A attempt still counts toward the hop cap — harmless (only tightens the cap), and the denied message never reaches the target so no reply is generated. Acceptable for P2.
- **Hosted assistant** is treated as `acceptsA2A: true` and is invited when a human addresses it; it can be a consent-checked *target*. It still cannot *initiate* A2A (P1 follow-up, out of scope).
- **In-memory state:** `thread_invited` is in-memory like the P1 hop counter and `thread_agents` — lost on restart, consistent with the rest of the runtime routing state. Persistence is a federation-era concern (P3+).
- **Connector handshake shape:** Task 5 must mirror the EXACT existing `card`-object plumbing (`agent.ts` → `performHandshake` → `handshake.ts`). Read those files before editing; the server already defaults `acceptsA2A` to true, so this task is purely the opt-out affordance.
```
