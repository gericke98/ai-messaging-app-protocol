# Federation Agent-Card Route Parity — Design (v1)

**Date:** 2026-06-05
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Context:** The client-facing agent-card route now resolves correctly via `published_card_for`
(`server/src/server.rs`): hosted assistant → static identity → live connector agent (AgentRegistry
by `{name}@{server_domain}`) → 404 for unknown/offline names (shipped in the hosted-assistant-card
and connector-agent-card sub-projects). But the **federation mTLS listener** has its own
`fed_agent_card_handler` (`server/src/main.rs`, built in `run_federation`) that captures only the
static `Arc<agent_card>` and **ignores the requested `{agent_name}`** — it serves the static card
under *any* name, with no 404. This is the last place the well-known card route does not behave
correctly. This sub-project brings the federation route to full parity with the client route.

## 1. Goal & scope

The federation listener's `/.well-known/ainm/{agent_name}/agent-card.json` resolves identically to
the client-facing route — hosted assistant → static identity → live connector agent → 404 — instead
of serving the static card under any name.

**Decisions (locked in brainstorming):**
- **Full parity.** Federation exposes hosted + static + **connector** cards, exactly like the client
  route. This requires the federation listener to share the *same* `AgentRegistry` instance that
  `handle_agent_connection` registers connector cards into.
- **Reuse the resolution logic.** Refactor `published_card_for` into a shared function both listeners
  call, so the 3-branch resolution can't drift between them. Each listener keeps its own (already
  identical) ETag/response wrapper; only the *resolution* is shared.

**Out of scope (v1):** changing federation auth / mTLS behavior; persisting cards; any change to the
client route's behavior; extracting a shared ETag-response helper (the two inline ETag blocks stay).

## 2. Architecture & components

```
server/src/server.rs   (modify) published_card_for: private fn(&ServerState,&str)
                                 → pub(crate) async fn over explicit pieces (hosted_agent,
                                 static_card, agent_registry, server_domain, agent_name).
                                 Update the one caller, agent_card_handler, to pass state.* pieces.
                        (modify) start_federation: accept an injected `AgentRegistry` param instead
                                 of creating it internally; store it in ServerState as today.
server/src/main.rs     (modify) run_federation: construct the AgentRegistry once; pass a clone into
                                 start_federation; keep a clone (+ a hosted_agent clone) to capture
                                 in the federation handler.
                        (modify) fed_agent_card_handler: resolve via the shared published_card_for
                                 (hosted/static/connector → 200, unknown/offline → 404); keep the
                                 existing ETag wrapper over the resolved card.
server/tests/          (new)    federation-listener card-route test (DB-or-skip; mTLS client):
                                 unknown name → 404, valid identity → 200. Connector-over-fed
                                 assertion if mTLS-client setup is practical; else manual.
```

No client-route behavior changes; iOS unaffected.

## 3. Shared resolver (`server/src/server.rs`)

`published_card_for` is currently a private `fn(state: &ServerState, agent_name: &str) ->
Option<Value>` with a single caller (`agent_card_handler`) and an `async` body (it `.await`s the
registry). Refactor it to a crate-visible free function over the exact pieces it reads:

```rust
pub(crate) async fn published_card_for(
    hosted_agent: Option<&crate::hosted_agent::HostedAgent>,
    static_card: &serde_json::Value,
    agent_registry: &AgentRegistry,
    server_domain: &str,
    agent_name: &str,
) -> Option<serde_json::Value>
```

The body is the existing 3-branch logic, reading from the params instead of `state.*`:
1. **Hosted assistant:** if `hosted_agent` is `Some` and its address local-part == `agent_name` →
   `build_assistant_card(&agent.address)`.
2. **Static card:** if `static_card`'s declared `address`/`ainm.address` local-part == `agent_name`
   → clone it.
3. **Live connector agent:** `agent_registry.get_agent_card(&format!("{agent_name}@{server_domain}"))`
   → serve if present.
4. Else `None`.

`agent_card_handler` (the client route, in `server.rs`) is updated to call:
```rust
published_card_for(
    state.hosted_agent.as_deref(),
    &state.agent_card,
    &state.agent_registry,
    &state.server_domain,
    &agent_name,
).await
```
`pub(crate)` makes it reachable from `main.rs` (same binary crate) as
`crate::server::published_card_for`. No behavior change to the client route — the existing
`agent_card_route` tests must stay green.

## 4. Share the AgentRegistry into the federation listener

The federation listener (`fed_app`) is built in `main.rs::run_federation`, separately from the Axum
app `start_federation` builds; it does not hold `ServerState`. To serve connector cards over
federation it must consult the *same* `AgentRegistry` instance that `handle_agent_connection`
(inside `start_federation`) registers into.

`AgentRegistry` is `Clone` and Arc-backed (it is a field of the `#[derive(Clone)] ServerState`), so
the same underlying maps are shared by any clone. Change `start_federation` to **accept an
`AgentRegistry` parameter** instead of creating `AgentRegistry::new()` internally:

- In `run_federation`, construct `let agent_registry = AgentRegistry::new();` once.
- Pass `agent_registry.clone()` into `start_federation` (new parameter); `start_federation` stores it
  in `ServerState` exactly as it stores the internally-created one today.
- Keep `agent_registry` (and a clone of `hosted_agent`, which is otherwise moved into
  `start_federation`) in `run_federation` scope to capture in the federation handler.

Only `start_federation`'s signature and its single call site change. `start` / `start_with_port`
(the non-federation entry points, e.g. UAT) keep creating their own registry and are untouched.

## 5. Rewrite `fed_agent_card_handler` (`main.rs`)

The handler closure currently captures `Arc<agent_card>` and ignores the path name. Change it to
capture the pieces the resolver needs — `hosted_agent_for_fed` (the pre-move clone), the static
`agent_card`, the shared `agent_registry`, and `server_id` (owned `String`) — and resolve:

```rust
move |headers: HeaderMap, Path(agent_name): Path<String>| {
    let hosted = hosted_agent_for_fed.clone();
    let static_card = agent_card.clone();
    let registry = agent_registry.clone();
    let domain = server_id.clone();
    async move {
        let Some(card) = crate::server::published_card_for(
            hosted.as_deref(), &static_card, &registry, &domain, &agent_name,
        ).await else {
            return axum::http::StatusCode::NOT_FOUND.into_response();
        };
        // existing ETag / If-None-Match / 200 wrapper, now over `card`
        ...
    }
}
```

The ETag wrapper (SHA-256 of the body, `If-None-Match` → 304, else 200 with `ETag` +
`application/json`) is unchanged — it now operates on the resolved `card` instead of the always-static
card. The path param is now used (`agent_name`, not `_agent_name`).

## 6. Data flow

Remote server → mTLS → `GET /.well-known/ainm/{name}/agent-card.json` on the federation listener →
`published_card_for(hosted, static, shared_registry, server_domain, name)` → hosted/static/connector
card (200 + ETag) or 404. A connector agent connected on the *local* listener is now visible to
federated peers because both listeners share one `AgentRegistry`. On connector disconnect the shared
registry drops the card (the existing cleanup fix applies), so the federation route 404s too.

## 7. Error handling

- **Unknown / offline name** → resolver `None` → 404 (replaces the bug: static card under any name).
- **Disconnected connector** → shared registry has no entry → 404 (same ephemeral lifecycle as the
  client route).
- **Null / identity-less static card** with no hosted/connector match → 404.
- The resolver never panics (all `Option`-chained, mirrors current code).

## 8. Testing

- **Resolver refactor:** behavior is unchanged, so the existing `agent_card_route` tests (client
  route) are the primary guard — they must stay green after the signature change. No new unit test is
  needed for the free function unless its extraction surfaces a gap (it should not).
- **Federation integration (DB-or-skip):** a new test that exercises the *federation* listener over
  mTLS using the dev client certs (`server/certs`). Assert `/.well-known/ainm/<identity>/agent-card.json`
  → 200 with a `name`, and `/.well-known/ainm/ghost/agent-card.json` → 404 (the bug regression). If a
  TLS client against the fed port is impractical in-harness (client-cert handshake setup), scope the
  automated test to unknown→404 + valid→200 and defer the connector-over-federation assertion to the
  manual step. Document whichever scoping is chosen in the test file.
- **Regression:** full server suite green except the two known pre-existing failures
  (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`); clippy clean on touched files.
- **Manual (human):** bring up two federated servers (docker-compose) and fetch an agent card across
  the federation boundary; confirm a valid identity resolves and an unknown name 404s.

## 9. Definition of Done

1. `published_card_for` is a shared `pub(crate) async` resolver over explicit pieces; the client
   `agent_card_handler` is updated to call it; the client route's behavior is unchanged (existing
   `agent_card_route` tests green).
2. `start_federation` accepts an injected `AgentRegistry`; `run_federation` constructs one and shares
   it across both listeners (local + federation).
3. `fed_agent_card_handler` resolves via the shared resolver: hosted/static/connector identities →
   200, unknown/offline → 404 (no more static-card-under-any-name).
4. A federation regression test locks unknown→404 and valid-identity→200 (DB-or-skip; connector-over-
   federation asserted automatically if the mTLS client harness is practical, else manual).
5. Server suite green except the two known pre-existing failures; clippy clean on touched files.
6. Cross-server card fetch verified manually (human).

## 10. Build order

Shared resolver refactor + update the client caller (keep `agent_card_route` green) → inject
`AgentRegistry` into `start_federation` + share one instance in `run_federation` → rewrite
`fed_agent_card_handler` to use the resolver → federation integration test → full suites green →
manual cross-server fetch. The resolver refactor lands first (it's purely internal and keeps the
client route green); the federation wiring builds on it.
