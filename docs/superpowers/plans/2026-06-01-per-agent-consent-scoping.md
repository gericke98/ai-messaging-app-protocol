# Per-Agent Consent Scoping Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enforce resolved consent scopes in `generate_briefing` so an agent sees only the memory categories its scopes permit, gate the peer card by the identity scope, and close the empty-filter leak.

**Architecture:** In `generate_briefing` (already calls `resolve_scopes`), translate the resolved scopes into an allowed-category set via `scope_to_categories`, pass it as the SQL `category_filter` on the candidate and boost queries, short-circuit to an empty briefing when nothing is allowed (because `query_nodes` treats `Some(&[])` as "no filter"), and only build the peer card when the identity scope is granted. Default consent stays `Standard`; no client or schema changes.

**Tech Stack:** Rust 2024 (crate `ainm-demo` under `demo/`), sqlx + Postgres, `cargo test`. Integration tests are DB-backed and `return` early if no Postgres is running.

**Spec:** `docs/superpowers/specs/2026-06-01-per-agent-consent-scoping-design.md`

**Pre-existing failures unrelated to this work (NOT regressions):** `test_graph_traversal`, `test_compilation_dedup_updates_confidence` (verified failing on `main`).

**Key reference — consent → category exposure:**
- Minimal (scopes: identity) → `fact, relationship`
- Standard (identity, preferences, expertise, goals) → `fact, relationship, preference, tool, skill, event, decision`
- Full (all six) → all 13

**Test infra facts (from `demo/tests/integration.rs`):**
- `test_pool().await` returns `Option<PgPool>`; tests start with `let Some(pool) = test_pool().await else { return };`.
- `unique_addr("prefix")` makes a unique user/agent address.
- Seed a node: `graph::insert_node(&pool, &user, NODE_TYPE, CATEGORY, LABEL, &serde_json::json!({}), CONFIDENCE, &serde_json::json!({}), &serde_json::json!([])).await.unwrap();`
- `briefing::generate_briefing(&pool, &user, &agent, THREAD_ID, ConsentLevel::X).await.unwrap()` — the 5th arg is the **default** consent (used when no per-agent grant exists). A non-existent thread id is fine (no recent messages → no query boost).
- `briefing::grant_scopes(&pool, &user, &agent, &["scope".to_string(), ...], ConsentLevel::X).await.unwrap()` creates a per-agent grant.
- `sections` is a JSON object keyed by **category** name; `peer_card` is a string.

---

### Task 1: Scope retrieval by allowed categories (+ empty short-circuit)

**Files:**
- Modify: `demo/src/briefing.rs` (inside `generate_briefing`, lines ~225-240)
- Test: `demo/tests/integration.rs` (new tests appended near the other briefing tests)

- [ ] **Step 1: Write the failing integration tests**

Append to `demo/tests/integration.rs`:

```rust
// ── Briefing: Consent scoping enforcement ─────────────────────────────────

#[tokio::test]
async fn briefing_minimal_scope_shows_only_identity_categories() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("pcs_min");
    let agent = unique_addr("pcs_min_agent");
    let prov = serde_json::json!({});
    let tags = serde_json::json!([]);
    graph::insert_node(&pool, &user, "semantic", "fact", "Name is Dana", &serde_json::json!({}), 0.95, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "semantic", "skill", "Expert in Rust", &serde_json::json!({}), 0.9, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "procedural", "pattern", "Works in sprints", &serde_json::json!({}), 0.85, &prov, &tags).await.unwrap();

    let b = briefing::generate_briefing(&pool, &user, &agent, "pcs_t_min", ConsentLevel::Minimal).await.unwrap();
    let sections = b.get("sections").unwrap().as_object().unwrap();
    assert!(sections.contains_key("fact"), "identity 'fact' must be present at minimal: {sections:?}");
    assert!(!sections.contains_key("skill"), "expertise 'skill' must be excluded at minimal");
    assert!(!sections.contains_key("pattern"), "'pattern' must be excluded at minimal");
}

#[tokio::test]
async fn briefing_standard_scope_excludes_history_and_patterns() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("pcs_std");
    let agent = unique_addr("pcs_std_agent");
    let prov = serde_json::json!({});
    let tags = serde_json::json!([]);
    graph::insert_node(&pool, &user, "semantic", "fact", "Founder", &serde_json::json!({}), 0.95, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "semantic", "preference", "Likes Rust", &serde_json::json!({}), 0.9, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "episodic", "decision", "Chose Axum", &serde_json::json!({}), 0.88, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "episodic", "outcome", "Shipped v1", &serde_json::json!({}), 0.9, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "procedural", "pattern", "Sprints", &serde_json::json!({}), 0.85, &prov, &tags).await.unwrap();

    let b = briefing::generate_briefing(&pool, &user, &agent, "pcs_t_std", ConsentLevel::Standard).await.unwrap();
    let sections = b.get("sections").unwrap().as_object().unwrap();
    assert!(sections.contains_key("fact"), "standard includes fact");
    assert!(sections.contains_key("preference"), "standard includes preference");
    assert!(sections.contains_key("decision"), "standard includes decision (goals)");
    assert!(!sections.contains_key("outcome"), "outcome is history-only, excluded at standard");
    assert!(!sections.contains_key("pattern"), "pattern is patterns-only, excluded at standard");
}

#[tokio::test]
async fn briefing_full_scope_shows_all_categories() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("pcs_full");
    let agent = unique_addr("pcs_full_agent");
    let prov = serde_json::json!({});
    let tags = serde_json::json!([]);
    graph::insert_node(&pool, &user, "semantic", "fact", "Founder", &serde_json::json!({}), 0.95, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "episodic", "outcome", "Shipped v1", &serde_json::json!({}), 0.9, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "procedural", "pattern", "Sprints", &serde_json::json!({}), 0.85, &prov, &tags).await.unwrap();

    let b = briefing::generate_briefing(&pool, &user, &agent, "pcs_t_full", ConsentLevel::Full).await.unwrap();
    let sections = b.get("sections").unwrap().as_object().unwrap();
    assert!(sections.contains_key("fact") && sections.contains_key("outcome") && sections.contains_key("pattern"),
        "full consent surfaces every category: {sections:?}");
}

#[tokio::test]
async fn briefing_unmapped_scope_returns_empty_without_leak() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("pcs_empty");
    let agent = unique_addr("pcs_empty_agent");
    let prov = serde_json::json!({});
    let tags = serde_json::json!([]);
    graph::insert_node(&pool, &user, "semantic", "fact", "Secret name", &serde_json::json!({}), 0.95, &prov, &tags).await.unwrap();

    // A non-empty grant whose scope maps to NO categories (resolve_scopes returns it verbatim
    // because it's non-empty; scope_to_categories("bogus") = vec![] → allowed is empty).
    briefing::grant_scopes(&pool, &user, &agent, &["bogus".to_string()], ConsentLevel::Minimal).await.unwrap();

    let b = briefing::generate_briefing(&pool, &user, &agent, "pcs_t_empty", ConsentLevel::Standard).await.unwrap();
    let sections = b.get("sections").unwrap().as_object().unwrap();
    assert!(sections.is_empty(), "no category granted → empty sections (no leak): {sections:?}");
    assert_eq!(b.get("peer_card").unwrap().as_str().unwrap(), "", "no peer card when nothing granted");
    let blob = serde_json::to_string(&b).unwrap();
    assert!(!blob.contains("Secret name"), "no out-of-scope fact may leak into the briefing");
}
```

- [ ] **Step 2: Run the tests to verify they fail (requires Postgres)**

Run: `cargo test -p ainm-demo --test integration briefing_minimal_scope_shows_only_identity_categories briefing_unmapped_scope_returns_empty_without_leak -- --test-threads=1`
Expected: FAIL — current `generate_briefing` passes `None` for categories, so `skill`/`pattern` appear at minimal and the bogus-scope briefing returns all facts ("Secret name" leaks).
(Note: `cargo test` accepts only one positional filter; run each test name in its own invocation if needed. If Postgres is NOT running these tests `return` early and pass trivially — start Postgres to observe red→green.)

- [ ] **Step 3: Implement scoping + short-circuit**

In `demo/src/briefing.rs`, replace this block (currently ~lines 225-231):

```rust
    // --- hybrid retrieval: query as BOOST, not filter ---

    // 1) Broad candidate set: ALL current facts for the user (no category filter —
    //    per-agent consent scoping is deferred for this MVP).
    let candidates = graph::query_nodes(
        db, user_addr, None, None, None, "current", 0.0, 200,
    ).await.map_err(|e| AppError::Storage(e))?;
```

with:

```rust
    // Consent enforcement: an agent sees only the categories its resolved scopes permit.
    let mut allowed: Vec<&str> = scopes
        .iter()
        .flat_map(|s| scope_to_categories(s))
        .collect();
    allowed.sort_unstable();
    allowed.dedup();

    // Granted nothing → see nothing. graph::query_nodes treats Some(&[]) as "no filter"
    // (it would return ALL categories), so we must short-circuit rather than pass an empty slice.
    if allowed.is_empty() {
        let _ = sqlx::query(
            "INSERT INTO briefing_log (user_addr, agent_addr, thread_id, scope, consent_level, node_count) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(user_addr)
        .bind(agent_addr)
        .bind(thread_id)
        .bind("minimal")
        .bind(consent_level.as_str())
        .bind(0i32)
        .execute(db)
        .await;

        return Ok(serde_json::json!({
            "briefing_id": briefing_id,
            "thread_id": thread_id,
            "user_identity": user_addr,
            "generated_at": protocol::now_rfc3339(),
            "scope": "minimal",
            "consent_level": consent_level.as_str(),
            "sections": serde_json::Map::<String, serde_json::Value>::new(),
            "graph_snapshot": serde_json::Value::Null,
            "peer_card": "",
            "char_budget_used": 0,
        }));
    }

    // --- hybrid retrieval: query as BOOST, not filter (within the consent-allowed categories) ---

    // 1) Broad candidate set: all CURRENT facts within the allowed categories.
    let candidates = graph::query_nodes(
        db, user_addr, None, None, Some(allowed.as_slice()), "current", 0.0, 200,
    ).await.map_err(|e| AppError::Storage(e))?;
```

Then scope the boost query. Change (currently ~line 237):

```rust
        Some(q) => graph::query_nodes(db, user_addr, Some(q), None, None, "current", 0.0, 200)
```

to:

```rust
        Some(q) => graph::query_nodes(db, user_addr, Some(q), None, Some(allowed.as_slice()), "current", 0.0, 200)
```

Leave everything else (peer card, sections, graph_snapshot, briefing_log, JSON assembly) unchanged in this task.

- [ ] **Step 4: Run the tests to verify they pass**

Run each: `cargo test -p ainm-demo --test integration briefing_minimal_scope_shows_only_identity_categories -- --test-threads=1` (and the other three names).
Expected: PASS (with Postgres up). Also `cargo build -p ainm-demo` clean.

- [ ] **Step 5: Commit**

```bash
git add demo/src/briefing.rs demo/tests/integration.rs
git commit -m "feat(memory): enforce consent scopes in briefing retrieval (+empty short-circuit)"
```

---

### Task 2: Gate the peer card by the identity scope

**Files:**
- Modify: `demo/src/briefing.rs` (peer-card block, lines ~255-263)
- Test: `demo/tests/integration.rs` (one new test)

- [ ] **Step 1: Write the failing test**

Append to `demo/tests/integration.rs`:

```rust
#[tokio::test]
async fn briefing_peer_card_gated_by_identity_scope() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("pcs_pc");
    let agent = unique_addr("pcs_pc_agent");
    let prov = serde_json::json!({});
    let tags = serde_json::json!([]);
    graph::insert_node(&pool, &user, "semantic", "fact", "Name is Dana", &serde_json::json!({}), 0.95, &prov, &tags).await.unwrap();
    graph::insert_node(&pool, &user, "procedural", "pattern", "Works in sprints", &serde_json::json!({}), 0.85, &prov, &tags).await.unwrap();

    // Per-agent grant of ONLY the patterns scope — no identity.
    briefing::grant_scopes(&pool, &user, &agent, &["patterns".to_string()], ConsentLevel::Full).await.unwrap();

    let b = briefing::generate_briefing(&pool, &user, &agent, "pcs_t_pc", ConsentLevel::Standard).await.unwrap();
    let sections = b.get("sections").unwrap().as_object().unwrap();
    assert!(sections.contains_key("pattern"), "granted patterns scope sees pattern facts");
    assert!(!sections.contains_key("fact"), "no identity scope → identity 'fact' excluded");
    assert_eq!(b.get("peer_card").unwrap().as_str().unwrap(), "",
        "peer card must be empty when the identity scope is not granted");
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p ainm-demo --test integration briefing_peer_card_gated_by_identity_scope -- --test-threads=1`
Expected: FAIL on the `peer_card == ""` assertion — the peer card currently queries identity categories unconditionally, so it populates "Name is Dana" even without the identity scope. (`fact` exclusion already passes thanks to Task 1.)

- [ ] **Step 3: Gate the peer card**

In `demo/src/briefing.rs`, replace the peer-card block (currently ~lines 255-263):

```rust
    // 4) Peer card from identity facts (separate query, unchanged behavior).
    // Peer card draws from the identity scope's real categories (fact, relationship).
    // The compiler never emits a literal "identity" category — querying it returned nothing.
    let identity_cats: Vec<&str> = scope_to_categories("identity");
    let identity_nodes = graph::query_nodes(
        db, user_addr, None, None, Some(&identity_cats), "current", 0.0, 20,
    ).await.unwrap_or_default();
    let identity_labels: Vec<String> = identity_nodes.iter().map(|n| n.label.clone()).collect();
    let peer_card = retrieval::build_peer_card(&identity_labels, 400);
```

with:

```rust
    // 4) Peer card from identity facts — gated by the identity scope (full consent enforcement).
    // Categories fact/relationship; the compiler never emits a literal "identity" category.
    let peer_card = if scopes.iter().any(|s| s.as_str() == "identity") {
        let identity_cats: Vec<&str> = scope_to_categories("identity");
        let identity_nodes = graph::query_nodes(
            db, user_addr, None, None, Some(&identity_cats), "current", 0.0, 20,
        ).await.unwrap_or_default();
        let identity_labels: Vec<String> = identity_nodes.iter().map(|n| n.label.clone()).collect();
        retrieval::build_peer_card(&identity_labels, 400)
    } else {
        String::new()
    };
```

- [ ] **Step 4: Run it to verify it passes**

Run: `cargo test -p ainm-demo --test integration briefing_peer_card_gated_by_identity_scope -- --test-threads=1`
Expected: PASS. `cargo build -p ainm-demo` clean.

- [ ] **Step 5: Commit**

```bash
git add demo/src/briefing.rs demo/tests/integration.rs
git commit -m "feat(memory): gate peer card by the identity scope"
```

---

### Task 3: Reconcile pre-existing briefing tests + full suite

**Files:**
- Modify: `demo/tests/integration.rs` (`test_briefing_minimal_consent`, ~lines 338-353)

- [ ] **Step 1: Strengthen `test_briefing_minimal_consent` to assert enforcement**

The current test (seeds a `fact` and a `skill`, briefs at Minimal) asserts `!sections.contains_key("expertise")` — which passes vacuously because sections key on *categories* (`skill`), never the scope name `expertise`. Under enforcement the `skill` fact is now actually excluded. Replace the assertion block at the end of `test_briefing_minimal_consent`:

```rust
    let sections = briefing.get("sections").unwrap().as_object().unwrap();
    // Minimal consent only shares identity (fact category) — not skills
    assert!(!sections.contains_key("expertise"));
```

with:

```rust
    let sections = briefing.get("sections").unwrap().as_object().unwrap();
    // Minimal consent shares only the identity scope's categories (fact/relationship) —
    // the seeded 'skill' (expertise scope) is excluded under enforcement.
    assert!(sections.contains_key("fact"), "identity 'fact' present at minimal: {sections:?}");
    assert!(!sections.contains_key("skill"), "expertise 'skill' excluded at minimal");
```

- [ ] **Step 2: Confirm `test_generate_briefing` still holds**

Read `test_generate_briefing` (~lines 316-336): it seeds `fact`, `preference`, `skill` and briefs at `Standard`. Standard includes all three categories, so `sections` is non-empty and its existing assertion still passes. No change needed — just verify by reading; do not edit.

- [ ] **Step 3: Run the full suite**

Run: `cargo test -p ainm-demo`
Expected: all pass EXCEPT the two known pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`). With Postgres up, the new consent-scoping tests and the strengthened minimal test pass. If Postgres is down, DB-backed tests `return` early — say so explicitly.

- [ ] **Step 4: Commit**

```bash
git add demo/tests/integration.rs
git commit -m "test(memory): strengthen minimal-consent test for enforced scoping"
```

---

## Final verification

- [ ] `cargo test -p ainm-demo` green except the two known pre-existing failures.
- [ ] Leak guard works: in `briefing_unmapped_scope_returns_empty_without_leak`, the seeded "Secret name" fact is absent from the serialized briefing.
- [ ] Spot-check the empty short-circuit is real: temporarily change the `if allowed.is_empty()` body to fall through (delete the early return) and confirm `briefing_unmapped_scope_returns_empty_without_leak` FAILS (proving the short-circuit, not luck, is what closes the leak), then restore it.
- [ ] `cargo build -p ainm-demo` clean.
- [ ] Hand off to `superpowers:finishing-a-development-branch`.
