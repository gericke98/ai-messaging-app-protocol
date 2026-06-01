# Compound-Memory Capture Layer (MVP) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a fresh agent visibly more useful by upgrading the AINM server's memory **briefing** from an undifferentiated category dump into a query-conditioned, ranked, budget-bounded slice with an always-on peer card — reusing the existing capture + compile + storage pipeline as-is.

**Architecture:** Capture (`store_and_maybe_compile`), compilation (`compiler.rs`), and storage (`memory_nodes` via `graph.rs`) already exist and are reused untouched. The new work lives in a small pure-logic module `demo/src/retrieval.rs` (query-string building, recency×confidence ranking, token-budget packing, peer-card assembly — all unit-tested without a DB) plus wiring those into `demo/src/briefing.rs::generate_briefing` (integration-tested against Postgres). Retrieval uses the existing Postgres FTS (`search_vector`); pgvector is deferred.

**Tech Stack:** Rust (edition 2021), axum 0.8, sqlx 0.8 (Postgres), tokio, chrono, serde_json. Tests: `cargo test` (pure unit tests need no DB; integration tests use the dev Postgres from `demo/docker-compose.yml`).

---

## Grounding facts (verified against current code)

- Server crate: `demo/`. Lib modules (`demo/src/lib.rs`): `briefing, compiler, config, db, error, graph, llm, protocol, repository`. **`server.rs`/`main.rs` are binary-only** — so all testable logic must live in lib modules (`retrieval`, `briefing`, `graph`).
- `briefing::generate_briefing(db, user_addr, agent_addr, thread_id, default_consent) -> Result<serde_json::Value, AppError>` (`briefing.rs:206`). Today it calls `graph::query_nodes(db, user_addr, None /*text_query*/, None, Some(&allowed_categories), "current", 0.0, 200)` (`briefing.rs:228`) — **no query-conditioning, no ranking, no budget, no peer card.**
- `graph::query_nodes(pool, user_addr, text_query: Option<&str>, node_type: Option<&str>, categories: Option<&[String]>, validity: &str, min_confidence: f32, limit: i64) -> Result<Vec<NodeRow>, StorageError>` (`graph.rs:191`); builds a tsquery against `search_vector` when `text_query` is `Some`.
- `graph::NodeRow` (`graph.rs:7`): fields `id: Uuid, user_addr: String, node_type: String, category: String, label: String, properties: serde_json::Value, confidence: f32, valid_from: DateTime<Utc>, valid_until: Option<DateTime<Utc>>, superseded_by: Option<Uuid>, provenance: serde_json::Value, tags: serde_json::Value, created_at, updated_at`. `NodeRow::to_protocol_json()` (`graph.rs:52`).
- `messages` table (`migrations/001`): `thread_id TEXT, sequence BIGINT, from_addr TEXT, content JSONB, created_at TIMESTAMPTZ`. Message text lives in `content` (JSONB) — connectors send `{ "text": "..." }`-style content; extract defensively.
- Tests: `demo/tests/integration.rs` has `test_pool() -> Option<PgPool>` (reads `TEST_DATABASE_URL`/`AINM_DATABASE__URL`/`postgres://ainm:dev_password@localhost:5432/ainm`, runs migrations, returns `None` to skip if unreachable). Integration tests import the lib directly (`ainm_demo::graph`, etc.).
- Postgres for integration tests: `cd demo && docker compose up -d postgres` (image `postgres:16`, db `ainm`, user `ainm`, password `dev_password`, port 5432).

---

## Task 1: Branch + `retrieval` module scaffold

**Files:**
- Modify: `demo/src/lib.rs`
- Create: `demo/src/retrieval.rs`

- [ ] **Step 1: Create the working branch**

Run:
```bash
git checkout main && git checkout -b feat/memory-briefing-retrieval
```
Expected: `Switched to a new branch 'feat/memory-briefing-retrieval'`

- [ ] **Step 2: Create `demo/src/retrieval.rs` with a module doc and empty test module**

```rust
//! Pure retrieval helpers for memory briefing: query-string building,
//! recency×confidence ranking, token-budget packing, peer-card assembly.
//! No DB or network here — all functions are pure and unit-tested.

#[cfg(test)]
mod tests {}
```

- [ ] **Step 3: Register the module in `demo/src/lib.rs`**

Add `pub mod retrieval;` alongside the existing `pub mod` declarations (keep alphabetical order: after `pub mod repository;` is fine, or wherever the list places it).

- [ ] **Step 4: Verify it compiles**

Run: `cd demo && cargo build`
Expected: builds with no errors (a warning about the empty test module is fine).

- [ ] **Step 5: Commit**

```bash
git add demo/src/retrieval.rs demo/src/lib.rs
git commit -m "feat(memory): scaffold retrieval module"
```

---

## Task 2: `build_query_string` — turn recent messages into an FTS-safe query

**Files:**
- Modify: `demo/src/retrieval.rs`

Builds a `to_tsquery`-safe OR-query from recent message texts: extract alphanumeric terms (lowercased, deduped, length ≥ 3, drop a small stopword set), join with ` | `. OR maximizes recall and is safe for `to_tsquery` (no multi-word syntax errors). Returns `None` when no usable terms (caller then falls back to the unconditioned path).

- [ ] **Step 1: Write the failing test** (append inside `mod tests`)

```rust
use super::*;

#[test]
fn build_query_string_extracts_or_joined_terms() {
    let msgs = vec![
        "I want help planning my Rust backend migration".to_string(),
        "the DB is Postgres".to_string(),
    ];
    let q = build_query_string(&msgs).unwrap();
    // OR-joined, lowercased, stopwords/short words dropped, deduped
    for term in ["rust", "backend", "migration", "postgres"] {
        assert!(q.contains(term), "expected `{term}` in `{q}`");
    }
    assert!(q.contains('|'));
    assert!(!q.contains("the")); // stopword/short dropped
}

#[test]
fn build_query_string_none_when_no_terms() {
    assert!(build_query_string(&["a an the".to_string()]).is_none());
    assert!(build_query_string(&[]).is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd demo && cargo test retrieval::tests::build_query_string`
Expected: FAIL — `cannot find function build_query_string`.

- [ ] **Step 3: Implement** (add above `mod tests`)

```rust
const STOPWORDS: &[&str] = &[
    "the", "and", "for", "you", "your", "are", "but", "with", "this", "that",
    "have", "was", "not", "can", "will", "what", "how", "why", "who", "our",
];

/// Build a `to_tsquery`-safe OR query from recent message texts.
/// Returns None if no usable terms remain.
pub fn build_query_string(messages: &[String]) -> Option<String> {
    let mut seen = std::collections::BTreeSet::new();
    for m in messages {
        for raw in m.split(|c: char| !c.is_alphanumeric()) {
            let term = raw.to_lowercase();
            if term.len() >= 3 && !STOPWORDS.contains(&term.as_str()) {
                seen.insert(term);
            }
        }
    }
    if seen.is_empty() {
        return None;
    }
    Some(seen.into_iter().collect::<Vec<_>>().join(" | "))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd demo && cargo test retrieval::tests::build_query_string`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add demo/src/retrieval.rs
git commit -m "feat(memory): build FTS-safe query string from recent messages"
```

---

## Task 3: `score_node` — recency × confidence ranking

**Files:**
- Modify: `demo/src/retrieval.rs`

Pure score for a candidate fact: `confidence * recency_weight`, where recency decays with age in days (half-life ~30 days). Operates on primitives so tests need no `NodeRow`.

- [ ] **Step 1: Write the failing test** (append inside `mod tests`)

```rust
use chrono::{TimeZone, Utc};

#[test]
fn score_node_newer_outranks_older_at_equal_confidence() {
    let now = Utc.with_ymd_and_hms(2026, 5, 30, 0, 0, 0).unwrap();
    let recent = Utc.with_ymd_and_hms(2026, 5, 29, 0, 0, 0).unwrap();
    let old = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    assert!(score_node(recent, now, 0.9) > score_node(old, now, 0.9));
}

#[test]
fn score_node_higher_confidence_outranks_at_equal_age() {
    let now = Utc.with_ymd_and_hms(2026, 5, 30, 0, 0, 0).unwrap();
    let t = Utc.with_ymd_and_hms(2026, 5, 20, 0, 0, 0).unwrap();
    assert!(score_node(t, now, 0.9) > score_node(t, now, 0.4));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd demo && cargo test retrieval::tests::score_node`
Expected: FAIL — `cannot find function score_node`.

- [ ] **Step 3: Implement** (add above `mod tests`)

```rust
use chrono::{DateTime, Utc};

const RECENCY_HALF_LIFE_DAYS: f64 = 30.0;

/// Rank score for a fact: confidence weighted by exponential recency decay.
pub fn score_node(valid_from: DateTime<Utc>, now: DateTime<Utc>, confidence: f32) -> f32 {
    let age_days = (now - valid_from).num_seconds().max(0) as f64 / 86_400.0;
    let recency = 0.5_f64.powf(age_days / RECENCY_HALF_LIFE_DAYS);
    (confidence as f64 * recency) as f32
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd demo && cargo test retrieval::tests::score_node`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add demo/src/retrieval.rs
git commit -m "feat(memory): recency x confidence node scoring"
```

---

## Task 4: `pack_to_budget` — greedy fill under a character budget

**Files:**
- Modify: `demo/src/retrieval.rs`

Given scored items `(id, score, char_len)`, return the ids to keep: sort by score desc, greedily add while cumulative `char_len` ≤ budget. Char length is the token proxy (avoids a tokenizer dependency for the MVP).

- [ ] **Step 1: Write the failing test** (append inside `mod tests`)

```rust
#[test]
fn pack_to_budget_keeps_highest_scores_within_budget() {
    let items = vec![
        RankedItem { id: 1, score: 0.2, char_len: 50 },
        RankedItem { id: 2, score: 0.9, char_len: 50 },
        RankedItem { id: 3, score: 0.5, char_len: 50 },
    ];
    // budget fits 2 items (100 chars)
    let kept = pack_to_budget(items, 100);
    assert_eq!(kept, vec![2, 3]); // highest scores first, in score order
}

#[test]
fn pack_to_budget_skips_item_that_would_overflow_but_keeps_later_smaller_one() {
    let items = vec![
        RankedItem { id: 1, score: 0.9, char_len: 80 },
        RankedItem { id: 2, score: 0.8, char_len: 50 }, // would overflow (130>100)
        RankedItem { id: 3, score: 0.7, char_len: 15 }, // fits (95<=100)
    ];
    let kept = pack_to_budget(items, 100);
    assert_eq!(kept, vec![1, 3]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd demo && cargo test retrieval::tests::pack_to_budget`
Expected: FAIL — `cannot find type RankedItem` / `function pack_to_budget`.

- [ ] **Step 3: Implement** (add above `mod tests`)

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct RankedItem {
    pub id: i64,
    pub score: f32,
    pub char_len: usize,
}

/// Greedy budget fill: sort by score desc, keep while cumulative char_len <= budget.
/// Returns kept ids in score order.
pub fn pack_to_budget(mut items: Vec<RankedItem>, budget_chars: usize) -> Vec<i64> {
    items.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let mut used = 0usize;
    let mut kept = Vec::new();
    for it in items {
        if used + it.char_len <= budget_chars {
            used += it.char_len;
            kept.push(it.id);
        }
    }
    kept
}
```

> Note: `RankedItem.id` is `i64` to keep this helper DB-type-agnostic; in `briefing.rs` we map each `NodeRow` to a stable index (its position in the candidate vec), not its UUID — see Task 7.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd demo && cargo test retrieval::tests::pack_to_budget`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add demo/src/retrieval.rs
git commit -m "feat(memory): greedy token-budget packing"
```

---

## Task 5: `build_peer_card` — capped identity anchor

**Files:**
- Modify: `demo/src/retrieval.rs`

Assemble a tiny always-injected identity summary from identity/preference fact labels, capped at a character budget (truncate the list, never mid-fact). Returns a short string.

- [ ] **Step 1: Write the failing test** (append inside `mod tests`)

```rust
#[test]
fn build_peer_card_joins_facts_under_cap() {
    let facts = vec![
        "Name is Santiago".to_string(),
        "Prefers Rust".to_string(),
        "Building AINM".to_string(),
    ];
    let card = build_peer_card(&facts, 200);
    assert!(card.contains("Santiago") && card.contains("Rust") && card.contains("AINM"));
}

#[test]
fn build_peer_card_truncates_whole_facts_to_cap() {
    let facts = vec!["a".repeat(40), "b".repeat(40), "c".repeat(40)];
    let card = build_peer_card(&facts, 90); // fits ~2 facts incl bullets/newlines
    assert!(card.len() <= 90);
    assert!(card.contains(&"a".repeat(40))); // first fact always kept if it fits
}

#[test]
fn build_peer_card_empty_when_no_facts() {
    assert_eq!(build_peer_card(&[], 200), "");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd demo && cargo test retrieval::tests::build_peer_card`
Expected: FAIL — `cannot find function build_peer_card`.

- [ ] **Step 3: Implement** (add above `mod tests`)

```rust
/// Build a capped, human-readable identity anchor from fact labels.
/// Adds facts as "- <label>\n" while staying within `cap_chars`. Empty if no facts fit.
pub fn build_peer_card(facts: &[String], cap_chars: usize) -> String {
    let mut out = String::new();
    for f in facts {
        let line = format!("- {f}\n");
        if out.len() + line.len() <= cap_chars {
            out.push_str(&line);
        }
    }
    out.trim_end().to_string()
}
```

> Note: the test cap of 90 expects the trailing newline trim to bring the final length within budget; `build_peer_card` adds lines whose cumulative length (incl. `- ` and `\n`) stays ≤ cap, then trims the final newline.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd demo && cargo test retrieval::tests::build_peer_card`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add demo/src/retrieval.rs
git commit -m "feat(memory): capped peer-card assembly"
```

---

## Task 6: `fetch_recent_thread_texts` — pull recent message text for query-conditioning

**Files:**
- Modify: `demo/src/graph.rs`
- Test: `demo/tests/integration.rs`

Add a DB helper that returns the text of the most recent N messages in a thread (newest first), extracting the `text` field from the `content` JSONB defensively.

- [ ] **Step 1: Write the failing integration test** (append to `demo/tests/integration.rs`)

```rust
#[tokio::test]
async fn fetch_recent_thread_texts_returns_newest_first() {
    let Some(pool) = test_pool().await else { return; };
    let thread = unique_addr(); // reuse the helper; unique thread id
    sqlx::query("INSERT INTO threads (id) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(&thread).execute(&pool).await.unwrap();
    for (seq, txt) in [(1i64, "hello rust"), (2, "postgres migration")] {
        sqlx::query("INSERT INTO messages (thread_id, sequence, from_addr, content) VALUES ($1,$2,$3,$4)")
            .bind(&thread).bind(seq).bind("u@a")
            .bind(serde_json::json!({"text": txt}))
            .execute(&pool).await.unwrap();
    }
    let texts = ainm_demo::graph::fetch_recent_thread_texts(&pool, &thread, 10).await.unwrap();
    assert_eq!(texts.len(), 2);
    assert_eq!(texts[0], "postgres migration"); // newest first
    assert!(texts.contains(&"hello rust".to_string()));
}
```

> If `threads` requires more NOT NULL columns than `id`, adjust the insert to match `migrations/001` (check the `threads` DDL; add required columns). The test skips silently when no Postgres is reachable.

- [ ] **Step 2: Run test to verify it fails**

Run: `cd demo && docker compose up -d postgres && cargo test --test integration fetch_recent_thread_texts`
Expected: FAIL — `no function or associated item named fetch_recent_thread_texts`.

- [ ] **Step 3: Implement** (add to `demo/src/graph.rs`)

```rust
/// Return the text of the most recent `limit` messages in a thread, newest first.
/// Extracts the `text` field from the `content` JSONB; skips messages without text.
pub async fn fetch_recent_thread_texts(
    pool: &sqlx::PgPool,
    thread_id: &str,
    limit: i64,
) -> Result<Vec<String>, StorageError> {
    let rows: Vec<(serde_json::Value,)> = sqlx::query_as(
        "SELECT content FROM messages WHERE thread_id = $1 ORDER BY sequence DESC LIMIT $2",
    )
    .bind(thread_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(content,)| {
            content.get("text").and_then(|v| v.as_str()).map(|s| s.to_string())
        })
        .collect())
}
```

> `StorageError` already implements `From<sqlx::Error>` (used by the other functions in this file — confirm and mirror the existing error handling; if `query_as` needs an explicit type, the tuple `(serde_json::Value,)` form above works with sqlx 0.8).

- [ ] **Step 4: Run test to verify it passes**

Run: `cd demo && cargo test --test integration fetch_recent_thread_texts`
Expected: PASS (1 test) — or SKIP (returns early) if Postgres unreachable; ensure Postgres is up so it actually runs.

- [ ] **Step 5: Commit**

```bash
git add demo/src/graph.rs demo/tests/integration.rs
git commit -m "feat(memory): fetch recent thread message texts"
```

---

## Task 7: Wire query-conditioned retrieval + ranking + budget + peer card into `generate_briefing`

**Files:**
- Modify: `demo/src/briefing.rs`
- Test: `demo/tests/integration.rs`

Replace the unconditioned `query_nodes(..., None, ..., 200)` retrieval with: derive a query string from recent thread messages → fetch a candidate pool with that `text_query` → rank by `score_node` → pack to a char budget → build a peer card from identity-category facts → add a `peer_card` field and a `token_budget_used` (char count) to the returned JSON, and build `sections` from the packed nodes. On empty query (no usable terms) fall back to the existing unconditioned retrieval so behavior never regresses.

- [ ] **Step 1: Write the failing integration test** (append to `demo/tests/integration.rs`)

```rust
#[tokio::test]
async fn briefing_is_query_conditioned_and_has_peer_card() {
    let Some(pool) = test_pool().await else { return; };
    let user = unique_addr();
    let thread = unique_addr();
    sqlx::query("INSERT INTO threads (id) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(&thread).execute(&pool).await.unwrap();
    // a recent message about Rust drives the query
    sqlx::query("INSERT INTO messages (thread_id, sequence, from_addr, content) VALUES ($1,1,$2,$3)")
        .bind(&thread).bind(&user).bind(serde_json::json!({"text": "help with my rust backend"}))
        .execute(&pool).await.unwrap();
    // one relevant fact + one identity fact + one irrelevant fact
    let prov = serde_json::json!({});
    let tags = serde_json::json!([]);
    ainm_demo::graph::insert_node(&pool, &user, "semantic", "preferences",
        "Prefers Rust for backend services", &serde_json::json!({}), 0.9, &prov, &tags).await.unwrap();
    ainm_demo::graph::insert_node(&pool, &user, "semantic", "identity",
        "Name is Santiago", &serde_json::json!({}), 0.95, &prov, &tags).await.unwrap();
    ainm_demo::graph::insert_node(&pool, &user, "semantic", "preferences",
        "Enjoys hiking on weekends", &serde_json::json!({}), 0.9, &prov, &tags).await.unwrap();

    let briefing = ainm_demo::briefing::generate_briefing(
        &pool, &user, "agent@x", &thread, ainm_demo::briefing::ConsentLevel::Standard,
    ).await.unwrap();

    // peer card present and includes identity
    let card = briefing.get("peer_card").and_then(|v| v.as_str()).unwrap_or("");
    assert!(card.contains("Santiago"), "peer_card should contain identity: {card}");
    // query-conditioned: the Rust fact is surfaced (serialize whole briefing and check)
    let blob = serde_json::to_string(&briefing).unwrap();
    assert!(blob.contains("Prefers Rust"), "relevant fact should be injected");
    assert!(briefing.get("token_budget_used").is_some());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd demo && cargo test --test integration briefing_is_query_conditioned`
Expected: FAIL — no `peer_card` field (current briefing has no peer card / query-conditioning).

- [ ] **Step 3: Implement** — edit `generate_briefing` in `demo/src/briefing.rs`

At the top of the function add `use crate::retrieval;`. Replace the existing retrieval block (around `briefing.rs:228`, the `graph::query_nodes(db, user_addr, None, None, Some(&allowed_categories), "current", 0.0, 200)` call) with:

```rust
    // --- query-conditioned retrieval (MVP) ---
    let recent = graph::fetch_recent_thread_texts(db, thread_id, 8).await.unwrap_or_default();
    let query = retrieval::build_query_string(&recent);

    // candidate pool: query-conditioned when we have terms, else unconditioned fallback
    let candidates = graph::query_nodes(
        db, user_addr,
        query.as_deref(),                 // Some(query) → FTS; None → all (no regression)
        None,
        Some(&allowed_categories),
        "current", 0.0, 200,
    ).await?;

    // rank by recency x confidence, pack under a char budget
    let now = chrono::Utc::now();
    let ranked: Vec<retrieval::RankedItem> = candidates.iter().enumerate().map(|(i, n)| {
        retrieval::RankedItem {
            id: i as i64,
            score: retrieval::score_node(n.valid_from, now, n.confidence),
            char_len: n.label.len(),
        }
    }).collect();
    const BRIEFING_CHAR_BUDGET: usize = 4000;
    let kept_idx = retrieval::pack_to_budget(ranked, BRIEFING_CHAR_BUDGET);
    let nodes: Vec<&graph::NodeRow> = kept_idx.iter().map(|&i| &candidates[i as usize]).collect();

    // peer card from identity facts among ALL candidates (not budget-limited)
    let identity_labels: Vec<String> = candidates.iter()
        .filter(|n| n.category == "identity")
        .map(|n| n.label.clone())
        .collect();
    let peer_card = retrieval::build_peer_card(&identity_labels, 400);
    let token_budget_used: usize = nodes.iter().map(|n| n.label.len()).sum();
```

Then, where the function currently buckets `nodes` into `sections` (around `briefing.rs:249-326`), make it iterate over the new `nodes: Vec<&NodeRow>` (deref as needed: `for n in &nodes { ... n.category ... n.to_protocol_json() ... }`). Finally, in the returned JSON object (around `briefing.rs:359`), add two fields:

```rust
        "peer_card": peer_card,
        "token_budget_used": token_budget_used,
```

> Implementation notes for the engineer:
> - The existing code may bind `nodes` as `Vec<NodeRow>`; you are switching to `Vec<&NodeRow>`. Adjust the section-building loop's borrows accordingly (use `n.category.as_str()`, call `n.to_protocol_json()` on the reference). If easier, collect owned clones: `let nodes: Vec<graph::NodeRow> = kept_idx.iter().map(|&i| candidates[i as usize].clone()).collect();` (NodeRow derives `Serialize/Deserialize`; add `#[derive(Clone)]` to `NodeRow` in `graph.rs` if it isn't already — that is an acceptable, minimal change).
> - Keep the existing `briefing_log` insert and `graph_snapshot` logic intact.
> - `cargo build` and fix any borrow/type errors before running the test.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd demo && cargo test --test integration briefing_is_query_conditioned`
Expected: PASS (1 test).

- [ ] **Step 5: Run the full lib + integration suite to confirm no regressions**

Run: `cd demo && cargo test`
Expected: existing tests still pass; new retrieval unit tests + briefing integration test pass. (Integration tests that need Postgres run because it's up; otherwise they skip.)

- [ ] **Step 6: Commit**

```bash
git add demo/src/briefing.rs demo/src/graph.rs demo/tests/integration.rs
git commit -m "feat(memory): query-conditioned, ranked, budgeted briefing with peer card"
```

---

## Task 8: Dogfood validation (manual — the value test)

**Files:**
- Create: `docs/superpowers/notes/2026-05-30-memory-briefing-verdict.md` (the deliverable)

> Requires a running server + Postgres + at least one connector with real conversation history. Do this WITH the user.

- [ ] **Step 1: Bring up the stack and seed real conversation**

Run `cd demo && docker compose up -d` (Postgres + servers). Connect a connector (e.g. `packages/connector-claude-code`) and hold several real conversations so the compiler populates `memory_nodes` (verify: `psql ... -c "SELECT category, label FROM memory_nodes LIMIT 20;"`).

- [ ] **Step 2: Capture two briefings for the same new task**

For a real task thread, capture the briefing produced by the new code (`peer_card` + ranked facts). For the baseline, temporarily compare against the pre-change behavior (git stash the Task 7 change, or set `BRIEFING_CHAR_BUDGET` huge + query `None`) to get the old undifferentiated dump. Save both.

- [ ] **Step 3: Judge and write the verdict**

In `docs/superpowers/notes/2026-05-30-memory-briefing-verdict.md`: for the real task, score the agent's response **with the new ranked/peer-card briefing vs the old dump vs no briefing** on the spike's rubric (usefulness from msg one · specificity · correct assumptions · no hallucinated facts), note the briefing's `token_budget_used`, spot-check 5 facts' provenance for fabrication, and write a one-paragraph **confirmed / weak / killed** verdict for whether query-conditioned ranked memory beats the dump.

- [ ] **Step 4: Commit the verdict**

```bash
git add docs/superpowers/notes/2026-05-30-memory-briefing-verdict.md
git commit -m "docs(memory): briefing value-test verdict"
```

---

## Self-Review

**Spec coverage:**
- Reuse capture/compile/storage as-is (spec §2 correction, §4.1–2) → Tasks reuse them; only `graph.rs`/`briefing.rs`/`retrieval.rs` touched. ✓
- Query-conditioned retrieval via FTS (spec §4.3, §6) → Tasks 2, 6, 7. ✓
- Ranking relevance×recency×confidence (spec §3 Tier 2, §4.3) → Task 3 (recency×confidence; FTS provides relevance filtering) + Task 7 wiring. ✓
- Token-budget packing, non-destructive (spec §3, §6 bounding) → Task 4 + Task 7. ✓
- Peer-card anchor (spec §4.4) → Task 5 + Task 7. ✓
- Inject via existing `ainm.memory.briefing`, no client change (spec §4.5) → Task 7 returns extra JSON fields; connector reads raw object. ✓
- pgvector deferred, FTS now (spec §6) → no pgvector task; FTS used. ✓
- Dogfood value test, confirmed/weak/killed (spec §7, §10) → Task 8. ✓
- Per-agent consent deferred (spec §6) → not built; consent stays hardcoded `Standard`. ✓ (intentional)

**Placeholder scan:** No TBD/TODO. The one judgement call left to the engineer (Task 7 borrow/clone of `nodes`) is spelled out with two concrete options + the exact `#[derive(Clone)]` fallback — not a vague "handle it." Task 6's `threads` insert has an explicit "adjust to match DDL" instruction with the reason.

**Type consistency:** `build_query_string(&[String]) -> Option<String>`, `score_node(DateTime<Utc>, DateTime<Utc>, f32) -> f32`, `RankedItem{id:i64,score:f32,char_len:usize}`, `pack_to_budget(Vec<RankedItem>, usize) -> Vec<i64>`, `build_peer_card(&[String], usize) -> String`, `fetch_recent_thread_texts(&PgPool,&str,i64) -> Result<Vec<String>,StorageError>` — all used consistently in Task 7's wiring. `query_nodes` called with the real signature from grounding (`text_query: Option<&str>` via `query.as_deref()`). `generate_briefing` signature unchanged (additive JSON fields only), so the connector and handler are untouched.

**Known risk carried (documented, not a defect):** Task 7 assumes `generate_briefing`'s current `sections`-building loop can be repointed at the packed node list with modest borrow adjustments; the engineer must `cargo build` and resolve borrows (clone fallback provided). FTS query quality is validated empirically in Task 8 — if weak, the spec's deferred pgvector path is the escalation.
