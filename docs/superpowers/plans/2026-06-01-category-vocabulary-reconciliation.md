# Category Vocabulary Reconciliation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the compound-memory category vocabulary a single, closed source of truth so the consent `scope → category` map is provably complete, and fix the empty-peer-card and orphaned-`relationship` bugs.

**Architecture:** A new `categories` module holds the canonical closed set (3 entity-type groups → derived `all()`). `briefing.rs::scope_to_categories` maps every scope onto that set (adding `relationship` to `identity`), enforced by a completeness test. The peer card queries the `identity` scope's real categories instead of the bogus literal `"identity"`. The compiler's extraction prompt is built from the module and closed (no `...`). Integration/dogfood test seeds migrate from the legacy display vocabulary to canonical categories. The consent category filter stays OFF.

**Tech Stack:** Rust 2024 (crate `ainm-demo` under `demo/`), `cargo test` / `cargo nextest`, sqlx + Postgres (integration tests need a running DB; they `return` early if none).

**Spec:** `docs/superpowers/specs/2026-06-01-category-vocabulary-reconciliation-design.md`

**Pre-existing failures unrelated to this work (do NOT treat as regressions):** `test_graph_traversal`, `test_compilation_dedup_updates_confidence` (shared-DB test-data bleed; fail on `main` too).

**Canonical migration map (legacy display vocab → canonical category), used in Task 5:**
`identity→fact` · `preferences→preference` · `projects→fact` · `decisions→decision` · `patterns→pattern` · `relationships→relationship`.

---

### Task 1: Canonical vocabulary module

**Files:**
- Create: `demo/src/categories.rs`
- Modify: `demo/src/lib.rs:1` (add `pub mod categories;`)
- Modify: `demo/src/main.rs:2` (add `mod categories;`)

- [ ] **Step 1: Create the module with its unit test**

Create `demo/src/categories.rs`:

```rust
//! Canonical compound-memory category vocabulary — the single source of truth.
//!
//! These are the only categories the compiler may emit. The three entity-type
//! groups are the source; `all()` derives the full closed set from them so they
//! cannot drift apart. `briefing::scope_to_categories` must map every category
//! here (enforced by a completeness test in `briefing.rs`).

/// Stable facts about the user.
pub const SEMANTIC: &[&str] = &["fact", "preference", "skill", "relationship", "tool"];
/// Time-bound events.
pub const EPISODIC: &[&str] = &["event", "decision", "outcome", "interaction"];
/// Learned patterns / procedures.
pub const PROCEDURAL: &[&str] = &["pattern", "workflow", "strategy", "communication"];

/// Every category the compiler may emit (closed set), in stable order.
pub fn all() -> Vec<&'static str> {
    SEMANTIC
        .iter()
        .chain(EPISODIC)
        .chain(PROCEDURAL)
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn all_is_the_union_of_groups_with_no_duplicates() {
        let all = all();
        assert_eq!(
            all.len(),
            SEMANTIC.len() + EPISODIC.len() + PROCEDURAL.len(),
            "all() must be exactly the three groups concatenated"
        );
        assert_eq!(all.len(), 13, "canonical set is 13 categories");
        let unique: BTreeSet<&&str> = all.iter().collect();
        assert_eq!(unique.len(), all.len(), "no category may appear in two groups");
    }
}
```

- [ ] **Step 2: Register the module in both crate roots**

In `demo/src/lib.rs`, add after `pub mod briefing;` (line 1):

```rust
pub mod categories;
```

In `demo/src/main.rs`, add after `mod briefing;` (line 2):

```rust
mod categories;
```

- [ ] **Step 3: Run the test to verify it passes**

Run: `cargo test -p ainm-demo --lib categories::tests::all_is_the_union_of_groups_with_no_duplicates`
Expected: `test result: ok. 1 passed`

- [ ] **Step 4: Verify the whole crate still builds**

Run: `cargo build -p ainm-demo`
Expected: builds with no errors (warnings about unused `categories` items are acceptable until Task 2/4 consume it).

- [ ] **Step 5: Commit**

```bash
git add demo/src/categories.rs demo/src/lib.rs demo/src/main.rs
git commit -m "feat(memory): add canonical category vocabulary module"
```

---

### Task 2: Map `relationship` into `identity` + completeness test

**Files:**
- Modify: `demo/src/briefing.rs:70-80` (`scope_to_categories`)
- Modify: `demo/src/briefing.rs:336+` (existing `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing completeness test**

In `demo/src/briefing.rs`, inside the existing `#[cfg(test)] mod tests { ... }` block (starts at line 337), add:

```rust
    #[test]
    fn scope_map_exactly_covers_the_canonical_set() {
        use std::collections::BTreeSet;
        let mapped: BTreeSet<&str> = SCOPES
            .iter()
            .copied()
            .flat_map(scope_to_categories)
            .collect();
        let canonical: BTreeSet<&str> = crate::categories::all().into_iter().collect();
        assert_eq!(
            mapped, canonical,
            "every canonical category must be reachable by some scope, and no scope may \
             map to a non-canonical category"
        );
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p ainm-demo --lib briefing::tests::scope_map_exactly_covers_the_canonical_set`
Expected: FAIL — `mapped` is missing `"relationship"` (left/right BTreeSets differ).

- [ ] **Step 3: Add `relationship` to the `identity` scope**

In `demo/src/briefing.rs`, change the `identity` arm of `scope_to_categories` (line 72):

```rust
        "identity" => vec!["fact", "relationship"],
```

Leave every other arm unchanged.

- [ ] **Step 4: Update the existing `test_scope_to_categories` expectation**

In the same test module, the existing assertion (line 378) reads:

```rust
        assert_eq!(scope_to_categories("identity"), vec!["fact"]);
```

Change it to:

```rust
        assert_eq!(scope_to_categories("identity"), vec!["fact", "relationship"]);
```

- [ ] **Step 5: Run both tests to verify they pass**

Run: `cargo test -p ainm-demo --lib briefing::tests::scope_map_exactly_covers_the_canonical_set briefing::tests::test_scope_to_categories`
Expected: `test result: ok. 2 passed`

- [ ] **Step 6: Commit**

```bash
git add demo/src/briefing.rs
git commit -m "fix(memory): map relationship into identity scope; lock scope/category completeness"
```

---

### Task 3: Fix the empty peer card

**Files:**
- Modify: `demo/src/briefing.rs:252-254` (peer-card query inside `generate_briefing`)

- [ ] **Step 1: Replace the bogus `"identity"` category filter with the identity scope's real categories**

In `demo/src/briefing.rs`, the current peer-card query (lines 252-254) is:

```rust
    let identity_nodes = graph::query_nodes(
        db, user_addr, None, None, Some(&["identity"]), "current", 0.0, 20,
    ).await.unwrap_or_default();
```

Replace it with:

```rust
    // Peer card draws from the identity scope's real categories (fact, relationship).
    // The compiler never emits a literal "identity" category — querying it returned nothing.
    let identity_cats: Vec<&str> = scope_to_categories("identity");
    let identity_nodes = graph::query_nodes(
        db, user_addr, None, None, Some(&identity_cats), "current", 0.0, 20,
    ).await.unwrap_or_default();
```

(`graph::query_nodes`'s `category_filter` parameter is `Option<&[&str]>`, so the `Vec<&str>` borrows directly — no conversion.)

- [ ] **Step 2: Verify it builds**

Run: `cargo build -p ainm-demo`
Expected: builds with no errors.

> Note: the behavioral proof (peer card populated from `fact`) is the migrated test
> `briefing_is_query_conditioned_and_has_peer_card` in Task 5. This task is the minimal code fix;
> committing it before the seed migration keeps the change reviewable on its own.

- [ ] **Step 3: Commit**

```bash
git add demo/src/briefing.rs
git commit -m "fix(memory): peer card queries identity-scope categories, not bogus 'identity'"
```

---

### Task 4: Close the extraction category set

**Files:**
- Modify: `demo/src/llm.rs:209` (turn the prompt const into a template + builder fn)
- Modify: `demo/src/llm.rs:135` (call the builder)
- Modify: `demo/src/llm.rs` (add a `#[cfg(test)] mod tests` at end of file)

- [ ] **Step 1: Convert the prompt const into a token template**

In `demo/src/llm.rs`, rename the existing `const EXTRACTION_SYSTEM_PROMPT` (line 209) to
`EXTRACTION_SYSTEM_PROMPT_TEMPLATE` and replace the three category lines and the schema `category`
line with substitution tokens. The full replacement const:

```rust
const EXTRACTION_SYSTEM_PROMPT_TEMPLATE: &str = r#"You are a knowledge extraction specialist. Extract structured entities and relationships from conversations.

## Rules
- Extract SPECIFIC entities only: named people, technologies, tools, projects, preferences, decisions, events, patterns
- Do NOT extract abstract concepts, pronouns, generic nouns, or vague terms
- Prefer specific labels: "TypeScript" not "a programming language", "VS Code" not "code editor"
- Relationships must reference extracted entity labels exactly
- Assign confidence: 0.9-1.0 for explicitly stated facts, 0.7-0.89 for strongly implied, 0.5-0.69 for inferred

## Entity Types
- semantic: stable facts (categories: <<SEMANTIC>>)
- episodic: time-bound events (categories: <<EPISODIC>>)
- procedural: learned patterns (categories: <<PROCEDURAL>>)

## Output Schema (JSON)
{
  "entities": [
    {
      "type": "semantic|episodic|procedural",
      "category": "<<ALL>>",
      "label": "human-readable description",
      "confidence": 0.0-1.0,
      "properties": {},
      "tags": []
    }
  ],
  "relationships": [
    {
      "source": "entity label (must match an entity above)",
      "target": "entity label (must match an entity above)",
      "type": "uses|prefers_over|works_at|decided_on|led_to|requires|part_of|learned_from",
      "confidence": 0.0-1.0
    }
  ]
}

The "category" field MUST be exactly one of: <<ALL>>. Do not invent new categories.
Only output valid JSON. No markdown, no explanation."#;

/// Build the extraction system prompt with the canonical, closed category set.
fn extraction_system_prompt() -> String {
    EXTRACTION_SYSTEM_PROMPT_TEMPLATE
        .replace("<<SEMANTIC>>", &crate::categories::SEMANTIC.join(", "))
        .replace("<<EPISODIC>>", &crate::categories::EPISODIC.join(", "))
        .replace("<<PROCEDURAL>>", &crate::categories::PROCEDURAL.join(", "))
        .replace("<<ALL>>", &crate::categories::all().join("|"))
}
```

(Using `.replace()` on sentinel tokens avoids escaping the JSON braces that `format!` would require.)

- [ ] **Step 2: Call the builder at the extraction site**

In `demo/src/llm.rs`, change line 135 from:

```rust
        let system = EXTRACTION_SYSTEM_PROMPT;
```

to:

```rust
        let system = extraction_system_prompt();
```

The call on line 141 is `self.chat_completion(system, &user_prompt)`. Change it to borrow:

```rust
        let response_text = self.chat_completion(&system, &user_prompt).await?;
```

(`chat_completion` takes `system: &str`; `system` is now an owned `String`.)

- [ ] **Step 3: Add the prompt-closure test**

At the end of `demo/src/llm.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extraction_prompt_is_closed_over_canonical_categories() {
        let p = extraction_system_prompt();
        for c in crate::categories::all() {
            assert!(p.contains(c), "prompt must mention canonical category `{c}`");
        }
        assert!(
            !p.contains("<<"),
            "all template tokens must be substituted (found a leftover `<<`)"
        );
        assert!(
            !p.contains("|...") && !p.contains("...\""),
            "category set must be closed — no open-ended `...`"
        );
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p ainm-demo --lib llm::tests::extraction_prompt_is_closed_over_canonical_categories`
Expected: `test result: ok. 1 passed`

- [ ] **Step 5: Commit**

```bash
git add demo/src/llm.rs
git commit -m "feat(memory): build extraction prompt from canonical categories; close the set"
```

---

### Task 5: Migrate test seeds to canonical vocabulary

**Files:**
- Modify: `demo/tests/integration.rs` (lines 514-518, 546-547, 576-595, 650-659)

This task converts every seed that uses the legacy display vocabulary to canonical categories, using
the migration map at the top of this plan. After it, the peer card (now querying `fact`/`relationship`)
is populated by the same seeds that assert on it.

- [ ] **Step 1: Migrate `briefing_is_query_conditioned_and_has_peer_card`**

In `demo/tests/integration.rs`, lines 514-519 currently seed legacy categories:

```rust
    ainm_demo::graph::insert_node(&pool, &user, "semantic", "preferences",
        "Prefers Rust for backend services", &serde_json::json!({}), 0.9, &prov, &tags).await.unwrap();
    ainm_demo::graph::insert_node(&pool, &user, "semantic", "identity",
        "Name is Santiago", &serde_json::json!({}), 0.95, &prov, &tags).await.unwrap();
    ainm_demo::graph::insert_node(&pool, &user, "semantic", "preferences",
        "Enjoys hiking on weekends", &serde_json::json!({}), 0.9, &prov, &tags).await.unwrap();
```

Replace the category strings (`"preferences"→"preference"`, `"identity"→"fact"`):

```rust
    ainm_demo::graph::insert_node(&pool, &user, "semantic", "preference",
        "Prefers Rust for backend services", &serde_json::json!({}), 0.9, &prov, &tags).await.unwrap();
    ainm_demo::graph::insert_node(&pool, &user, "semantic", "fact",
        "Name is Santiago", &serde_json::json!({}), 0.95, &prov, &tags).await.unwrap();
    ainm_demo::graph::insert_node(&pool, &user, "semantic", "preference",
        "Enjoys hiking on weekends", &serde_json::json!({}), 0.9, &prov, &tags).await.unwrap();
```

The assertions (peer_card contains "Santiago"; blob contains "Prefers Rust") are unchanged — they now
pass because the peer card queries `fact` (where "Name is Santiago" now lives).

- [ ] **Step 2: Migrate `briefing_includes_salient_unmatched_facts`**

Lines 546-547 currently:

```rust
    ainm_demo::graph::insert_node(&pool,&user,"semantic","projects",
        "Next milestone is connector auth hardening Phase 16",&serde_json::json!({}),0.9,&prov,&tags).await.unwrap();
```

Change `"projects"→"fact"` (per the migration map):

```rust
    ainm_demo::graph::insert_node(&pool,&user,"semantic","fact",
        "Next milestone is connector auth hardening Phase 16",&serde_json::json!({}),0.9,&prov,&tags).await.unwrap();
```

Assertion (blob contains "Phase 16") unchanged.

- [ ] **Step 3: Migrate the `dogfood_emit_briefings` seed table**

Lines 576-595 are a `(category, label, confidence)` table using legacy names. Apply the migration map
to the category column only (labels and confidences unchanged):

```rust
        ("fact", "Name is Santiago Gericke", 0.98),
        ("fact", "Founder and engineer at TechTorch", 0.95),
        ("fact", "Works primarily in Rust, Swift, and TypeScript", 0.90),
        ("preference", "Prefers Rust for backend services", 0.92),
        ("preference", "Prefers native SwiftUI over React Native for iOS", 0.90),
        ("preference", "Values production-grade implementations over demos", 0.93),
        ("preference", "Prefers terse, direct, critical feedback", 0.88),
        ("preference", "Defaults to test-driven development", 0.85),
        ("fact", "Building AINM, an AI-native messaging app for chatting with many agents", 0.95),
        ("fact", "Current milestone is v3.3 Universal Agent Client", 0.90),
        ("fact", "Building a compound-memory capture layer that compiles conversations into a user profile", 0.92),
        ("fact", "Shipped the v3.2 iOS reference client", 0.90),
        ("decision", "Chose a server-first hybrid architecture for memory compute", 0.85),
        ("decision", "Retired the GSD workflow in favor of superpowers skills", 0.85),
        ("decision", "Treats portable, user-owned memory as the core moat", 0.90),
        ("decision", "Decided hosted runtime is the consent-enforcement boundary", 0.82),
        ("pattern", "Works in focused sprints and pivots based on research", 0.80),
        ("pattern", "Often acts as a critic to pressure-test scope before building", 0.82),
        ("fact", "Wants to validate whether compound memory makes new agents better before investing further", 0.88),
        ("fact", "Next up: connector auth hardening (Phase 16)", 0.80),
```

- [ ] **Step 4: Migrate the `dogfood_scale_test` important-facts seed**

Lines 650-659 (the `[IMP*]` facts). Apply the migration map to the category column:

```rust
        ("fact", "[IMP1] Current milestone is v3.3 Universal Agent Client", 0.95),
        ("fact", "[IMP2] Next up: connector auth hardening Phase 16", 0.9),
        ("fact", "[IMP3] Building the compound-memory capture layer this sprint", 0.95),
        ("fact", "[IMP4] Wants to validate compound memory before investing further", 0.9),
        ("decision", "[IMP5] Decided to run a scale test of memory briefing", 0.88),
        ("preference", "[IMP6] Prefers to plan the week around highest-leverage AINM work", 0.9),
        ("fact", "[IMP7] iOS reference client v3.2 shipped, v3.3 in progress", 0.9),
        ("pattern", "[IMP8] Works in focused sprints", 0.85),
        ("fact", "[IMP9] Needs to decide what to build after the memory layer", 0.9),
        ("fact", "[IMP10] Name is Santiago Gericke, founder at TechTorch", 0.98),
```

(If the noise-fact generation loop lower in `dogfood_scale_test` uses a legacy category string literal,
change it to a canonical one such as `"fact"`. Read the loop before editing; only the category literal
may need changing.)

- [ ] **Step 5: Confirm no legacy category literals remain in the test file**

Run: `rg -n '"(identity|preferences|projects|decisions|patterns|relationships)"' demo/tests/integration.rs`
Expected: matches ONLY on `scope` strings (e.g. inside `grant_scopes`/`resolve_scopes` tests around
lines 267-310 and 280/300 where `"identity"`, `"history"`, `"patterns"` are **scope** names passed to
`grant_scopes` — those are correct and must NOT change). There must be **no** remaining matches used as
the 4th argument (category) to `insert_node`. If any category-position match remains, migrate it.

- [ ] **Step 6: Run the full test suite**

Run: `cargo test -p ainm-demo` (and the ignored dogfood tests explicitly if a DB is up:
`cargo test -p ainm-demo -- --ignored dogfood_emit_briefings dogfood_scale_test`)
Expected: all pass EXCEPT the two known pre-existing failures (`test_graph_traversal`,
`test_compilation_dedup_updates_confidence`). If Postgres is not running, DB-backed tests `return`
early (pass trivially) — note that in the run output.

- [ ] **Step 7: Commit**

```bash
git add demo/tests/integration.rs
git commit -m "test(memory): migrate seeds to canonical category vocabulary"
```

---

## Final verification

- [ ] Run the full library test suite: `cargo test -p ainm-demo --lib`
  Expected: `categories::tests`, `briefing::tests` (incl. completeness + updated scope map), and
  `llm::tests` all green.
- [ ] Confirm completeness guard works: temporarily delete `"relationship"` from
  `categories::SEMANTIC`, run `cargo test -p ainm-demo --lib briefing::tests::scope_map_exactly_covers_the_canonical_set` → it should FAIL (proving the guard bites), then restore it.
- [ ] `cargo build -p ainm-demo` clean.
- [ ] Hand off to `superpowers:finishing-a-development-branch`.
