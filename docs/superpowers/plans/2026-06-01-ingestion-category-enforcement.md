# Ingestion-Side Category Enforcement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the compiler reject off-list categories at ingestion so `memory_nodes` only ever holds canonical, consent-mappable categories.

**Architecture:** Add a pure `categories::normalize` helper that trims + ASCII-lowercases a raw category and membership-tests it against the closed canonical set, returning the canonical `&'static str` or `None`. In `compiler::run_compilation`, immediately after LLM extraction, normalize-and-filter the candidate nodes: in-set nodes are stored with their canonical category, off-list nodes are dropped with a `tracing::warn!`. Edges to dropped nodes are skipped naturally because `integrate_edges` resolves endpoints by label through the surviving-node map.

**Tech Stack:** Rust 2024, sqlx/Postgres, tokio, tracing. Tests via `cargo test` (DB-backed integration tests gated on a test pool).

**Spec:** `docs/superpowers/specs/2026-06-01-ingestion-category-enforcement-design.md`

---

### Task 1: `categories::normalize` helper + unit tests

**Files:**
- Modify: `demo/src/categories.rs` (add `normalize` fn after `all()` at line 23; add unit tests in the existing `#[cfg(test)] mod tests` block)

- [ ] **Step 1: Write the failing unit tests**

Add these tests inside the existing `#[cfg(test)] mod tests` block in `demo/src/categories.rs` (after the `all_is_the_union_of_groups_with_no_duplicates` test, before the closing `}` of the module):

```rust
    #[test]
    fn normalize_accepts_exact_canonical() {
        assert_eq!(normalize("fact"), Some("fact"));
        assert_eq!(normalize("decision"), Some("decision"));
        assert_eq!(normalize("communication"), Some("communication"));
    }

    #[test]
    fn normalize_recovers_casing_and_whitespace_variants() {
        assert_eq!(normalize("Fact"), Some("fact"));
        assert_eq!(normalize(" preference "), Some("preference"));
        assert_eq!(normalize("DECISION"), Some("decision"));
        assert_eq!(normalize("Skill"), Some("skill"));
    }

    #[test]
    fn normalize_rejects_off_list_and_empty() {
        assert_eq!(normalize("project"), None);
        assert_eq!(normalize("misc"), None);
        assert_eq!(normalize(""), None);
        assert_eq!(normalize("   "), None);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ainm-demo --lib categories::tests::normalize 2>&1 | tail -20`
(run from the `demo/` directory, or use `--manifest-path demo/Cargo.toml`)
Expected: FAIL to compile — `cannot find function `normalize` in this scope` / `normalize` not found in `categories`.

- [ ] **Step 3: Write the minimal implementation**

Insert this function in `demo/src/categories.rs` immediately after the `all()` function (after its closing `}` on line 23, before the `#[cfg(test)]` block):

```rust
/// Normalize a raw category to its canonical form, or `None` if off-list.
///
/// Trims surrounding whitespace and ASCII-lowercases the input, then
/// membership-tests against the closed canonical set. Returns the canonical
/// `&'static str` literal so callers store the exact vocabulary form rather
/// than the model's casing.
pub fn normalize(raw: &str) -> Option<&'static str> {
    let key = raw.trim().to_ascii_lowercase();
    all().into_iter().find(|&c| c == key.as_str())
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ainm-demo --lib categories::tests 2>&1 | tail -20`
Expected: PASS — all `normalize_*` tests plus the existing `all_is_the_union_of_groups_with_no_duplicates` pass.

- [ ] **Step 5: Commit**

```bash
git add demo/src/categories.rs
git commit -m "feat(categories): add normalize() for ingestion category enforcement

Trims + ASCII-lowercases a raw category and membership-tests it against the
closed canonical set, returning the canonical &'static str or None. Recovers
trivial casing/whitespace variants; off-list categories return None."
```

---

### Task 2: Enforce normalize-and-filter in `run_compilation` + compiler integration test

**Files:**
- Modify: `demo/src/compiler.rs:69-74` (insert normalize-and-filter between extraction and dedup)
- Test: `demo/tests/integration.rs` (add `test_compilation_rejects_off_list_categories` after `test_compilation_with_mock_llm`, ~line 421)

- [ ] **Step 1: Write the failing integration test**

Add this test in `demo/tests/integration.rs` immediately after `test_compilation_with_mock_llm` (after its closing `}` at line 421). It reuses the already-imported `CandidateNode`, `ExtractionResult`, `MockLlmClient`, `CompileRequest`, `ConversationMessage`, `compiler`, `graph`, and the `unique_addr` / `test_pool` helpers used by the surrounding tests.

```rust
#[tokio::test]
async fn test_compilation_rejects_off_list_categories() {
    let Some(pool) = test_pool().await else { return };
    let user = unique_addr("offlist");

    // Three candidate nodes: a valid `fact`, a casing-variant `Skill` (should be
    // normalized + stored as `skill`), and an off-list `project` (must be dropped).
    let mock = MockLlmClient::new(vec![ExtractionResult {
        nodes: vec![
            CandidateNode {
                node_type: "semantic".to_string(),
                category: "fact".to_string(),
                label: "Works at TechTorch".to_string(),
                confidence: 0.9,
                properties: serde_json::json!({}),
                tags: vec![],
            },
            CandidateNode {
                node_type: "semantic".to_string(),
                category: "Skill".to_string(),
                label: "Writes Rust".to_string(),
                confidence: 0.85,
                properties: serde_json::json!({}),
                tags: vec![],
            },
            CandidateNode {
                node_type: "semantic".to_string(),
                category: "project".to_string(),
                label: "Building AINM".to_string(),
                confidence: 0.8,
                properties: serde_json::json!({}),
                tags: vec![],
            },
        ],
        edges: vec![],
    }]);

    let req = CompileRequest {
        user_addr: user.clone(),
        thread_id: "offlist-thread".to_string(),
        messages: vec![
            ConversationMessage { from: user.clone(), content: "I work at TechTorch writing Rust on the AINM project".to_string(), timestamp: "2026-01-01T00:00:00Z".to_string() },
            ConversationMessage { from: "agent@test".to_string(), content: "Sounds like an interesting systems project".to_string(), timestamp: "2026-01-01T00:01:00Z".to_string() },
        ],
    };

    let result = compiler::run_compilation(pool.clone(), Arc::new(mock), req)
        .await.expect("compilation failed");

    // Only the two valid nodes are persisted; the off-list `project` node is dropped.
    assert_eq!(result.nodes_created, 2, "off-list node must not be persisted");

    let nodes = graph::query_nodes(&pool, &user, None, None, None, "current", 0.0, 10).await.unwrap();
    let labels: Vec<&str> = nodes.iter().map(|n| n.label.as_str()).collect();
    assert!(labels.contains(&"Works at TechTorch"), "valid fact node should be stored");
    assert!(labels.contains(&"Writes Rust"), "casing-variant node should be stored");
    assert!(!labels.contains(&"Building AINM"), "off-list `project` node must be absent");

    // The casing-variant node is stored with its NORMALIZED canonical category.
    let skill_node = nodes.iter().find(|n| n.label == "Writes Rust").expect("skill node missing");
    assert_eq!(skill_node.category, "skill", "category must be normalized to canonical lowercase");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p ainm-demo --test integration test_compilation_rejects_off_list_categories 2>&1 | tail -25`
Expected: If a test DB is configured, FAIL on `assert_eq!(result.nodes_created, 2, ...)` (currently 3 — the off-list node is still persisted) and/or the normalized-category assertion (currently stored as `"Skill"`). If no test pool is configured the test early-returns (passes vacuously); in that case rely on Step 4's behavioral reasoning and confirm compilation succeeds.

- [ ] **Step 3: Write the minimal implementation**

In `demo/src/compiler.rs`, replace the extraction + dedup block at lines 69-74:

```rust
    // Step 1: LLM extraction
    let extraction = llm.extract(&req.messages).await?;

    // Step 2: Dedup candidates against existing graph
    let (new_nodes, updated_count, superseded_count) =
        dedup_and_integrate_nodes(&db, &req.user_addr, &req.thread_id, &extraction.nodes).await?;
```

with:

```rust
    // Step 1: LLM extraction
    let extraction = llm.extract(&req.messages).await?;

    // Step 1b: Ingestion-side category enforcement. The extraction prompt
    // instructs the model to use the canonical closed set, but the model may
    // still deviate. Normalize trivial casing/whitespace variance to the
    // canonical form; drop (and log) any node whose category is genuinely
    // off-list so storage only ever holds consent-mappable categories.
    let valid_nodes: Vec<CandidateNode> = extraction
        .nodes
        .into_iter()
        .filter_map(|mut c| match crate::categories::normalize(&c.category) {
            Some(canon) => {
                c.category = canon.to_string();
                Some(c)
            }
            None => {
                tracing::warn!(
                    "[compiler] dropping node with off-list category {:?} (label {:?})",
                    c.category,
                    c.label
                );
                None
            }
        })
        .collect();

    // Step 2: Dedup candidates against existing graph
    let (new_nodes, updated_count, superseded_count) =
        dedup_and_integrate_nodes(&db, &req.user_addr, &req.thread_id, &valid_nodes).await?;
```

Note: `extraction.edges` is still moved (used unchanged) in the subsequent `integrate_edges` call at line 78 — the partial move of `extraction.nodes` via `into_iter()` is fine because `ExtractionResult` has no `Drop` impl. Confirm `CandidateNode` is in scope in `compiler.rs` (it is imported via the `use crate::llm::{... CandidateNode ...}` at the top of the file); if not already imported, add `CandidateNode` to that `use`.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p ainm-demo --test integration test_compilation_rejects_off_list_categories 2>&1 | tail -25`
Expected: PASS (or vacuous pass with no test pool). Also confirm the crate still builds: `cargo build -p ainm-demo 2>&1 | tail -5` → no errors.

- [ ] **Step 5: Run the full suite to check for regressions**

Run: `cargo test -p ainm-demo 2>&1 | tail -30`
Expected: All green EXCEPT the two known pre-existing failures `test_graph_traversal` and `test_compilation_dedup_updates_confidence` (verified failing on `main` — NOT regressions from this work). If any OTHER test fails, it is a regression and must be fixed before committing.

- [ ] **Step 6: Commit**

```bash
git add demo/src/compiler.rs demo/tests/integration.rs
git commit -m "feat(compiler): enforce closed category set at ingestion

run_compilation now normalizes-and-filters candidate nodes after extraction:
in-set nodes are stored with their canonical category, off-list nodes are
dropped with a warn! before reaching storage. Edges to dropped nodes are
skipped naturally (resolved by label through the surviving-node map).
Closes the gap where an off-list category would be stored and then orphaned
from every consent scope."
```

---

## Self-Review

**1. Spec coverage:**
- §4.1 `categories::normalize` → Task 1 (impl + unit tests including casing/whitespace/off-list/empty). ✓
- §4.2 enforce in `run_compilation` (normalize-and-filter, drop+warn, store canonical) → Task 2 Step 3. ✓
- §4.3 edges to dropped nodes skipped → no code needed; documented in Task 2 Step 3 note. ✓
- §5 testing: unit cases (Task 1 Step 1 covers `"Fact"`, `" preference "`, `"DECISION"`, `"fact"`, `"project"`, `"misc"`, `""`) ✓; compiler integration with valid/casing-variant/off-list (Task 2 Step 1) ✓; regression suite (Task 2 Step 5) ✓.
- §8 definition of done: all four items map to Task 1 + Task 2. ✓
- Out of scope (§6: retroactive cleanup, node_type validation) — correctly not in any task. ✓

**2. Placeholder scan:** No TBD/TODO/"handle edge cases"/"similar to". All code blocks complete. ✓

**3. Type consistency:** `normalize(raw: &str) -> Option<&'static str>` defined in Task 1 and called as `crate::categories::normalize(&c.category)` in Task 2 — names and signature match. `CandidateNode` fields (`node_type`, `category`, `label`, `confidence`, `properties`, `tags`) match `demo/src/llm.rs:12-19`. `CompileRequest`, `run_compilation`, `graph::query_nodes` signatures match existing usage in `integration.rs`. `node.category` field accessed in the test matches the stored model. ✓
