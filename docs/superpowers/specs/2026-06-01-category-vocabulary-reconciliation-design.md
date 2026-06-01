# Category Vocabulary Reconciliation — Design

**Date:** 2026-06-01
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Related:** `docs/superpowers/notes/2026-05-31-memory-briefing-verdict.md` (follow-up #3),
`2026-05-30-compound-memory-capture-layer-design.md` (the briefing/consent layer this corrects)

## 1. Goal

Make the compound-memory **category vocabulary** internally consistent so the consent
`scope → category` mapping is provably complete, and fix two latent bugs that bite the moment real
compiler-produced data (rather than the dogfood seed) flows through the briefing. This is the
prerequisite the verdict note named "before per-agent consent scoping is un-deferred."

**Scope (locked during brainstorming):** reconcile the vocabulary, fix the two latent bugs, and lock
the result with tests. The consent **category filter stays OFF** in `generate_briefing` — un-deferring
per-agent scoping remains a separate later task.

## 2. The problem (current state)

Three category vocabularies coexist in the repo:

1. **Compiler emits** (`demo/src/llm.rs` extraction prompt): `fact, preference, skill, relationship,
   tool` (semantic) · `event, decision, outcome, interaction` (episodic) · `pattern, workflow,
   strategy, communication` (procedural) — 13 categories, and the output-schema line ends with
   `...` (open-ended, so the LLM may invent categories).
2. **`scope_to_categories` maps to** (`demo/src/briefing.rs`): the same set **minus `relationship`**.
3. **Seed / spike / iOS-display vocabulary**: `identity, preferences, projects, decisions, patterns,
   relationships` — used by the dogfood seed and, critically, the **peer-card query**.

Two concrete latent bugs result:

- **Empty peer card on real data.** `briefing.rs` queries `category = "identity"`, but the compiler
  emits `fact` — it never emits `identity`. The peer card only worked in the dogfood because the seed
  used the V3 name `"identity"`.
- **`relationship` orphaned.** It is emitted (semantic) but mapped to no scope, so it would be
  invisible once consent scoping is enabled.

Root cause: there is no single source of truth for the category set, and the open-ended `...` means
even a corrected map could be re-orphaned by a future LLM-invented category.

## 3. Decisions locked (brainstorming)

- **Canonical set is the compiler-emitted vocabulary (V1).** Scopes remain a separate human-facing
  consent layer that maps *onto* categories.
- **Closed category set.** Remove the `...`; pin a fixed list; enforce completeness with a test.
- **`relationship` → `identity` scope** (`identity → [fact, relationship]`): who you're connected to
  is part of who you are, shared at every consent level, and lets the peer card surface a key
  relationship.
- **Single source of truth + completeness test** (Approach A): the no-orphan property is mechanical
  and drift-proof, not a state we hope holds.

## 4. Architecture / components

### 4.1 Canonical vocabulary module — `demo/src/categories.rs` (new)

The three entity-type groups are the source of truth; the full set is derived from them, so they
cannot drift apart.

```rust
//! Canonical compound-memory category vocabulary — single source of truth.
pub const SEMANTIC:   &[&str] = &["fact", "preference", "skill", "relationship", "tool"];
pub const EPISODIC:   &[&str] = &["event", "decision", "outcome", "interaction"];
pub const PROCEDURAL: &[&str] = &["pattern", "workflow", "strategy", "communication"];

/// Every category the compiler may emit (closed set), in stable order.
pub fn all() -> Vec<&'static str> {
    SEMANTIC.iter().chain(EPISODIC).chain(PROCEDURAL).copied().collect()
}
```

Registered as `mod categories;` in **both** `demo/src/lib.rs` and `demo/src/main.rs` (the briefing
path compiles into both the library and the binary — same pattern already used for `retrieval`).

### 4.2 Mapping + peer-card fix — `demo/src/briefing.rs`

`scope_to_categories` adds `relationship` under `identity`; all other arms unchanged:

```rust
fn scope_to_categories(scope: &str) -> Vec<&'static str> {
    match scope {
        "identity"    => vec!["fact", "relationship"], // was vec!["fact"]
        "preferences" => vec!["preference", "tool"],
        "expertise"   => vec!["skill"],
        "goals"       => vec!["event", "decision"],
        "history"     => vec!["event", "decision", "outcome", "interaction"],
        "patterns"    => vec!["pattern", "workflow", "strategy", "communication"],
        _ => vec![],
    }
}
```

Union of all six scopes' categories == `categories::all()` (13), with no extras. (Scopes may overlap —
`goals ⊂ history` — which is fine; completeness only requires the union to equal the canonical set.)

The peer card stops hardcoding the bogus `"identity"` category and queries the identity scope's real
categories:

```rust
let identity_cats: Vec<&str> = scope_to_categories("identity"); // ["fact", "relationship"]
let identity_nodes =
    graph::query_nodes(db, user_addr, None, None, Some(&identity_cats), "current", 0.0, 20)
        .await.unwrap_or_default();
```

(`graph::query_nodes`'s `category_filter` is `Option<&[&str]>`, so the `Vec<&str>` from
`scope_to_categories` passes directly — no conversion. The current bogus call is
`Some(&["identity"])`.)

### 4.3 Close the extraction set — `demo/src/llm.rs`

Build the prompt's three category lists from `categories::{SEMANTIC, EPISODIC, PROCEDURAL}` rather
than a hand-typed prose list; replace the open-ended `"preference|fact|skill|tool|event|decision|
pattern|..."` in the output schema with the exact pipe-joined closed list from `categories::all()`;
and add one instruction line:

> `category` MUST be exactly one of the listed categories. Do not invent new categories.

The `semantic|episodic|procedural` *type* grouping in the prompt is retained for pedagogy; only the
category enumeration becomes generated + closed.

## 5. Testing

- **Completeness (the guarantee):** `union(scope_to_categories(s) for s in SCOPES)` equals
  `categories::all()` exactly — fails on any orphaned category or any mapped-but-non-canonical
  category. Lives in `briefing.rs` tests (has access to `SCOPES` + `scope_to_categories`).
- **Prompt closure:** the constructed extraction prompt contains every value in `categories::all()`
  and the output-schema category line contains no literal `...`.
- **Updated `test_scope_to_categories`:** `identity` now expects `["fact", "relationship"]`; other
  arms unchanged; `"unknown"` still empty.
- **Seed migration:** migrate the integration/dogfood seed from V3 names to canonical:
  `identity→fact`, `preferences→preference`, `projects→fact`, `decisions→decision`,
  `patterns→pattern`, `relationships→relationship`. Update the affected assertions in
  `briefing_is_query_conditioned_and_has_peer_card`, `briefing_includes_salient_unmatched_facts`,
  and the `#[ignore]` dogfood tests so they assert against real vocabulary (peer card populated from
  `fact`/`relationship`; sections keyed by canonical categories).
- **Regression:** full `cargo test` / `cargo nextest` green except the two pre-existing,
  work-independent failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`).

## 6. Files touched

- **Create:** `demo/src/categories.rs`
- **Modify:** `demo/src/lib.rs`, `demo/src/main.rs` (`mod categories;`)
- **Modify:** `demo/src/llm.rs` (generated + closed category enumeration in the extraction prompt)
- **Modify:** `demo/src/briefing.rs` (`scope_to_categories` + peer-card query + completeness/prompt
  tests + updated `test_scope_to_categories`)
- **Modify:** `demo/tests/integration.rs` (seed migration + assertion fixes)

## 7. Out of scope

- **Consent category filter stays OFF** in `generate_briefing` (candidate query keeps passing `None`
  for categories). Un-deferring per-agent scoping is a separate task this work unblocks.
- **Runtime rejection of off-list categories at ingestion** (`compiler.rs`). The prompt instructs the
  closed set; *enforcing* it on the stored node is separate hardening — recorded as a follow-up.
- **Friendly section display labels** (`Identity`/`Projects`/… instead of `fact`/`preference`/…). A
  UI concern, not vocabulary correctness; the iOS client renders sections dynamically so technical
  keys won't break it.
- **The throwaway Python spike's 6-category vocabulary** — not on the product path; left as-is.

## 8. Risks

- **Section keys become technical** for real data (`fact`, `preference`, …). Acceptable: dynamic
  rendering on the client, and friendly labels are an explicit non-goal here.
- **LLM may still emit an off-list category** despite the closed-set instruction. With the filter
  off, such a node simply appears under its own dynamic section — no crash, no data loss. Ingestion
  enforcement is the named follow-up.
- **Seed migration touches several tests at once.** Mitigated by doing the migration as one focused
  task and running the full suite immediately after.

## 9. Definition of done

1. `demo/src/categories.rs` exists as the single source of truth; `mod categories;` in lib + main.
2. `scope_to_categories` maps `identity → [fact, relationship]`; the completeness test passes,
   proving the six scopes' union equals the canonical 13 with no extras.
3. The extraction prompt enumerates the closed set (built from `categories`) with no `...`, locked by
   the prompt test.
4. The peer card is populated from `fact`/`relationship` (verified by a migrated-seed integration
   test), fixing the empty-peer-card bug on real-vocabulary data.
5. Full test suite green except the two pre-existing unrelated failures.
