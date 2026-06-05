# Federation Agent-Card Route Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The federation mTLS listener's agent-card route resolves identically to the client route (hosted → static → connector → 404) instead of serving the static card under any name.

**Architecture:** Refactor `published_card_for` into a shared `pub(crate) async` resolver over explicit pieces (no `&ServerState`), called by both the client handler and the federation handler. Inject one shared `AgentRegistry` into `start_federation` so the federation listener sees the same connector cards. Rewrite `fed_agent_card_handler` to resolve via the shared function and 404 on miss.

**Tech Stack:** Rust (Axum, serde_json, tokio), binary-only `server.rs` + `main.rs` (same binary crate).

**Conventions for this plan:**
- All git operations use `rtk proxy git` (never bare `git`). Commits go **directly to `main`**.
- Server tests: `cd server && cargo test <filter>`. Two pre-existing failures are expected and unrelated: `test_graph_traversal`, `test_compilation_dedup_updates_confidence` (`tests/integration.rs`).
- `server.rs` and `main.rs` are the same binary crate (`server.rs` is `mod server;` in `main.rs`); `crate::server::<item>` resolves from `main.rs` for `pub`/`pub(crate)` items.
- `AgentRegistry` is `pub` and `Clone` (Arc-backed), so clones share the same underlying maps.
- **Testing decision (per spec §7/§8 contingency):** there is no existing mTLS-client test harness and the dev `reqwest` lacks client-cert features, so the fed-transport e2e (200/404 over mTLS) is the **manual** cross-server step (Task 5). Automated coverage is the **shared-resolver unit test** (Task 1) plus the existing `agent_card_route` client tests as the refactor regression guard. The ETag wrapper is NOT extracted (spec keeps it per-listener).

---

## Task 1: Shared resolver — refactor `published_card_for` + unit tests

Turn `published_card_for` from a private `fn(&ServerState, &str)` into a `pub(crate) async fn` over explicit pieces, update its one caller, and add a deterministic unit test. Behavior is unchanged → the existing `agent_card_route` tests must stay green.

**Files:**
- Modify: `server/src/server.rs` (`published_card_for` ~line 1007, `agent_card_handler` caller ~line 1052, add `#[cfg(test)] mod tests`)

- [ ] **Step 1: Rewrite `published_card_for` to take explicit pieces**

The CURRENT function (around lines 1007-1045) is:

```rust
async fn published_card_for(state: &ServerState, agent_name: &str) -> Option<serde_json::Value> {
    // (1) Hosted assistant: build its canonical card on demand.
    if let Some(agent) = &state.hosted_agent {
        let local = agent.address.split('@').next().unwrap_or("");
        if !local.is_empty() && local == agent_name {
            return Some(crate::agent_card::build_assistant_card(&agent.address));
        }
    }

    // (2) Static card: serve only under its own declared identity.
    let card = &state.agent_card;
    if let Some(addr) = card
        .get("address")
        .and_then(|v| v.as_str())
        .or_else(|| {
            card.get("ainm")
                .and_then(|a| a.get("address"))
                .and_then(|v| v.as_str())
        })
    {
        let local = addr.split('@').next().unwrap_or("");
        if !local.is_empty() && local == agent_name {
            return Some(card.clone());
        }
    }

    // (3) Live connector agent: serve its registered (ephemeral) card while connected.
    let full_addr = format!("{agent_name}@{}", state.server_domain);
    if let Some(card) = state.agent_registry.get_agent_card(&full_addr).await {
        return Some(card);
    }

    None
}
```

Replace it with the crate-visible free-function form (keep the doc comment; update its first line to note it's shared by both listeners):

```rust
/// The agent card this server publishes for `agent_name`, or `None` (→ 404).
/// Shared by the client-facing `agent_card_handler` and the federation listener's handler so the
/// resolution logic can't drift between listeners.
///
/// Resolution order: (1) hosted assistant → (2) static identity card → (3) live connector agent
/// (AgentRegistry by `{agent_name}@{server_domain}`) → `None`.
pub(crate) async fn published_card_for(
    hosted_agent: Option<&crate::hosted_agent::HostedAgent>,
    static_card: &serde_json::Value,
    agent_registry: &AgentRegistry,
    server_domain: &str,
    agent_name: &str,
) -> Option<serde_json::Value> {
    // (1) Hosted assistant: build its canonical card on demand.
    if let Some(agent) = hosted_agent {
        let local = agent.address.split('@').next().unwrap_or("");
        if !local.is_empty() && local == agent_name {
            return Some(crate::agent_card::build_assistant_card(&agent.address));
        }
    }

    // (2) Static card: serve only under its own declared identity.
    if let Some(addr) = static_card
        .get("address")
        .and_then(|v| v.as_str())
        .or_else(|| {
            static_card
                .get("ainm")
                .and_then(|a| a.get("address"))
                .and_then(|v| v.as_str())
        })
    {
        let local = addr.split('@').next().unwrap_or("");
        if !local.is_empty() && local == agent_name {
            return Some(static_card.clone());
        }
    }

    // (3) Live connector agent: serve its registered (ephemeral) card while connected.
    let full_addr = format!("{agent_name}@{server_domain}");
    if let Some(card) = agent_registry.get_agent_card(&full_addr).await {
        return Some(card);
    }

    None
}
```

- [ ] **Step 2: Update the client caller (`agent_card_handler`)**

Find the call in `agent_card_handler` (~line 1052):

```rust
    let Some(card) = published_card_for(&state, &agent_name).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
```

Replace with:

```rust
    let Some(card) = published_card_for(
        state.hosted_agent.as_deref(),
        &state.agent_card,
        &state.agent_registry,
        &state.server_domain,
        &agent_name,
    )
    .await
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
```

(`state.hosted_agent` is `Option<Arc<HostedAgent>>`; `.as_deref()` yields `Option<&HostedAgent>`.)

- [ ] **Step 3: Add a resolver unit test module**

`server.rs` has no `#[cfg(test)]` module yet. Append one at the end of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn published_card_for_resolves_static_connector_and_unknown() {
        // Static identity card for "helper@testdom".
        let static_card = json!({
            "name": "helper",
            "address": "helper@testdom",
            "ainm": { "address": "helper@testdom" }
        });

        // A connector agent "coder@testdom" registered in the (ephemeral) AgentRegistry.
        let registry = AgentRegistry::new();
        let connector_card = json!({ "name": "Coder", "ainm": { "address": "coder@testdom" } });
        let _rx = registry.register("coder@testdom", &connector_card).await;

        // (2) Static identity match.
        let got = published_card_for(None, &static_card, &registry, "testdom", "helper").await;
        assert_eq!(got.as_ref().and_then(|c| c.get("name")).and_then(|n| n.as_str()), Some("helper"));

        // (3) Live connector match by {name}@{domain}.
        let got = published_card_for(None, &static_card, &registry, "testdom", "coder").await;
        assert_eq!(got.as_ref().and_then(|c| c["ainm"].get("address")).and_then(|a| a.as_str()), Some("coder@testdom"));

        // Unknown name → None (→ 404).
        let got = published_card_for(None, &static_card, &registry, "testdom", "ghost").await;
        assert!(got.is_none(), "unknown name must resolve to None");

        // A static card whose identity does not match falls through to connector/None, not early-return.
        let other = json!({ "address": "someoneelse@testdom" });
        let got = published_card_for(None, &other, &registry, "testdom", "coder").await;
        assert!(got.is_some(), "non-matching static card must not block the connector branch");
    }
}
```

(The hosted-assistant branch is unchanged logic and is covered by `build_assistant_card`'s unit tests and the assistant-card route test; constructing a real `HostedAgent` requires a keypair, so it's not duplicated here.)

- [ ] **Step 4: Build + run the resolver test and the client route regression**

Run: `cd server && cargo build --bin ainm-server 2>&1 | tail -8`
Expected: clean compile, no new warnings.

Run: `cd server && cargo test --bin ainm-server published_card_for`
Expected: PASS — `published_card_for_resolves_static_connector_and_unknown` passes.

Run: `cd server && cargo test --test agent_card_route`
Expected: PASS — the client route's behavior is unchanged by the refactor.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "refactor(server): published_card_for as shared pub(crate) resolver + unit test"
```

---

## Task 2: Inject a shared `AgentRegistry` into `start_federation`

Let `run_federation` construct one `AgentRegistry` and share it across both listeners by passing it into `start_federation` instead of having `start_federation` create its own.

**Files:**
- Modify: `server/src/server.rs` (`start_federation` signature ~line 911, ServerState construction ~line 937)
- Modify: `server/src/main.rs` (`run_federation` call site ~line 603)

- [ ] **Step 1: Add the parameter to `start_federation`**

In `server/src/server.rs`, the `start_federation` signature ends with:

```rust
    cred_cipher: Option<std::sync::Arc<crate::credentials::CredCipher>>,
    quota: crate::config::QuotaConfig,
    shutdown: crate::lifecycle::ShutdownController,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
```

Add an `agent_registry` parameter immediately before `shutdown`:

```rust
    cred_cipher: Option<std::sync::Arc<crate::credentials::CredCipher>>,
    quota: crate::config::QuotaConfig,
    agent_registry: AgentRegistry,
    shutdown: crate::lifecycle::ShutdownController,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
```

- [ ] **Step 2: Use the injected registry in `ServerState`**

In the `ServerState { ... }` construction inside `start_federation` (~line 937), change:

```rust
        agent_registry: AgentRegistry::new(),
```

to:

```rust
        agent_registry,
```

(Only the `start_federation` ServerState uses the injected one. The `start` and `start_with_port` constructions at ~lines 774 and 850 keep `AgentRegistry::new()` — they have no federation listener.)

- [ ] **Step 3: Construct + pass the registry in `run_federation`**

In `server/src/main.rs`, immediately BEFORE the `let (server_addr, server_handle) = server::start_federation(` call (~line 603), add:

```rust
    // Shared agent registry: one instance backs both the local listener (inside start_federation)
    // and the federation listener's card handler, so connector cards are visible over federation.
    let agent_registry = server::AgentRegistry::new();
    // Keep a hosted-agent handle for the federation card handler (hosted_agent is moved below).
    let hosted_agent_for_fed = hosted_agent.clone();
```

Then in the `start_federation(...)` argument list, add `agent_registry.clone(),` immediately before the final `controller.clone(),` argument:

```rust
        cred_cipher,
        quota_config,
        agent_registry.clone(),
        controller.clone(),
    )
    .await?;
```

- [ ] **Step 4: Verify it compiles**

Run: `cd server && cargo build --bin ainm-server 2>&1 | tail -8`
Expected: clean compile. `agent_registry` and `hosted_agent_for_fed` are currently unused after construction (a warning is acceptable at this step — Task 3 consumes them in the fed handler). If the unused warning blocks `-D warnings` anywhere, ignore for now; Task 3 resolves it. (Plain `cargo build` only warns, does not fail.)

- [ ] **Step 5: Commit**

```bash
rtk proxy git add server/src/server.rs server/src/main.rs
rtk proxy git commit -m "feat(server): inject shared AgentRegistry into start_federation"
```

---

## Task 3: Rewrite `fed_agent_card_handler` to use the shared resolver

Make the federation card handler resolve via `published_card_for` and 404 on miss, instead of serving the static card under any name.

**Files:**
- Modify: `server/src/main.rs` (`fed_agent_card_handler` ~lines 627-663)

- [ ] **Step 1: Replace the handler closure**

The CURRENT code (around lines 627-663) is:

```rust
    // Build agent card handler for federation listener (serves same card as local listener)
    let agent_card_for_fed = Arc::new(agent_card.clone());
    let fed_agent_card_handler = {
        let card = agent_card_for_fed.clone();
        move |headers: axum::http::HeaderMap, axum::extract::Path(_agent_name): axum::extract::Path<String>| {
            let card = card.clone();
            async move {
                let body = serde_json::to_vec(&*card).unwrap();
                let hash = ring::digest::digest(&ring::digest::SHA256, &body);
                let etag_value = format!(
                    "\"{}\"",
                    base64::engine::general_purpose::URL_SAFE_NO_PAD
                        .encode(hash.as_ref())
                );
                if let Some(if_none_match) = headers.get(axum::http::header::IF_NONE_MATCH) {
                    if if_none_match.to_str().unwrap_or("") == etag_value {
                        return (
                            axum::http::StatusCode::NOT_MODIFIED,
                            [(axum::http::header::ETAG, etag_value)],
                        )
                            .into_response();
                    }
                }
                (
                    axum::http::StatusCode::OK,
                    [
                        (
                            axum::http::header::CONTENT_TYPE,
                            "application/json".to_string(),
                        ),
                        (axum::http::header::ETAG, etag_value),
                    ],
                    body,
                )
                    .into_response()
            }
        }
    };
```

Replace it with a version that resolves via the shared resolver and 404s on miss (the ETag wrapper is preserved, now over the resolved card):

```rust
    // Federation card handler: resolves identically to the client route via the shared
    // `published_card_for` (hosted → static → live connector → 404), over the shared AgentRegistry.
    let fed_agent_card_handler = {
        let hosted = hosted_agent_for_fed.clone();
        let static_card = agent_card.clone();
        let registry = agent_registry.clone();
        let domain = server_id.to_string();
        move |headers: axum::http::HeaderMap, axum::extract::Path(agent_name): axum::extract::Path<String>| {
            let hosted = hosted.clone();
            let static_card = static_card.clone();
            let registry = registry.clone();
            let domain = domain.clone();
            async move {
                let Some(card) = crate::server::published_card_for(
                    hosted.as_deref(),
                    &static_card,
                    &registry,
                    &domain,
                    &agent_name,
                )
                .await
                else {
                    return axum::http::StatusCode::NOT_FOUND.into_response();
                };

                let body = serde_json::to_vec(&card).unwrap();
                let hash = ring::digest::digest(&ring::digest::SHA256, &body);
                let etag_value = format!(
                    "\"{}\"",
                    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash.as_ref())
                );
                if let Some(if_none_match) = headers.get(axum::http::header::IF_NONE_MATCH) {
                    if if_none_match.to_str().unwrap_or("") == etag_value {
                        return (
                            axum::http::StatusCode::NOT_MODIFIED,
                            [(axum::http::header::ETAG, etag_value)],
                        )
                            .into_response();
                    }
                }
                (
                    axum::http::StatusCode::OK,
                    [
                        (axum::http::header::CONTENT_TYPE, "application/json".to_string()),
                        (axum::http::header::ETAG, etag_value),
                    ],
                    body,
                )
                    .into_response()
            }
        }
    };
```

Notes:
- `hosted` is `Option<Arc<HostedAgent>>`; `.as_deref()` → `Option<&HostedAgent>`.
- `static_card` is the static `agent_card` (`serde_json::Value`), cloned per request (matches the client handler, which also clones).
- The route registration line (`.route("/.well-known/ainm/{agent_name}/agent-card.json", axum::routing::get(fed_agent_card_handler))`) is unchanged — it still references `fed_agent_card_handler`.
- This consumes `hosted_agent_for_fed` and `agent_registry`, clearing the Task 2 unused warnings.
- The now-removed `let agent_card_for_fed = Arc::new(agent_card.clone());` line is deleted (the new closure captures `agent_card` directly). Ensure no other code references `agent_card_for_fed` (it was only used here).

- [ ] **Step 2: Verify it compiles cleanly**

Run: `cd server && cargo build --bin ainm-server 2>&1 | tail -10`
Expected: clean compile, NO warnings about unused `agent_registry` / `hosted_agent_for_fed` / `agent_card_for_fed` (the first two are now used; the third is removed).

- [ ] **Step 3: Confirm the client route still passes**

Run: `cd server && cargo test --test agent_card_route`
Expected: PASS (the client route is untouched; this task only changed the federation handler).

- [ ] **Step 4: Commit**

```bash
rtk proxy git add server/src/main.rs
rtk proxy git commit -m "feat(server): federation card route resolves via shared published_card_for (404 on miss)"
```

---

## Task 4: Full suites green + Definition of Done

Verification only. No code changes unless a clear trivial fix is needed (commit any such fix with `fix(...)`).

**Files:** none.

- [ ] **Step 1: Full server suite**

Run: `cd server && cargo test 2>&1 | tail -40`
Expected: green EXCEPT the two known pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`). The new `published_card_for` unit test passes; `agent_card_route` passes; DB-backed tests skip without a DB. Confirm no other regressions; list any in full.

- [ ] **Step 2: Clippy (touched files)**

Run: `cd server && cargo clippy --bin ainm-server -- -D warnings 2>&1 | tail -30`
Expected: no warnings attributable to the `published_card_for` refactor, the `start_federation` signature change, or the `fed_agent_card_handler` rewrite. Pre-existing repo-wide clippy debt in OTHER files (compiler.rs, graph.rs, etc.) is out of scope — distinguish clearly. Report the verdict.

- [ ] **Step 3: Confirm DoD (spec §9)**

1. `published_card_for` is a shared `pub(crate) async` resolver; client handler updated; client route behavior unchanged (agent_card_route green). ✅ (Task 1)
2. `start_federation` accepts an injected `AgentRegistry`; `run_federation` shares one instance. ✅ (Task 2)
3. `fed_agent_card_handler` resolves via the shared resolver: hosted/static/connector → 200, unknown/offline → 404. ✅ (Task 3)
4. Federation regression: locked by the shared-resolver unit test (unknown→None, static/connector→Some); the fed-transport 200/404 over mTLS is the manual cross-server step (Task 5), per the spec's §7/§8 contingency. ✅ (Tasks 1, 5)
5. Server suite green except the two known failures; clippy clean on touched files. ✅ (Steps 1–2)
6. Manual cross-server fetch — Task 5.

No commit unless a fix was required.

---

## Task 5: MANUAL cross-server verification (human-run)

The fed-transport behavior over mTLS is verified by a human (no automated mTLS-client harness exists). Two federated servers, or one server hit with an mTLS client.

**Runbook (human):**

1. Bring up the federation (e.g. `docker-compose` with two servers, or a single DB-backed server: `AINM_DATABASE__URL=postgres://ainm:dev_password@localhost:5432/ainm cargo run -p ainm-server -- --mode agent --identity localhost --port 8080 --federation-port 8443 --cert server/certs/server-a.pem --key server/certs/server-a-key.pem --ca-cert server/certs/ca.pem`).
2. With an mTLS client presenting the dev client cert (e.g. `curl --cert server/certs/server-a.pem --key server/certs/server-a-key.pem --cacert server/certs/ca.pem -k https://localhost:8443/.well-known/ainm/<identity>/agent-card.json`), confirm:
   - A valid identity (the server's static/agent identity, e.g. `helper`, or the hosted `assistant`) → **200** with the real card.
   - An unknown name (e.g. `ghost`) → **404** (previously: served the static card under any name).
   - (Optional) Connect a connector agent and confirm its `{name}@<host>` card resolves over the federation port while connected, and 404s after it disconnects.
3. If running two real federated servers, fetch a peer's agent card across the federation boundary and confirm it resolves.

- [ ] **Step 1:** Human runs the runbook and confirms valid→200 / unknown→404 over the federation listener. Mark complete when verified.
