# Ingestion-Side Category Enforcement — Design

**Date:** 2026-06-01
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Related:** `2026-06-01-category-vocabulary-reconciliation-design.md` (defined the closed canonical set
and instructed the extraction prompt to use it — but did not *enforce* it at ingestion),
`2026-06-01-per-agent-consent-scoping-design.md` (consent depends on categories being canonical).

## 1. Goal

Make the compiler **reject off-list categories at ingestion**: a candidate node whose LLM-supplied
category is not in the canonical closed set never reaches storage. The extraction prompt already
instructs the model to use the closed set, but nothing enforces it — an off-list category would be
stored and then be orphaned from every consent scope (invisible, un-mappable). This closes the gap so
`memory_nodes` only ever holds canonical, consent-mappable categories.

## 2. What exists / the gap

- `categories.rs` — canonical closed set: `SEMANTIC`/`EPISODIC`/`PROCEDURAL` + `all()` (13 categories).
- `compiler.rs::run_compilation` — `llm.extract` → `extraction.nodes: Vec<CandidateNode>` →
  `dedup_and_integrate_nodes` → `graph::insert_node`. Each `CandidateNode.category` is a `String`
  taken verbatim from the LLM.
- The extraction prompt (`llm.rs`) instructs the model to emit only canonical categories, but
  `run_compilation` inserts whatever the model returned with **no validation**.

**Gap:** no ingestion-time validation of `CandidateNode.category` against the canonical set.

## 3. Decisions locked (brainstorming)

- **Off-list → drop + log.** Skip the node entirely; emit a `tracing::warn!` with the offending
  category and label (so prompt drift is observable). Cleanest consent posture — storage only holds
  canonical categories; the cost is losing a fact whose category we cannot trust.
- **Normalize then check.** Trim + ASCII-lowercase the category before membership testing, and store
  the canonical form. Recovers trivially-deviating-but-valid categories (`"Fact"` → `"fact"`); only
  genuinely off-list categories (`"project"`, `"misc"`) are dropped.
- **Enforce in `compiler.rs::run_compilation`** (Approach A) via a pure `categories::normalize` helper
  — single choke point, `llm.rs` stays a plain parser.

## 4. Architecture / changes

### 4.1 `categories::normalize` — `demo/src/categories.rs` (new fn)

```rust
/// Normalize a raw category to its canonical form, or None if off-list.
/// Trims + ASCII-lowercases, then membership-tests against the closed set.
pub fn normalize(raw: &str) -> Option<&'static str> {
    let key = raw.trim().to_ascii_lowercase();
    all().into_iter().find(|&c| c == key.as_str())
}
```

Returns the canonical `&'static str` so callers store the exact literal, never the model's casing.
Lives beside `all()` in the vocabulary's own module; pure and unit-testable.

### 4.2 Enforce in `run_compilation` — `demo/src/compiler.rs`

Immediately after extraction, normalize-and-filter candidate nodes before dedup:

```rust
let extraction = llm.extract(&req.messages).await?;

// Ingestion-side enforcement: drop nodes whose category is not in the canonical
// closed set (the prompt instructs it, but the model may still deviate). Normalize
// trivial casing/whitespace variance to the canonical form rather than dropping it.
let valid_nodes: Vec<CandidateNode> = extraction.nodes.into_iter().filter_map(|mut c| {
    match crate::categories::normalize(&c.category) {
        Some(canon) => {
            c.category = canon.to_string();
            Some(c)
        }
        None => {
            tracing::warn!(
                "[compiler] dropping node with off-list category {:?} (label {:?})",
                c.category, c.label
            );
            None
        }
    }
}).collect();

let (new_nodes, updated_count, superseded_count) =
    dedup_and_integrate_nodes(&db, &req.user_addr, &req.thread_id, &valid_nodes).await?;
```

`extraction.edges` is used unchanged in the subsequent `integrate_edges` call (partial move of
`extraction.nodes` is fine; `ExtractionResult` has no `Drop`). If `CandidateNode` is not already
field-mutable via an owned binding, the `into_iter()` owned `c` makes `c.category = …` valid.

### 4.3 Edges to dropped nodes

`integrate_edges` resolves edge endpoints by label through the `new_nodes` (label→id) map; a dropped
node's label is absent, so any edge referencing it is skipped. This is correct — we do not link to a
node we refused to store. No additional code.

## 5. Testing

- **Unit (`categories::normalize`):**
  `"Fact" → Some("fact")`, `" preference " → Some("preference")`, `"DECISION" → Some("decision")`,
  `"fact" → Some("fact")`, `"project" → None`, `"misc" → None`, `"" → None`.
- **Compiler integration (mock LLM):** craft an extraction with three nodes — a valid `"fact"`, a
  casing-variant `"Skill"`, and an off-list `"project"`. After `run_compilation`:
  - the `fact` node is stored;
  - the `"Skill"` node is stored with category exactly `"skill"` (normalized);
  - the `"project"` node is absent from `memory_nodes`.
  Model this on the existing `test_compilation_with_mock_llm` / `MockLlmClient` pattern in
  `demo/tests/integration.rs`.
- **Regression:** full suite green except the two known pre-existing failures (`test_graph_traversal`,
  `test_compilation_dedup_updates_confidence`).

## 6. Out of scope

- **Retroactive cleanup of existing off-list nodes** — this is forward-only ingestion enforcement.
  Real data is fresh and the reconciliation already migrated seeds; a backfill/migration could be a
  later follow-up.
- **`node_type` validation** (`semantic`/`episodic`/`procedural`) — a separate small enum, not the
  closed *category* set; not in this scope.
- **Surfacing drops to the user** — drops are logged only (see Risk).

## 7. Risk

Dropping is silent to the user (logged, not surfaced). If prompt drift caused frequent off-list
categories, facts would quietly vanish — but the `warn!` log makes it detectable, and the shipped
closed-set prompt instruction makes frequent drift unlikely. Normalization further reduces drops to
genuinely-wrong categories. Acceptable for an LLM-error guard.

## 8. Definition of done

1. `categories::normalize` exists, returns the canonical `&'static str` for in-set (incl. casing/
   whitespace variants) and `None` for off-list, with unit tests.
2. `run_compilation` normalizes-and-filters candidate nodes after extraction; off-list nodes are
   dropped with a `warn!`, in-set nodes are stored with their canonical category.
3. A compiler integration test proves an off-list category node is not persisted while a valid node
   and a casing-variant node are (the latter with its normalized category).
4. Full suite green except the two known pre-existing failures.
