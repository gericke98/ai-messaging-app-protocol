# Runtime A2A — P1 (Same-Server Core) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make two same-server agents able to address each other: promote the envelope `to` field, server-stamp `sender_type`, replace the blind agent broadcast with targeted thread-scoped delivery, bound agent→agent loops with a human-resettable hop cap, and teach the connector SDK who addressed it.

**Architecture:** A new pure `routing.rs` module (recipient resolution + hop-counter state machine) carries the correctness weight under exhaustive unit tests. A single `deliver_message` orchestrator in `server.rs` replaces the inline `to_agent_tx` broadcast in **both** connection handlers; it stamps `sender_type`, shows agent-originated messages to humans, resolves the target, applies the hop guard, and delivers to the addressed connected agent (activating the dead `AgentRegistry::send_to_agent`) or the hosted assistant. Agent thread-membership and a per-thread hop counter live on shared state. The connector SDK surfaces `senderType`/`to`/`addressed`/`verified` and lets a reply set `to`. No iOS changes — SP3/SP4 already render this.

**Tech Stack:** Rust (Axum/Tokio) server at `server/src`; cargo + nextest; black-box WS integration tests against the compiled binary (DB-or-skip). TypeScript connector SDK at `packages/connector` (+ `connector-openai`, `connector-claude-code`), built/tested with its existing npm scripts.

**Standing constraints:**
- All git via `rtk proxy git` (NEVER bare `git`). Commit directly to `main` (no branches).
- Do NOT commit Xcode auto-rewrites of `*.pbxproj`/`*.xcscheme` — N/A here (no iOS), but if any appear, `rtk proxy git checkout --` them.
- Pure-logic-first testing. No iOS changes in P1.
- Never call the product a "demo".

**P1/P2 boundary (important):** P1 *tracks* agent thread-membership (needed for mention resolution and for P2) and delivers to an addressed, connected agent. The `acceptsA2A` Agent Card opt-out and the full `consent_ok` predicate (non-member rejection) are **P2** — do NOT implement them here. In P1, an addressed connected agent is delivered to (subject only to the hop guard).

**Backward-compat note (deliberate):** The blind broadcast is removed for the *resolved-target* path. A single legacy fallback remains: a **human** sender whose message resolves to NO target (no `to`, no matching mention) still reaches connected agents via `to_agent_tx` — this preserves today's 1:1 connector flow for clients that don't set `to`. Agent senders never trigger this fallback (no new broadcast on the A2A path). This is called out again in the relevant task and in §Success Criteria.

---

## File Structure

**Server (Rust):**
- `server/src/routing.rs` — **NEW.** Pure: `first_mention_localpart`, `resolve_recipient`, `a2a_hop_decision` + `HopDecision`. Fully unit-tested. No I/O.
- `server/migrations/007_message_to_addr.sql` — **NEW.** Adds `messages.to_addr TEXT NULL`.
- `server/src/config.rs` — add `A2aConfig { hop_cap }` (env `AINM_A2A__HOP_CAP`, default 6).
- `server/src/server.rs` — register `mod routing` via lib root (see Task 1 note); add agent thread-membership + accessors to `AgentRegistry`; add `participants_of` to `HumanRegistry`; add `a2a_hops` + `a2a_hop_cap` to `ServerState`; add `deliver_message` + `thread_participants` + `a2a_limit_notice`; rewire both handlers; extend `store_and_maybe_compile` to persist `to_addr` + `signature`.
- `server/src/lib.rs` (or wherever `mod` declarations live) — declare `pub mod routing;` if modules are declared there. (Verify: `rtk proxy grep -n "mod protocol" server/src/lib.rs server/src/main.rs`.)
- `server/tests/a2a_routing_e2e.rs` — **NEW.** Black-box WS: two agent clients, agent→agent delivery, hop-cap trip + notice, human reset. DB-or-skip.

**Connector SDK (TypeScript):**
- `packages/connector/src/protocol/types.ts` — `IncomingMessage` gains `senderType`/`to`/`addressed`/`verified`; `MessageContext.sendMessage` gains optional `{ to?: string }`.
- `packages/connector/src/agent.ts` — populate the new fields in `handleMessageDelivery`; thread `to` through `sendMessage` and the auto-send-on-return path.
- `packages/connector-openai/src/openai-agent.ts`, `packages/connector-claude-code/src/claude-code-agent.ts` — adopt the `addressed` gate; echo peer `to` when the sender was an agent.

---

## Build & Test Commands (reference)

- Server unit tests for the new module:
  `cargo test -p ainm-server routing`  (or `cargo nextest run -p ainm-server routing` if nextest is installed)
- Server full lib/bin tests: `cargo test -p ainm-server`
- Clippy: `cargo clippy -p ainm-server --all-targets -- -D warnings`
- The black-box WS test needs Postgres; provide `TEST_DATABASE_URL` (or `AINM_DATABASE__URL`). Without it the test self-skips.
- Connector SDK: from `packages/connector`, `npm run build && npm test`. Reference connectors: `npm run build` in each package.

---

## Task 1: Pure routing module — recipient resolution

**Files:**
- Create: `server/src/routing.rs`
- Modify: module declaration root — run `rtk proxy grep -n "pub mod protocol\|mod protocol" server/src/lib.rs server/src/main.rs` to find where modules are declared, then add `pub mod routing;` next to `pub mod protocol;`.

- [ ] **Step 1: Create the module with `first_mention_localpart` + `resolve_recipient` and their failing tests**

Create `server/src/routing.rs`:

```rust
//! Pure routing helpers for runtime A2A (P1, same-server core).
//!
//! No I/O, no registries, no async — just the decision logic that says *who* a
//! message is addressed to and *whether* an agent→agent hop may proceed. Kept
//! pure so it is exhaustively unit-testable; the orchestrator in `server.rs`
//! supplies the live participant list / counters.

/// Local-part (before `@`) of an address. `"a@b"` → `"a"`; `"a"` → `"a"`.
fn local_part(addr: &str) -> &str {
    addr.split('@').next().unwrap_or(addr)
}

/// Extract the local-part of the FIRST `@mention` in `text`, if any.
///
/// A mention is `@` immediately followed by 1+ chars of `[A-Za-z0-9._-]`. The
/// returned string excludes the leading `@` (e.g. `"hey @analyst go"` → `Some("analyst")`).
/// `@` with no valid char after it is ignored.
pub fn first_mention_localpart(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'@' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() {
                let c = bytes[j];
                if c.is_ascii_alphanumeric() || c == b'.' || c == b'_' || c == b'-' {
                    j += 1;
                } else {
                    break;
                }
            }
            if j > start {
                return Some(text[start..j].to_string());
            }
        }
        i += 1;
    }
    None
}

/// Resolve the routing target for a message.
///
/// 1. An explicit non-empty `to` wins verbatim.
/// 2. Otherwise the first `@mention` whose local-part matches a known
///    participant's local-part resolves to that participant's full address.
/// 3. Otherwise `None` (no agent target; humans observe).
pub fn resolve_recipient(
    to: Option<&str>,
    content_text: &str,
    known_participants: &[String],
) -> Option<String> {
    if let Some(t) = to {
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    let mention = first_mention_localpart(content_text)?;
    known_participants
        .iter()
        .find(|p| local_part(p) == mention)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_mention_extracts_localpart() {
        assert_eq!(first_mention_localpart("hey @analyst please"), Some("analyst".to_string()));
        assert_eq!(first_mention_localpart("@bob"), Some("bob".to_string()));
        assert_eq!(first_mention_localpart("a.b_c-d @x.y_z-1 end"), Some("x.y_z-1".to_string()));
    }

    #[test]
    fn first_mention_none_when_absent_or_bare() {
        assert_eq!(first_mention_localpart("no mention here"), None);
        assert_eq!(first_mention_localpart("trailing @"), None);
        assert_eq!(first_mention_localpart("email like a@b is still a mention"), Some("a".to_string()));
    }

    #[test]
    fn resolve_prefers_explicit_to() {
        let parts = vec!["analyst@acme".to_string(), "writer@acme".to_string()];
        assert_eq!(
            resolve_recipient(Some("writer@acme"), "ignored @analyst", &parts),
            Some("writer@acme".to_string())
        );
    }

    #[test]
    fn resolve_empty_to_falls_through_to_mention() {
        let parts = vec!["analyst@acme".to_string()];
        assert_eq!(
            resolve_recipient(Some(""), "ping @analyst", &parts),
            Some("analyst@acme".to_string())
        );
    }

    #[test]
    fn resolve_mention_matches_localpart_to_full_address() {
        let parts = vec!["analyst@acme.test".to_string(), "alice@localhost".to_string()];
        assert_eq!(
            resolve_recipient(None, "could @analyst weigh in", &parts),
            Some("analyst@acme.test".to_string())
        );
    }

    #[test]
    fn resolve_none_when_no_to_and_mention_unknown() {
        let parts = vec!["analyst@acme".to_string()];
        assert_eq!(resolve_recipient(None, "hi @ghost", &parts), None);
        assert_eq!(resolve_recipient(None, "plain text", &parts), None);
        assert_eq!(resolve_recipient(None, "plain text", &[]), None);
    }
}
```

Add the module declaration found in Step's Modify note, e.g. in `server/src/lib.rs`:

```rust
pub mod routing;
```

- [ ] **Step 2: Run the tests to verify they pass (module compiles, logic correct)**

Run: `cargo test -p ainm-server routing::tests`
Expected: PASS (6 tests). If `cargo test` reports `routing` is not a module, the `pub mod routing;` declaration is in the wrong file — put it where `pub mod protocol;` is.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add server/src/routing.rs server/src/lib.rs
rtk proxy git commit -m "feat(a2a): pure resolve_recipient + mention parsing (P1 Task 1)"
```

---

## Task 2: Pure routing module — A2A hop-counter state machine

**Files:**
- Modify: `server/src/routing.rs`

- [ ] **Step 1: Append `HopDecision` + `a2a_hop_decision` and failing tests**

Append to `server/src/routing.rs` (above the `#[cfg(test)]` block, i.e. add the type + fn before `mod tests`, and add the test fns inside `mod tests`):

```rust
/// Outcome of one A2A hop-counter transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HopDecision {
    /// New consecutive-A2A-hop count to store for the thread (0 = cleared).
    pub new_count: u32,
    /// Whether the target agent should be invoked. `false` = circuit tripped.
    pub deliver_to_target: bool,
    /// Whether THIS message tripped the cap (used to emit the one-shot notice).
    pub tripped: bool,
}

/// Pure A2A hop-counter transition for one inbound message in a thread.
///
/// `current` is the stored count of consecutive agent→agent hops with no human
/// message between, for this thread. `cap` is the configured maximum.
///
/// Rules:
/// - Non-agent (human) sender: resets the counter to 0 and always delivers.
/// - Agent sender, non-agent target (agent→human / no agent target): counter
///   unchanged, always delivers.
/// - Agent sender, agent target (the A2A case): if `current >= cap`, do NOT
///   deliver and report `tripped`; otherwise increment and deliver.
pub fn a2a_hop_decision(
    current: u32,
    sender_is_agent: bool,
    target_is_agent: bool,
    cap: u32,
) -> HopDecision {
    if !sender_is_agent {
        return HopDecision { new_count: 0, deliver_to_target: true, tripped: false };
    }
    if !target_is_agent {
        return HopDecision { new_count: current, deliver_to_target: true, tripped: false };
    }
    if current >= cap {
        return HopDecision { new_count: current, deliver_to_target: false, tripped: true };
    }
    HopDecision { new_count: current + 1, deliver_to_target: true, tripped: false }
}
```

Add inside `mod tests`:

```rust
    #[test]
    fn human_sender_resets_and_delivers() {
        let d = a2a_hop_decision(5, false, true, 6);
        assert_eq!(d, HopDecision { new_count: 0, deliver_to_target: true, tripped: false });
        // even targeting a human
        let d2 = a2a_hop_decision(3, false, false, 6);
        assert_eq!(d2.new_count, 0);
        assert!(d2.deliver_to_target);
    }

    #[test]
    fn agent_to_human_leaves_counter_unchanged() {
        let d = a2a_hop_decision(4, true, false, 6);
        assert_eq!(d, HopDecision { new_count: 4, deliver_to_target: true, tripped: false });
    }

    #[test]
    fn agent_to_agent_increments_until_cap() {
        let d = a2a_hop_decision(0, true, true, 6);
        assert_eq!(d, HopDecision { new_count: 1, deliver_to_target: true, tripped: false });
        let d5 = a2a_hop_decision(5, true, true, 6);
        assert_eq!(d5, HopDecision { new_count: 6, deliver_to_target: true, tripped: false });
    }

    #[test]
    fn agent_to_agent_trips_at_cap() {
        let d = a2a_hop_decision(6, true, true, 6);
        assert_eq!(d, HopDecision { new_count: 6, deliver_to_target: false, tripped: true });
        // and stays tripped above cap
        let d2 = a2a_hop_decision(9, true, true, 6);
        assert!(!d2.deliver_to_target);
        assert!(d2.tripped);
    }
```

- [ ] **Step 2: Run the tests to verify they pass**

Run: `cargo test -p ainm-server routing::tests`
Expected: PASS (10 tests total now).

- [ ] **Step 3: Commit**

```bash
rtk proxy git add server/src/routing.rs
rtk proxy git commit -m "feat(a2a): pure a2a_hop_decision state machine (P1 Task 2)"
```

---

## Task 3: Migration + persist `to_addr` and `signature`

**Files:**
- Create: `server/migrations/007_message_to_addr.sql`
- Modify: `server/src/server.rs` — `store_and_maybe_compile` (currently `server.rs:1256`) and its two call sites (human `server.rs:2560`, agent `server.rs:1828`).

- [ ] **Step 1: Create the migration**

Create `server/migrations/007_message_to_addr.sql`:

```sql
-- Runtime A2A (P1): record the resolved recipient on each stored message so
-- threads replay with directionality (mirrors the iOS `toAddress`, SP3 migration v9).
ALTER TABLE messages ADD COLUMN IF NOT EXISTS to_addr TEXT NULL;
```

- [ ] **Step 2: Extend `store_and_maybe_compile` to accept and persist `to_addr` + `signature`**

In `server/src/server.rs`, change the function signature (currently `server.rs:1256-1263`) from:

```rust
async fn store_and_maybe_compile(
    db: &sqlx::PgPool,
    compile_tx: &Option<tokio::sync::mpsc::Sender<crate::compiler::CompileRequest>>,
    thread_id: &str,
    from_addr: &str,
    content: &serde_json::Value,
    trigger_compilation: bool,
) {
```

to:

```rust
async fn store_and_maybe_compile(
    db: &sqlx::PgPool,
    compile_tx: &Option<tokio::sync::mpsc::Sender<crate::compiler::CompileRequest>>,
    thread_id: &str,
    from_addr: &str,
    to_addr: Option<&str>,
    signature: Option<&str>,
    content: &serde_json::Value,
    trigger_compilation: bool,
) {
```

And change the INSERT (currently `server.rs:1285-1294`) from:

```rust
    let _ = sqlx::query(
        "INSERT INTO messages (thread_id, sequence, from_addr, content) \
         VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
    )
    .bind(thread_id)
    .bind(seq)
    .bind(from_addr)
    .bind(content)
    .execute(db)
    .await;
```

to:

```rust
    let _ = sqlx::query(
        "INSERT INTO messages (thread_id, sequence, from_addr, to_addr, content, signature) \
         VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING",
    )
    .bind(thread_id)
    .bind(seq)
    .bind(from_addr)
    .bind(to_addr)
    .bind(content)
    .bind(signature)
    .execute(db)
    .await;
```

- [ ] **Step 3: Update the human call site** (currently `server.rs:2560-2568`). Replace:

```rust
                                store_and_maybe_compile(
                                    db,
                                    &state.compile_tx,
                                    thread_id,
                                    from,
                                    &content,
                                    false, // don't trigger compilation on human messages
                                )
                                .await;
```

with:

```rust
                                let to_addr = params.get("to").and_then(|t| t.as_str());
                                let signature = params.get("signature").and_then(|s| s.as_str());
                                store_and_maybe_compile(
                                    db,
                                    &state.compile_tx,
                                    thread_id,
                                    from,
                                    to_addr,
                                    signature,
                                    &content,
                                    false, // don't trigger compilation on human messages
                                )
                                .await;
```

- [ ] **Step 4: Update the agent call site** (currently `server.rs:1828-1836`). Replace:

```rust
                                store_and_maybe_compile(
                                    db,
                                    &state.compile_tx,
                                    thread_id,
                                    from,
                                    &content,
                                    true, // trigger compilation on agent responses
                                )
                                .await;
```

with:

```rust
                                let to_addr = params.get("to").and_then(|t| t.as_str());
                                let signature = params.get("signature").and_then(|s| s.as_str());
                                store_and_maybe_compile(
                                    db,
                                    &state.compile_tx,
                                    thread_id,
                                    from,
                                    to_addr,
                                    signature,
                                    &content,
                                    true, // trigger compilation on agent responses
                                )
                                .await;
```

- [ ] **Step 5: Compile**

Run: `cargo build -p ainm-server`
Expected: builds clean. (No new test here — exercised by the integration test in Task 9 and the existing `ws_conversation_e2e` which still passes a `to`.)

- [ ] **Step 6: Commit**

```bash
rtk proxy git add server/migrations/007_message_to_addr.sql server/src/server.rs
rtk proxy git commit -m "feat(a2a): persist messages.to_addr + signature (P1 Task 3)"
```

---

## Task 4: Agent thread-membership on `AgentRegistry`

**Files:**
- Modify: `server/src/server.rs` — `AgentRegistry` struct (`server.rs:147-151`), `new` (`server.rs:154-159`), `cleanup` (`server.rs:176-199`); activate `send_to_agent` (`server.rs:216-225`, remove its `#[allow(dead_code)]`).

- [ ] **Step 1: Add a `thread_agents` map to the struct**

Replace the struct (currently `server.rs:147-151`):

```rust
#[derive(Clone)]
pub struct AgentRegistry {
    connections: Arc<Mutex<HashMap<String, Vec<tokio::sync::mpsc::UnboundedSender<String>>>>>,
    agent_cards: Arc<Mutex<HashMap<String, serde_json::Value>>>,
}
```

with:

```rust
#[derive(Clone)]
pub struct AgentRegistry {
    connections: Arc<Mutex<HashMap<String, Vec<tokio::sync::mpsc::UnboundedSender<String>>>>>,
    agent_cards: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    /// thread_id → set of agent identities participating in that thread (runtime A2A).
    thread_agents: Arc<Mutex<HashMap<String, std::collections::HashSet<String>>>>,
}
```

Replace `new` (currently `server.rs:154-159`):

```rust
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            agent_cards: Arc::new(Mutex::new(HashMap::new())),
        }
    }
```

with:

```rust
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            agent_cards: Arc::new(Mutex::new(HashMap::new())),
            thread_agents: Arc::new(Mutex::new(HashMap::new())),
        }
    }
```

- [ ] **Step 2: Add membership accessors + `is_connected`; activate `send_to_agent`**

Replace `send_to_agent` (currently `server.rs:216-225`):

```rust
    /// Send a message to a specific agent.
    #[allow(dead_code)]
    pub async fn send_to_agent(&self, identity: &str, msg: &str) {
        let conns = self.connections.lock().await;
        if let Some(senders) = conns.get(identity) {
            for tx in senders {
                let _ = tx.send(msg.to_string());
            }
        }
    }
```

with (note: `#[allow(dead_code)]` removed — it is now live):

```rust
    /// Send a message to a specific connected agent (runtime A2A targeted delivery).
    pub async fn send_to_agent(&self, identity: &str, msg: &str) {
        let conns = self.connections.lock().await;
        if let Some(senders) = conns.get(identity) {
            for tx in senders {
                let _ = tx.send(msg.to_string());
            }
        }
    }

    /// True when `identity` has at least one live connection.
    pub async fn is_connected(&self, identity: &str) -> bool {
        let conns = self.connections.lock().await;
        conns.get(identity).map(|s| !s.is_empty()).unwrap_or(false)
    }

    /// Record that an agent participates in a thread (runtime A2A membership).
    pub async fn join_thread(&self, identity: &str, thread_id: &str) {
        let mut map = self.thread_agents.lock().await;
        map.entry(thread_id.to_string())
            .or_default()
            .insert(identity.to_string());
    }

    /// Agent identities participating in a thread.
    pub async fn participants_of(&self, thread_id: &str) -> Vec<String> {
        let map = self.thread_agents.lock().await;
        map.get(thread_id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Remove an agent identity from every thread's membership set (disconnect cleanup).
    pub async fn leave_all_threads(&self, identity: &str) {
        let mut map = self.thread_agents.lock().await;
        map.retain(|_, members| {
            members.remove(identity);
            !members.is_empty()
        });
    }
```

- [ ] **Step 3: Drop membership on disconnect.** In `cleanup` (currently `server.rs:176-199`), after the `if now_empty { ... }` block that removes the cached card, add a membership purge. Replace the tail of `cleanup`:

```rust
        if now_empty {
            // No connections remain: drop the cached card so the well-known route 404s.
            self.agent_cards.lock().await.remove(identity);
        }
    }
```

with:

```rust
        if now_empty {
            // No connections remain: drop the cached card so the well-known route 404s,
            // and remove this agent from every thread membership set (runtime A2A).
            self.agent_cards.lock().await.remove(identity);
            self.leave_all_threads(identity).await;
        }
    }
```

- [ ] **Step 4: Add `participants_of` to `HumanRegistry`.** After `HumanRegistry::join_thread` (currently ends `server.rs:99`), add:

```rust
    /// User addresses participating in a thread.
    pub async fn participants_of(&self, thread_id: &str) -> Vec<String> {
        let map = self.thread_participants.lock().await;
        map.get(thread_id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }
```

- [ ] **Step 5: Compile**

Run: `cargo build -p ainm-server`
Expected: builds clean. `send_to_agent`/`is_connected`/`join_thread`/`participants_of`/`leave_all_threads` may warn as unused until Task 6/7 wire them — that's expected this task; do NOT add `#[allow(dead_code)]` (they get used in Task 6/7). If the unused warnings block a `-D warnings` clippy, defer clippy to Task 10.

- [ ] **Step 6: Commit**

```bash
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "feat(a2a): agent thread-membership + send_to_agent activation (P1 Task 4)"
```

---

## Task 5: Config `A2aConfig` + `ServerState` hop state

**Files:**
- Modify: `server/src/config.rs` (struct `AppConfig` `config.rs:10-21`).
- Modify: `server/src/server.rs` — `ServerState` struct (starts `server.rs:240`; read the whole struct first with `rtk proxy grep -n "pub struct ServerState" server/src/server.rs` then read to its closing brace) and ALL its construction sites.

- [ ] **Step 1: Add `A2aConfig` to config**

In `server/src/config.rs`, add a field to `AppConfig` (after the `quota` field, `config.rs:19-20`):

```rust
    #[serde(default)]
    pub a2a: A2aConfig,
```

And add the type + default (place near `QuotaConfig`, after `config.rs:43`):

```rust
/// Runtime A2A (agent-to-agent) policy knobs.
#[derive(Debug, Deserialize, Clone)]
pub struct A2aConfig {
    /// Max consecutive agent→agent hops in a thread with no human between.
    /// Loaded from `AINM_A2A__HOP_CAP`. Default 6.
    #[serde(default = "default_a2a_hop_cap")]
    pub hop_cap: u32,
}
impl Default for A2aConfig {
    fn default() -> Self {
        Self { hop_cap: default_a2a_hop_cap() }
    }
}
fn default_a2a_hop_cap() -> u32 { 6 }
```

- [ ] **Step 2: Add a config test**

In `config.rs` `mod tests`, add:

```rust
    #[test]
    fn a2a_hop_cap_defaults_to_six() {
        let cfg = A2aConfig::default();
        assert_eq!(cfg.hop_cap, 6);
    }

    #[test]
    fn a2a_hop_cap_parses_from_toml() {
        let cfg: A2aConfig = toml::from_str("hop_cap = 3").unwrap();
        assert_eq!(cfg.hop_cap, 3);
    }
```

Run: `cargo test -p ainm-server config::tests`
Expected: PASS (existing 2 + new 2).

- [ ] **Step 3: Add hop state to `ServerState`**

First read the full struct: `rtk proxy grep -n "pub struct ServerState" server/src/server.rs` and read from there to the closing `}`. Add two fields to `ServerState` (place after the `agent_registry` field, `server.rs:249`):

```rust
    /// Per-thread consecutive-A2A-hop counter (runtime A2A loop guard).
    pub a2a_hops: Arc<Mutex<HashMap<String, u32>>>,
    /// Max consecutive agent→agent hops per thread before the circuit trips.
    pub a2a_hop_cap: u32,
```

- [ ] **Step 4: Update every `ServerState { ... }` construction site**

Find them: `rtk proxy grep -n "ServerState {" server/src/*.rs`. For each constructor, add:

```rust
        a2a_hops: Arc::new(Mutex::new(HashMap::new())),
        a2a_hop_cap: /* see below */,
```

For the production constructor (in `main.rs`, where `config` / `AppConfig` is in scope — it already wires `quota`), use:

```rust
        a2a_hop_cap: config.a2a.hop_cap,
```

For any constructor that does NOT have an `AppConfig` in scope (e.g. a test/in-memory builder, like the one used by `--mode` without DB), use the literal default:

```rust
        a2a_hop_cap: 6,
```

(If you are unsure whether a given site has `config` in scope, the compiler error will tell you; use `6` where `config` is unavailable.)

- [ ] **Step 5: Compile**

Run: `cargo build -p ainm-server`
Expected: builds clean once all `ServerState { .. }` sites have the two new fields.

- [ ] **Step 6: Commit**

```bash
rtk proxy git add server/src/config.rs server/src/server.rs server/src/main.rs
rtk proxy git commit -m "feat(a2a): A2aConfig hop_cap + ServerState hop counter (P1 Task 5)"
```

---

## Task 6: `deliver_message` orchestrator + `thread_participants` + notice helper

**Files:**
- Modify: `server/src/server.rs` — add three free functions near `store_and_maybe_compile` (after it, around `server.rs:1359`). These are the heart of P1; the existing pure helpers `content_to_plain` (`server.rs:28`) and `content_mentions` (`server.rs:23`) are reused.

- [ ] **Step 1: Add `thread_participants`, `a2a_limit_notice`, and `deliver_message`**

Insert after `store_and_maybe_compile` ends (after `server.rs:1359`):

```rust
/// Union of known participant addresses in a thread (humans + agents + the
/// hosted assistant if enabled). Used only for `@mention` resolution; an explicit
/// `to` does not need this list.
async fn thread_participants(state: &ServerState, thread_id: &str) -> Vec<String> {
    let mut v = state.human_registry.participants_of(thread_id).await;
    v.extend(state.agent_registry.participants_of(thread_id).await);
    if let Some(agent) = state.hosted_agent.as_ref() {
        if agent.enabled {
            v.push(agent.address.clone());
        }
    }
    v
}

/// Build the one-shot "agents paused" system notice delivered to humans when the
/// A2A hop cap trips. It is a normal `ainm.message.deliver` with a system sender so
/// the existing client renders it inline (no new client surface — P1 constraint).
fn a2a_limit_notice(thread_id: &str, cap: u32) -> String {
    let text = format!(
        "Agents paused after {cap} consecutive exchanges — reply to continue."
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

/// Local delivery orchestrator for runtime A2A (P1, same-server core).
///
/// Replaces the blind `to_agent_tx` broadcast. Given an inbound message envelope
/// (`params`) and the server-classified `sender_type` ("human" | "agent"):
///   - agent-originated messages are shown to humans in the thread (A2A display copy);
///   - the routing target is resolved (`to` → mention → none);
///   - the A2A hop guard is applied (human-resettable, capped); on trip a visible
///     system notice goes to humans and the target is NOT invoked;
///   - a connected agent target is delivered to via `AgentRegistry::send_to_agent`;
///   - the hosted assistant target spawns `runtime_server::handle_turn`;
///   - a HUMAN sender whose message resolves to no reachable agent falls back to the
///     legacy `to_agent_tx` broadcast (preserves 1:1 connector flow for clients that
///     do not set `to`). Agent senders never trigger this fallback.
async fn deliver_message(
    state: &ServerState,
    params: &serde_json::Value,
    sender_type: &str,
    verified: bool,
) {
    let thread_id = params.get("thread_id").and_then(|t| t.as_str()).unwrap_or("");
    let from = params.get("from").and_then(|f| f.as_str()).unwrap_or("");
    let to_field = params.get("to").and_then(|t| t.as_str());
    let content = params.get("content").cloned().unwrap_or(serde_json::json!([]));

    let sender_is_agent = sender_type == "agent";

    // Stamp sender_type onto the deliver wrapper (server-authoritative; never from payload).
    let deliver = protocol::new_notification(
        "ainm.message.deliver",
        serde_json::json!({
            "message": params,
            "verified": verified,
            "sender_type": sender_type,
        }),
    );
    let deliver_text = serde_json::to_string(&deliver).unwrap();

    // Agent-originated messages are always shown to humans (this is what SP3/SP4 render).
    if sender_is_agent {
        state.human_registry.send_to_thread(thread_id, &deliver_text).await;
    }

    // Record the sender's thread membership (agents only; humans are tracked elsewhere).
    if sender_is_agent && !from.is_empty() && !thread_id.is_empty() {
        state.agent_registry.join_thread(from, thread_id).await;
    }

    // Resolve the target.
    let participants = thread_participants(state, thread_id).await;
    let content_text = content_to_plain(&content);
    let target = crate::routing::resolve_recipient(to_field, &content_text, &participants);

    let hosted_addr = state
        .hosted_agent
        .as_ref()
        .filter(|a| a.enabled)
        .map(|a| a.address.clone());

    let Some(t) = target else {
        // No target. Legacy fallback only for human senders (1:1 connector reach).
        if !sender_is_agent {
            let _ = state.to_agent_tx.send(deliver_text);
        }
        return;
    };

    let target_is_hosted = hosted_addr.as_deref() == Some(t.as_str());
    let target_is_connected_agent = state.agent_registry.is_connected(&t).await;
    let target_is_agent = target_is_hosted || target_is_connected_agent;

    // Hop guard (pure decision over the stored per-thread counter).
    let current = { *state.a2a_hops.lock().await.get(thread_id).unwrap_or(&0) };
    let decision =
        crate::routing::a2a_hop_decision(current, sender_is_agent, target_is_agent, state.a2a_hop_cap);
    {
        let mut hops = state.a2a_hops.lock().await;
        if decision.new_count == 0 {
            hops.remove(thread_id);
        } else {
            hops.insert(thread_id.to_string(), decision.new_count);
        }
    }
    if decision.tripped {
        let notice = a2a_limit_notice(thread_id, state.a2a_hop_cap);
        state.human_registry.send_to_thread(thread_id, &notice).await;
        return;
    }
    if !decision.deliver_to_target {
        return;
    }

    if target_is_hosted {
        state.agent_registry.join_thread(&t, thread_id).await;
        let state2 = state.clone();
        let tid = thread_id.to_string();
        let requester = from.to_string();
        let user_text = content_text.clone();
        state.shutdown.spawn_turn(async move {
            crate::runtime_server::handle_turn(&state2, &tid, &requester, &user_text).await;
        });
    } else if target_is_connected_agent {
        state.agent_registry.join_thread(&t, thread_id).await;
        state.agent_registry.send_to_agent(&t, &deliver_text).await;
    } else if !sender_is_agent {
        // Target named but not a reachable agent; human sender → legacy fallback.
        let _ = state.to_agent_tx.send(deliver_text);
    }
}
```

- [ ] **Step 2: Compile**

Run: `cargo build -p ainm-server`
Expected: builds clean. `deliver_message` will warn as unused until Task 7 wires it — expected; do NOT silence it.

> Note: confirm the hosted-agent field/type. The trigger code at `server.rs:2574-2596` uses `state.hosted_agent.clone()`, `agent.enabled`, `agent.address`, and `state.shutdown.spawn_turn(...)`. The code above mirrors those exact accessors. If `hosted_agent`'s `address`/`enabled` are named differently, match the names used at `server.rs:2574-2596`.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "feat(a2a): deliver_message orchestrator + hop guard + notice (P1 Task 6)"
```

---

## Task 7: Wire `deliver_message` into both handlers (kill the broadcast)

**Files:**
- Modify: `server/src/server.rs` — human handler `ainm.message.send` local-delivery (`server.rs:2466-2534`) and hosted-trigger block (`server.rs:2571-2599`); agent handler local-delivery (`server.rs:1768-1799`); agent handler forwarder (`server.rs:1706-1723`).

- [ ] **Step 1: Consume the per-agent registry channel in the agent handler**

The agent's `AgentRegistry` mpsc receiver (`from_registry_rx`, `server.rs:1706`) is currently never forwarded — only the `to_agent_tx` broadcast is. Targeted `send_to_agent` writes to that mpsc channel, so the agent must forward it to its socket. Replace the forwarder block (currently `server.rs:1706-1723`):

```rust
    let from_registry_rx = state.agent_registry.register(&from_addr, &agent_card).await;
    tracing::info!("Agent registered: {}", from_addr);

    let mut from_human_rx = state.to_agent_tx.subscribe();
    let human_registry = state.human_registry.clone();
    let identity_store = state.identity_store.clone();
    let archival_store = state.archival_store.clone();
    let ws_sender_clone = ws_sender.clone();

    // Task: forward human messages to agent WebSocket
    let forward_handle = tokio::spawn(async move {
        while let Ok(msg) = from_human_rx.recv().await {
            let mut sender = ws_sender_clone.lock().await;
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });
```

with (adds a second forwarder draining the per-agent targeted channel):

```rust
    let mut from_registry_rx = state.agent_registry.register(&from_addr, &agent_card).await;
    tracing::info!("Agent registered: {}", from_addr);

    let mut from_human_rx = state.to_agent_tx.subscribe();
    let human_registry = state.human_registry.clone();
    let identity_store = state.identity_store.clone();
    let archival_store = state.archival_store.clone();
    let ws_sender_clone = ws_sender.clone();
    let ws_sender_targeted = ws_sender.clone();

    // Task: forward broadcast (legacy / human-fallback) messages to agent WebSocket.
    let forward_handle = tokio::spawn(async move {
        while let Ok(msg) = from_human_rx.recv().await {
            let mut sender = ws_sender_clone.lock().await;
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Task: forward TARGETED messages (runtime A2A `send_to_agent`) to agent WebSocket.
    let targeted_forward_handle = tokio::spawn(async move {
        while let Some(msg) = from_registry_rx.recv().await {
            let mut sender = ws_sender_targeted.lock().await;
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });
```

Then find where `forward_handle` is awaited/aborted at the end of `handle_agent_connection` (search `rtk proxy grep -n "forward_handle" server/src/server.rs`). Wherever `forward_handle.abort()` (or `.await`) appears in the agent handler's teardown, add the same for `targeted_forward_handle`. For example if teardown reads:

```rust
    forward_handle.abort();
```

change it to:

```rust
    forward_handle.abort();
    targeted_forward_handle.abort();
```

(If teardown uses `let _ = forward_handle.await;` instead, mirror that form for `targeted_forward_handle`.)

- [ ] **Step 2: Rewire the AGENT handler local delivery to `deliver_message`**

Replace the agent handler's local-delivery branches (currently `server.rs:1768-1799`):

```rust
                            // Federation-aware routing for agent responses
                            if let Some(ref fed) = state.federation {
                                let origin = federation::parse_origin_domain(thread_id)
                                    .unwrap_or("");

                                if origin != fed.server_id && !origin.is_empty() {
                                    // Thread originated on remote server: relay response back
                                    fed.relay_sender.enqueue(origin, params.clone());
                                } else {
                                    // Local thread: deliver to thread participants
                                    let deliver = protocol::new_notification(
                                        "ainm.message.deliver",
                                        serde_json::json!({
                                            "message": params,
                                            "verified": verified,
                                        }),
                                    );
                                    let deliver_text = serde_json::to_string(&deliver).unwrap();
                                    human_registry.send_to_thread(thread_id, &deliver_text).await;
                                }
                            } else {
                                // Legacy mode
                                let deliver = protocol::new_notification(
                                    "ainm.message.deliver",
                                    serde_json::json!({
                                        "message": params,
                                        "verified": verified,
                                    }),
                                );
                                let deliver_text = serde_json::to_string(&deliver).unwrap();
                                human_registry.send_to_thread(thread_id, &deliver_text).await;
                            }
```

with:

```rust
                            // Federation-aware routing for agent responses
                            if let Some(ref fed) = state.federation {
                                let origin = federation::parse_origin_domain(thread_id)
                                    .unwrap_or("");

                                if origin != fed.server_id && !origin.is_empty() {
                                    // Thread originated on remote server: relay response back (P3 generalizes this).
                                    fed.relay_sender.enqueue(origin, params.clone());
                                } else {
                                    // Local thread: targeted delivery (humans + addressed agent / hosted).
                                    deliver_message(&state, params, "agent", verified).await;
                                }
                            } else {
                                // Legacy mode: targeted local delivery.
                                deliver_message(&state, params, "agent", verified).await;
                            }
```

> The `human_registry` local clone (`server.rs:1710`) may now be unused in the agent handler. If the compiler flags it, prefix it `let _human_registry = ...` or remove the binding. Do the minimal change the compiler asks for.

- [ ] **Step 3: Rewire the HUMAN handler local delivery to `deliver_message`**

In the human handler, replace the **legacy** (non-federation) delivery block (currently `server.rs:2520-2534`):

```rust
                            } else {
                                // Legacy mode: deliver locally
                                let deliver = protocol::new_notification(
                                    "ainm.message.deliver",
                                    serde_json::json!({
                                        "message": params,
                                        "verified": verified,
                                    }),
                                );
                                let deliver_text = serde_json::to_string(&deliver).unwrap();
                                match to_agent_tx.send(deliver_text) {
                                    Ok(n) => tracing::info!("[routing] Delivered to {n} agent subscriber(s)"),
                                    Err(e) => tracing::warn!("[routing] No agent subscribers: {e}"),
                                }
                            }
```

with:

```rust
                            } else {
                                // Legacy mode: targeted local delivery (humans-fallback broadcast inside).
                                deliver_message(&state, params, "human", verified).await;
                            }
```

And inside the **federation** `origin == fed.server_id || origin.is_empty()` branch, replace the local agent broadcast (currently `server.rs:2484-2488`):

```rust
                                    let deliver_text = serde_json::to_string(&deliver).unwrap();
                                    // Deliver locally (to agent if connected via WS)
                                    match to_agent_tx.send(deliver_text.clone()) {
                                        Ok(n) => tracing::info!("[routing] Delivered to {n} agent subscriber(s) (federated)"),
                                        Err(e) => tracing::warn!("[routing] No agent subscribers (federated): {e}"),
                                    }
                                    // Also deliver via federation local_delivery_tx
                                    let _ = fed.local_delivery_tx.send(deliver_text);
```

with (keep the federation `local_delivery_tx` channel; replace only the agent broadcast with targeted delivery):

```rust
                                    let deliver_text = serde_json::to_string(&deliver).unwrap();
                                    // Targeted local delivery (humans-fallback broadcast inside).
                                    deliver_message(&state, params, "human", verified).await;
                                    // Also deliver via federation local_delivery_tx (unchanged).
                                    let _ = fed.local_delivery_tx.send(deliver_text);
```

> The `deliver`/`deliver_text` binding just above that block (federation path, `server.rs:2476-2483`) is still used by `fed.local_delivery_tx.send(deliver_text)`, so keep it. If the compiler reports `deliver_text` unused in some path, keep the one feeding `local_delivery_tx`.

- [ ] **Step 4: Remove the now-redundant hosted-agent trigger block** (folded into `deliver_message`). Delete the entire block (currently `server.rs:2571-2599`):

```rust
                            // Hosted agent: if provisioned + enabled and this thread/message
                            // targets it, run the loop in the background (streams its reply
                            // back to thread participants).
                            if let Some(agent) = state.hosted_agent.clone() {
                                if agent.enabled {
                                    let content = params
                                        .get("content")
                                        .cloned()
                                        .unwrap_or(serde_json::json!([]));
                                    let targets_agent = msg_thread_id.contains("assistant@")
                                        || params.get("to").and_then(|t| t.as_str())
                                            .map(|t| t == agent.address).unwrap_or(false)
                                        || content_mentions(&content, &agent.address);
                                    tracing::debug!(
                                        "[agent-trigger] thread={msg_thread_id} to={:?} agent={} targets_agent={targets_agent}",
                                        params.get("to").and_then(|t| t.as_str()),
                                        agent.address,
                                    );
                                    if targets_agent {
                                        let state2 = state.clone();
                                        let tid = msg_thread_id.to_string();
                                        let requester = from.to_string();
                                        let user_text = content_to_plain(&content);
                                        state.shutdown.spawn_turn(async move {
                                            crate::runtime_server::handle_turn(&state2, &tid, &requester, &user_text).await;
                                        });
                                    }
                                }
                            }
```

> **Behavior parity check:** the deleted block triggered the hosted agent on `thread.contains("assistant@")` OR `to == agent.address` OR mention. `deliver_message` triggers it when `resolve_recipient` returns the hosted address — i.e. on explicit `to == hosted` or an `@mention` matching the hosted local-part. The `assistant@`-in-thread-id heuristic is intentionally dropped (the spec makes `to` canonical; the iOS assistant thread and `ws_conversation_e2e` both set `to == assistant@<domain>`, so the hosted path stays covered). If `content_mentions` (`server.rs:23`) becomes unused after this deletion, the compiler will warn — leave it if still referenced elsewhere, otherwise prefix with `#[allow(dead_code)]` or remove. Prefer removal only if truly unused.

- [ ] **Step 5: Build + run the existing e2e to confirm no regression**

Run: `cargo build -p ainm-server`
Expected: clean build.

Run (needs DB; self-skips otherwise): `cargo test -p ainm-server --test ws_conversation_e2e`
Expected: PASS — `alice` (human) sends with `to == assistant@<domain>`; `deliver_message` resolves the hosted target and spawns `handle_turn`; the signed reply is delivered to the thread. If it now fails, the hosted-trigger fold or the human-handler rewire is wrong — compare the resolved target against `state.hosted_agent.address`.

- [ ] **Step 6: Commit**

```bash
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "feat(a2a): route both handlers through deliver_message; remove broadcast (P1 Task 7)"
```

---

## Task 8: Black-box WS integration test — agent→agent + hop cap + reset

**Files:**
- Create: `server/tests/a2a_routing_e2e.rs`

This test acts as TWO agent WebSocket clients (`participant_type: "agent"`) plus one human client in a single thread, against the compiled binary. It reuses the helper shapes from `server/tests/ws_conversation_e2e.rs` (copy them — integration tests are standalone). DB-or-skip.

- [ ] **Step 1: Write the test file**

Create `server/tests/a2a_routing_e2e.rs`:

```rust
//! Runtime A2A (P1) black-box test: two agents in one thread address each other,
//! the hop cap trips after N consecutive agent→agent hops, a system notice is
//! delivered to the human, and a human message resets the circuit. DB-or-skip.

use ainm_server::{identity, protocol, signing};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use std::process::Stdio;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

async fn pool_and_url_or_skip() -> Option<(sqlx::PgPool, String)> {
    let url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("AINM_DATABASE__URL"))
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()?;
    let pool = sqlx::PgPool::connect(&url).await.ok()?;
    let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    sqlx::migrate::Migrator::new(migrations_dir).await.ok()?.run(&pool).await.ok()?;
    Some((pool, url))
}

async fn make_bound_account(pool: &sqlx::PgPool, address: &str, domain: &str) -> identity::Identity {
    let username = address.split('@').next().unwrap();
    let account_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (username, domain, address, email_hash, email_verified, status) \
         VALUES ($1, $2, $3, $4, true, 'active') RETURNING id",
    )
    .bind(username).bind(domain).bind(address).bind(format!("hash-{address}"))
    .fetch_one(pool).await.expect("insert account");
    let id = identity::generate_keypair(address).expect("keypair");
    let pk_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&id.public_key_bytes);
    sqlx::query("INSERT INTO account_keys (account_id, public_key) VALUES ($1, $2)")
        .bind(account_id).bind(&pk_b64).execute(pool).await.expect("insert key");
    id
}

async fn cleanup_domain(pool: &sqlx::PgPool, domain: &str) {
    let _ = sqlx::query("DELETE FROM hosted_agents WHERE address LIKE $1")
        .bind(format!("%@{domain}")).execute(pool).await;
    let _ = sqlx::query("DELETE FROM accounts WHERE domain = $1").bind(domain).execute(pool).await;
}

async fn free_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

async fn next_text(
    rx: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
    timeout: Duration,
) -> Option<String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.checked_duration_since(tokio::time::Instant::now())?;
        match tokio::time::timeout(remaining, rx.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => return Some(t.to_string()),
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => return None,
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(_))) => return None,
            Err(_) => return None,
        }
    }
}

type WsSink = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;
type WsStream = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
>;

/// Connect + signed handshake. `participant_type` is "human" or "agent".
async fn connect_and_handshake(
    ws_url: &str,
    id: &identity::Identity,
    participant_type: &str,
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
            "capabilities": {
                "streaming": true,
                "contentTypes": ["text"],
                "participant_type": participant_type,
            }
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

/// Build, sign, and send an `ainm.message.send` with an explicit `to`.
async fn send_addressed(tx: &mut WsSink, sender: &identity::Identity, thread_id: &str, to: &str, text: &str) {
    let mut envelope = serde_json::json!({
        "id": protocol::new_message_id(),
        "from": sender.address,
        "thread_id": thread_id,
        "timestamp": protocol::now_rfc3339(),
        "type": "message",
        "content": [{
            "type": "text", "version": "1.0", "source": "agent",
            "text": text, "fallback": text,
        }],
        "to": to,
    });
    signing::sign_envelope(&sender.key_pair, &mut envelope).expect("sign");
    let req = protocol::new_request("ainm.message.send", envelope, serde_json::json!(protocol::new_message_id()));
    tx.send(Message::Text(serde_json::to_string(&req).unwrap().into())).await.unwrap();
}

/// Read frames until an `ainm.message.deliver` whose inner message.from == `from` arrives.
/// Returns (text, sender_type). None on timeout.
async fn await_deliver_from(rx: &mut WsStream, from: &str, timeout: Duration) -> Option<(String, String)> {
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        let remaining = deadline.checked_duration_since(tokio::time::Instant::now()).unwrap_or(Duration::ZERO);
        let frame = next_text(rx, remaining).await?;
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&frame) else { continue };
        if v.get("method").and_then(|m| m.as_str()) != Some("ainm.message.deliver") { continue; }
        let params = v.get("params");
        let msg = params.and_then(|p| p.get("message"));
        let msg_from = msg.and_then(|m| m.get("from")).and_then(|f| f.as_str()).unwrap_or("");
        if msg_from != from { continue; }
        let sender_type = params.and_then(|p| p.get("sender_type")).and_then(|s| s.as_str()).unwrap_or("").to_string();
        let text = msg.and_then(|m| m.get("content")).and_then(|c| c.as_array())
            .and_then(|a| a.first()).and_then(|b| b.get("text")).and_then(|t| t.as_str())
            .unwrap_or("").to_string();
        return Some((text, sender_type));
    }
    None
}

#[tokio::test]
async fn agent_to_agent_routes_and_hop_cap_trips() {
    let Some((pool, db_url)) = pool_and_url_or_skip().await else {
        eprintln!("no DB; skipping agent_to_agent_routes_and_hop_cap_trips");
        return;
    };

    let run = format!("{:x}", uuid::Uuid::new_v4().as_u128());
    let domain = format!("a2a-{run}.local");
    let alice = make_bound_account(&pool, &format!("alice@{domain}"), &domain).await;
    let agent_a = make_bound_account(&pool, &format!("alpha@{domain}"), &domain).await;
    let agent_b = make_bound_account(&pool, &format!("beta@{domain}"), &domain).await;

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
        .env("AINM_A2A__HOP_CAP", "2")        // small cap so the test is fast/deterministic
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null()).stderr(Stdio::piped()).kill_on_drop(true)
        .spawn().expect("spawn ainm-server");

    // Wait for readiness.
    let http = reqwest::Client::new();
    let ready_url = format!("http://127.0.0.1:{port}/health/ready");
    let mut ready = false;
    for _ in 0..150 {
        if let Ok(r) = http.get(&ready_url).send().await { if r.status().is_success() { ready = true; break; } }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "server did not become ready");

    let ws_url = format!("ws://127.0.0.1:{port}/ws");
    let (mut human_tx, mut _human_rx) = connect_and_handshake(&ws_url, &alice, "human").await;
    let (mut a_tx, mut a_rx) = connect_and_handshake(&ws_url, &agent_a, "agent").await;
    let (mut _b_tx, mut b_rx) = connect_and_handshake(&ws_url, &agent_b, "agent").await;

    let thread_id = protocol::new_message_id();

    // The human first addresses agent A so the thread + A's membership are established,
    // and so A is a known thread participant. (Human → agent resets/zeros the hop counter.)
    send_addressed(&mut human_tx, &alice, &thread_id, &agent_a.address, "kick off please @alpha").await;
    // Agent A receives it (sender_type human).
    let got = await_deliver_from(&mut a_rx, &alice.address, Duration::from_secs(10)).await
        .expect("agent A should receive the human kickoff");
    assert_eq!(got.1, "human", "kickoff sender_type should be human");

    // Now agent A addresses agent B: A→B is hop 1 (cap=2) → delivered.
    send_addressed(&mut a_tx, &agent_a, &thread_id, &agent_b.address, "hi @beta, hop 1").await;
    let b_got = await_deliver_from(&mut b_rx, &agent_a.address, Duration::from_secs(10)).await
        .expect("agent B should receive A's first A2A message");
    assert_eq!(b_got.1, "agent", "A→B sender_type should be agent");
    assert!(b_got.0.contains("hop 1"));

    // B→A is hop 2 → delivered.
    send_addressed(&mut _b_tx, &agent_b, &thread_id, &agent_a.address, "reply @alpha, hop 2").await;
    let a_got = await_deliver_from(&mut a_rx, &agent_b.address, Duration::from_secs(10)).await
        .expect("agent A should receive B's reply (hop 2)");
    assert!(a_got.0.contains("hop 2"));

    // A→B again would be hop 3 > cap(2): NOT delivered to B; human gets a system notice instead.
    send_addressed(&mut a_tx, &agent_a, &thread_id, &agent_b.address, "hop 3 should be blocked @beta").await;

    // B must NOT receive "hop 3" within a short window.
    let blocked = await_deliver_from(&mut b_rx, &agent_a.address, Duration::from_secs(3)).await;
    assert!(
        blocked.as_ref().map(|(t, _)| t.contains("hop 3")).unwrap_or(false) == false,
        "agent B should NOT receive the over-cap hop-3 message, got: {blocked:?}"
    );

    // The human should receive the system notice.
    let notice = await_deliver_from(&mut _human_rx, "system@local", Duration::from_secs(5)).await;
    assert!(notice.is_some(), "human should receive the a2a-paused system notice");
    assert_eq!(notice.as_ref().unwrap().1, "system");
    assert!(notice.unwrap().0.to_lowercase().contains("paused"));

    let _ = child.kill().await;
    cleanup_domain(&pool, &domain).await;
}
```

- [ ] **Step 2: Run the test**

Run (needs DB): `cargo test -p ainm-server --test a2a_routing_e2e`
Expected: PASS when a DB is available; `skipping ...` printed (and pass) when not.

If it hangs at `await_deliver_from` for B's first message: the targeted forwarder (Task 7 Step 1) isn't draining `from_registry_rx` — verify `targeted_forward_handle` was added and not aborted early. If B receives "hop 3": the hop counter isn't shared on `ServerState` or `a2a_hop_cap` isn't read from config — verify Task 5.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add server/tests/a2a_routing_e2e.rs
rtk proxy git commit -m "test(a2a): black-box agent→agent routing + hop-cap trip + notice (P1 Task 8)"
```

---

## Task 9: Connector SDK types — surface `senderType`/`to`/`addressed`/`verified`

**Files:**
- Modify: `packages/connector/src/protocol/types.ts` (`IncomingMessage` lines 14-22; `MessageContext.sendMessage` line 38).

- [ ] **Step 1: Extend `IncomingMessage` and the `sendMessage` signature**

Replace `IncomingMessage` (currently `types.ts:14-22`):

```ts
export interface IncomingMessage {
  id: string;
  from: string;
  threadId: string;
  timestamp: string;
  text: string;
  contentBlocks: ContentBlock[];
  raw: Record<string, unknown>;
}
```

with:

```ts
export interface IncomingMessage {
  id: string;
  from: string;
  threadId: string;
  timestamp: string;
  text: string;
  contentBlocks: ContentBlock[];
  /** Server-stamped sender classification. "agent" when another agent sent this. */
  senderType: "human" | "agent" | "system";
  /** The canonical recipient the server resolved for this message (or null). */
  to: string | null;
  /** Convenience: true when `to` equals this agent's own identity. */
  addressed: boolean;
  /** Whether the server verified the sender's signature. */
  verified: boolean;
  raw: Record<string, unknown>;
}
```

Replace the `sendMessage` line in `MessageContext` (currently `types.ts:38`):

```ts
  sendMessage: (content: ContentBlock[]) => Promise<void>;
```

with:

```ts
  sendMessage: (content: ContentBlock[], opts?: { to?: string }) => Promise<void>;
```

- [ ] **Step 2: Type-check**

Run (from `packages/connector`): `npm run build`
Expected: FAILS — `agent.ts` no longer satisfies the types (the `IncomingMessage` literal in `handleMessageDelivery` is missing the new required fields, and `sendMessage` signature differs). That failure is expected and fixed in Task 10. (If your tsconfig build doesn't fail here because `agent.ts` builds independently, proceed; Task 10 makes it correct regardless.)

- [ ] **Step 3: Commit**

```bash
rtk proxy git add packages/connector/src/protocol/types.ts
rtk proxy git commit -m "feat(a2a): connector IncomingMessage senderType/to/addressed/verified (P1 Task 9)"
```

---

## Task 10: Connector `agent.ts` — populate fields + thread `to`

**Files:**
- Modify: `packages/connector/src/agent.ts` — `handleMessageDelivery` (`agent.ts:182-293`).

- [ ] **Step 1: Populate the new `IncomingMessage` fields**

Replace the `IncomingMessage` construction (currently `agent.ts:188-200`):

```ts
    const contentBlocks = (message.content as ContentBlock[]) ?? [];
    const textBlock = contentBlocks.find((b) => b.type === "text");
    const text = (textBlock?.text as string) ?? "";

    const incoming: IncomingMessage = {
      id: (message.id as string) ?? randomUUID(),
      from: (message.from as string) ?? "",
      threadId: (message.thread_id as string) ?? "",
      timestamp: (message.timestamp as string) ?? new Date().toISOString(),
      text,
      contentBlocks,
      raw: message as Record<string, unknown>,
    };
```

with:

```ts
    const contentBlocks = (message.content as ContentBlock[]) ?? [];
    const textBlock = contentBlocks.find((b) => b.type === "text");
    const text = (textBlock?.text as string) ?? "";

    // Server-stamped fields live on the deliver WRAPPER (params), not inside `message`.
    const senderTypeRaw = params.sender_type as string | undefined;
    const senderType: IncomingMessage["senderType"] =
      senderTypeRaw === "agent" || senderTypeRaw === "system" ? senderTypeRaw : "human";
    const toResolved = (message.to as string | undefined) ?? null;
    const addressed = toResolved !== null && toResolved === this.config.identity;
    const verified = (params.verified as boolean | undefined) ?? false;

    const incoming: IncomingMessage = {
      id: (message.id as string) ?? randomUUID(),
      from: (message.from as string) ?? "",
      threadId: (message.thread_id as string) ?? "",
      timestamp: (message.timestamp as string) ?? new Date().toISOString(),
      text,
      contentBlocks,
      senderType,
      to: toResolved,
      addressed,
      verified,
      raw: message as Record<string, unknown>,
    };
```

- [ ] **Step 2: Thread `to` through `ctx.sendMessage`**

Replace `sendMessage` in the full `ctx` (currently `agent.ts:227-239`):

```ts
      async sendMessage(content: ContentBlock[]): Promise<void> {
        const envelope: Record<string, unknown> = {
          id: randomUUID(),
          from: keypair.publicKey.toString(),
          thread_id: threadId,
          timestamp: new Date().toISOString(),
          type: "message",
          content,
        };
        const signed = await signEnvelope(keypair, envelope);
        const notification = buildNotification("ainm.message.send", { message: signed });
        await sendFn(JSON.stringify(notification));
      },
```

with:

```ts
      async sendMessage(content: ContentBlock[], opts?: { to?: string }): Promise<void> {
        const envelope: Record<string, unknown> = {
          id: randomUUID(),
          from: agentIdentity,
          thread_id: threadId,
          timestamp: new Date().toISOString(),
          type: "message",
          content,
        };
        if (opts?.to) {
          envelope.to = opts.to;
        }
        const signed = await signEnvelope(keypair, envelope);
        const notification = buildNotification("ainm.message.send", { message: signed });
        await sendFn(JSON.stringify(notification));
      },
```

> Note: this also fixes `from` to use `agentIdentity` (`this.config.identity`) instead of `keypair.publicKey.toString()`, matching the auto-send path and what the server verifies against. `agentIdentity` is already in scope at `agent.ts:204`.

- [ ] **Step 3: Thread `to` through the auto-send-on-return path**

The handler can return a string OR `{ text, to }`. Keep the simple string contract and add optional addressing. Replace the auto-send block (currently `agent.ts:269-292`):

```ts
    const response = await this.messageHandler(incoming, ctx);

    // If handler returns a string, auto-send it as a signed message
    if (typeof response === "string") {
      const envelope: Record<string, unknown> = {
        id: randomUUID(),
        from: this.config.identity,
        thread_id: threadId,
        timestamp: new Date().toISOString(),
        type: "message",
        content: [
          {
            type: "text",
            version: "1.0",
            source: "agent",
            text: response,
            fallback: response,
          },
        ],
      };
      const signed = await signEnvelope(keypair, envelope);
      const request = buildRequest("ainm.message.send", signed as Record<string, unknown>);
      await this.transport.send(JSON.stringify(request));
    }
```

with (auto-echo the peer's address when the inbound sender was an agent, so agent replies form clean directed chains the UI renders):

```ts
    const response = await this.messageHandler(incoming, ctx);

    // If handler returns a string, auto-send it as a signed message.
    if (typeof response === "string") {
      // When an AGENT addressed us, reply back to that peer by default (runtime A2A).
      const replyTo = incoming.senderType === "agent" ? incoming.from : undefined;
      const envelope: Record<string, unknown> = {
        id: randomUUID(),
        from: this.config.identity,
        thread_id: threadId,
        timestamp: new Date().toISOString(),
        type: "message",
        content: [
          {
            type: "text",
            version: "1.0",
            source: "agent",
            text: response,
            fallback: response,
          },
        ],
      };
      if (replyTo) {
        envelope.to = replyTo;
      }
      const signed = await signEnvelope(keypair, envelope);
      const request = buildRequest("ainm.message.send", signed as Record<string, unknown>);
      await this.transport.send(JSON.stringify(request));
    }
```

- [ ] **Step 4: Build**

Run (from `packages/connector`): `npm run build`
Expected: PASS (types satisfied). Then `npm test`
Expected: PASS — if any existing test constructs an `IncomingMessage` or asserts on `sendMessage` params, update those tests to include the new fields / accept the new `from`. Make the minimal change to keep them green.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add packages/connector/src/agent.ts
rtk proxy git commit -m "feat(a2a): connector populates A2A fields + sends `to` (P1 Task 10)"
```

---

## Task 11: Reference connectors — addressed-gate + peer reply

**Files:**
- Modify: `packages/connector-openai/src/openai-agent.ts` (handler `openai-agent.ts:36-148`).
- Modify: `packages/connector-claude-code/src/claude-code-agent.ts` (its `onMessage` handler — read it first: `rtk proxy grep -n "onMessage" packages/connector-claude-code/src/claude-code-agent.ts`).

- [ ] **Step 1: Add the addressed-gate to the OpenAI connector**

In `packages/connector-openai/src/openai-agent.ts`, at the very top of the `agent.onMessage` async handler body (currently the first statement is `try {` at `openai-agent.ts:38`), insert a gate BEFORE the `try`:

Replace (currently `openai-agent.ts:36-38`):

```ts
  agent.onMessage(
    async (message: IncomingMessage, ctx: MessageContext): Promise<string | void> => {
      try {
```

with:

```ts
  agent.onMessage(
    async (message: IncomingMessage, ctx: MessageContext): Promise<string | void> => {
      // Runtime A2A: only respond when this agent was actually addressed. Prevents
      // every connected agent from answering every message in a multi-agent thread.
      // (A 1:1 thread where the human set `to` to this agent is still `addressed`.)
      if (!message.addressed) {
        return;
      }
      try {
```

> The auto-reply already echoes the peer `to` when the sender was an agent (Task 10 Step 3), so no further change is needed for directed replies — returning the string is enough.

- [ ] **Step 2: Add the same gate to the Claude Code connector**

Open `packages/connector-claude-code/src/claude-code-agent.ts`, find the `agent.onMessage(async (message, ctx) => { ... })` handler, and insert the identical gate as the first statement inside the handler body (before any other logic):

```ts
      // Runtime A2A: only respond when this agent was actually addressed.
      if (!message.addressed) {
        return;
      }
```

Match the file's existing parameter names (`message`, `ctx`) and indentation. If the handler destructures or names the message param differently, use that name with `.addressed`.

- [ ] **Step 3: Build both connectors**

Run (from `packages/connector-openai`): `npm run build`
Run (from `packages/connector-claude-code`): `npm run build`
Expected: both PASS. If either depends on a built `@ainm/connector`, build that first (`packages/connector && npm run build`).

- [ ] **Step 4: Commit**

```bash
rtk proxy git add packages/connector-openai/src/openai-agent.ts packages/connector-claude-code/src/claude-code-agent.ts
rtk proxy git commit -m "feat(a2a): reference connectors gate on `addressed` (P1 Task 11)"
```

---

## Task 12: Full-suite verification + clippy + DoD

**Files:** none (verification only).

- [ ] **Step 1: Server — full build, clippy, tests**

Run: `cargo build -p ainm-server`
Expected: clean.

Run: `cargo clippy -p ainm-server --all-targets -- -D warnings`
Expected: zero warnings. Fix any `dead_code`/unused-binding warnings the rewire introduced (e.g. the agent handler's `human_registry` clone, or `content_mentions` if now unused) by removing the dead binding — do NOT blanket-`#[allow]`.

Run: `cargo test -p ainm-server`
Expected: PASS. The two long-standing pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence` in `tests/integration.rs`) may still fail — confirm they are the ONLY failures and unrelated to this change (they predate it). The DB-gated tests (`ws_conversation_e2e`, `a2a_routing_e2e`) pass with a DB or self-skip without one.

- [ ] **Step 2: Connector SDK + reference connectors**

Run (from `packages/connector`): `npm run build && npm test`
Run (from `packages/connector-openai`): `npm run build`
Run (from `packages/connector-claude-code`): `npm run build`
Expected: all PASS.

- [ ] **Step 3: Definition-of-Done audit against the spec (P1 scope)**

Confirm each, citing the implementing task:
- [ ] Envelope `to` is honored for routing; `sender_type` is server-stamped on the deliver wrapper (Tasks 6, 7).
- [ ] `messages.to_addr` migration applied; `to_addr` + `signature` persisted (Task 3).
- [ ] `resolve_recipient` + `a2a_hop_decision` are pure and exhaustively unit-tested (Tasks 1, 2).
- [ ] Blind broadcast replaced by targeted delivery; `send_to_agent` is live (Tasks 4, 6, 7). The ONLY remaining `to_agent_tx` send is the human-no-target fallback (Task 6) — confirm via `rtk proxy grep -n "to_agent_tx.send" server/src/server.rs` (expect: only inside `deliver_message`).
- [ ] Agent thread-membership tracked + cleaned up on disconnect (Task 4).
- [ ] Per-thread hop cap (config `AINM_A2A__HOP_CAP`, default 6), human-reset, visible system notice on trip, NO silent drop (Tasks 5, 6, 8).
- [ ] Hosted-assistant trigger folded into `resolve_recipient`; `ws_conversation_e2e` still green (Task 7).
- [ ] Connector `IncomingMessage` exposes `senderType`/`to`/`addressed`/`verified`; `ctx.sendMessage({to})` + auto-send peer-`to` (Tasks 9, 10).
- [ ] Both reference connectors gate on `addressed` (Task 11).
- [ ] No iOS changes (P1 constraint) — confirm `rtk proxy git diff --name-only main~12 -- ios/` shows nothing from this work (adjust range as needed).
- [ ] Success criteria 1, 4, 5 from the spec hold: same-server agent→agent works and is hop-bounded with human reset (1); the SP1–SP4 UI renders live A2A with zero client changes (4 — verified structurally: agent messages carry `sender_type:"agent"` and reach humans via `send_to_thread`); no regression and no silent drop (5 — clippy clean, suites green, trip emits a notice).

- [ ] **Step 4: Final commit (if any fixes were made)**

```bash
rtk proxy git add -A
rtk proxy git commit -m "chore(a2a): P1 full-suite green + clippy + DoD audit (P1 Task 12)"
```

---

## Notes & Risks (read before executing)

- **Hosted-agent accessors:** Task 6/7 mirror `state.hosted_agent` / `agent.enabled` / `agent.address` / `state.shutdown.spawn_turn` exactly as used at `server.rs:2574-2596`. If those names differ in the live tree, match the live names.
- **`ServerState` construction sites (Task 5):** there may be more than one (production in `main.rs`, possibly an in-memory/test builder). The compiler enumerates missing-field errors; add the two fields to each, using `config.a2a.hop_cap` where `config` is in scope and the literal `6` where it is not.
- **Module declaration (Task 1):** `pub mod routing;` must sit beside `pub mod protocol;` (likely `server/src/lib.rs`). Verify before assuming.
- **The one surviving broadcast** (human → no-target fallback) is a deliberate backward-compat choice for clients that don't set `to`. It is NOT an A2A path (agent senders never use it). Tightening it to require `to` is a possible later follow-up, not P1.
- **`from` field fix (Task 10):** `ctx.sendMessage` previously signed with `from: keypair.publicKey.toString()`, which does not match the server's verification (`identity_store.get_public_key_bytes(from)` keyed by address). Switching to `agentIdentity` aligns it with the already-correct auto-send path; this is a latent-bug fix folded in because we are editing the same lines.
- **DB-gated tests** self-skip without Postgres. To actually exercise A2A locally, run with `TEST_DATABASE_URL` set. The hop-cap test uses `AINM_A2A__HOP_CAP=2` for speed.
```
