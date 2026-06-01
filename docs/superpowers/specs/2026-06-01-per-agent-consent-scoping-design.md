# Per-Agent Consent Scoping — Design

**Date:** 2026-06-01
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Related:** `2026-06-01-category-vocabulary-reconciliation-design.md` (the prerequisite that made the
scope→category map provably complete), `2026-05-30-compound-memory-capture-layer-design.md` (which
deferred per-agent consent scoping to "the next layer").

## 1. Goal

Turn on consent scoping in `generate_briefing`: an agent joining a thread sees **only** the memory
categories its resolved consent scopes permit. Today the scopes are resolved but ignored — the
candidate query passes `None` for categories, so every agent sees everything. This makes the hosted
runtime an actual enforcement boundary (the consent moat), now safe to do because the
scope→category map was made provably complete in the vocabulary-reconciliation work.

## 2. What already exists (reuse, don't rebuild)

- **`memory_grants` table** (`migrations/002_compound_memory.sql:81`) — per-(user, agent) `scopes` +
  `consent_level`.
- **Grant/revoke RPC** — `ainm.memory.grant` (`server.rs:858`), `ainm.memory.revoke` (`server.rs:888`).
- **`resolve_scopes`** (`briefing.rs`) — returns `(Vec<String> scopes, ConsentLevel)`: per-agent grant
  if present, else the caller-supplied default consent level's scopes. Already called at the top of
  `generate_briefing`.
- **`scope_to_categories`** (`briefing.rs`) — maps a scope to its canonical categories; union over all
  scopes provably equals `categories::all()` (locked by `scope_map_exactly_covers_the_canonical_set`).
- **`SCOPES` / `ConsentLevel::included_scopes`** — the six scopes and the minimal/standard/full tiers.

**The only gap:** `generate_briefing` does not use the resolved scopes to filter retrieval, and the
peer card is not consent-gated.

## 3. Decisions locked (brainstorming)

- **Peer card is gated by the identity scope** — no `identity` in the resolved scopes → empty peer
  card. Full enforcement, no exceptions (matches "hosting = enforcement boundary").
- **Default consent stays `Standard`** — an agent with no explicit grant sees identity + preferences +
  expertise + goals, preserving the "fresh agent is instantly useful" value proven in the dogfood.
  `server.rs` continues passing `ConsentLevel::Standard` as the default; this layer does not add
  per-user default-consent storage.
- **Filter at the SQL layer** (Approach A) with an explicit empty-allowed short-circuit. Disallowed
  facts never leave the database.

## 4. Architecture / changes (all in `demo/src/briefing.rs::generate_briefing`)

### 4.1 Resolve scopes → allowed categories

After the existing `resolve_scopes` call:

```rust
let mut allowed: Vec<&str> = scopes
    .iter()
    .flat_map(|s| scope_to_categories(s))
    .collect();
allowed.sort_unstable();
allowed.dedup();
```

`scope_to_categories` returns `Vec<&'static str>`; unknown scope names yield `vec![]`, so a malformed
or hostile grant scope contributes no categories.

### 4.2 Empty-allowed short-circuit (CRITICAL safety property)

`graph::query_nodes` treats `Some(&[])` as "no category filter" (its `if !cats.is_empty()` guard skips
the clause), which would return **all** categories. Therefore: if `allowed.is_empty()`, return a
well-formed empty briefing — empty `sections`, `peer_card: ""`, `graph_snapshot: null`,
`char_budget_used: 0`, `node_count: 0` — and still write `briefing_log`. Never pass an empty slice to
`query_nodes`.

### 4.3 Scope the retrieval queries

The broad candidate query (currently `None`) and the query-matched boost query both pass
`Some(allowed.as_slice())`. Ranking (`score_node`), packing (`pack_to_budget`), and dynamic section
bucketing are unchanged — they now operate over a consent-scoped candidate set automatically.

```rust
let candidates = graph::query_nodes(
    db, user_addr, None, None, Some(allowed.as_slice()), "current", 0.0, 200,
).await.map_err(|e| AppError::Storage(e))?;
// ... boost query likewise passes Some(allowed.as_slice())
```

### 4.4 Gate the peer card by identity scope

```rust
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

(The peer card's own query is naturally a subset of `allowed` when `identity` is granted, so it needs
no further intersection.)

## 5. Safety properties

- **No empty-filter leak** — the §4.2 short-circuit guarantees "granted nothing → see nothing."
- **No SQL injection from grants** — `query_nodes` interpolates category strings, but the only values
  reaching it come from `scope_to_categories`, which returns hardcoded `&'static str` literals; user-
  controlled grant scope names that aren't recognized map to `vec![]` and never reach the SQL.
- **Enforcement at the host** — filtering happens server-side before any data is serialized to the
  agent; the agent never receives out-of-scope facts to ignore.

## 6. What does NOT change

- Default consent level (`Standard`) and where it's set (`server.rs`).
- `memory_grants`, grant/revoke RPC, `resolve_scopes`, `scope_to_categories` — untouched.
- Briefing JSON shape — `sections` simply contains fewer categories under tighter consent; no field
  added or removed, so **no iOS/connector client changes**.
- The hybrid retrieval algorithm (boost-not-filter on the query, recency×confidence ranking, char
  budget, min-score floor) — unchanged; it now ranks within the scoped candidate set.

## 7. Consent → category exposure (resulting behavior)

| Consent (no grant → Standard) | Scopes | Categories visible |
|---|---|---|
| Minimal | identity | `fact, relationship` |
| Standard | identity, preferences, expertise, goals | `fact, relationship, preference, tool, skill, event, decision` |
| Full | all six | all 13 |

A per-agent grant overrides the default; e.g. a grant of only `patterns` → `pattern, workflow,
strategy, communication` and an **empty peer card** (no identity scope).

## 8. Testing (integration, DB-backed; `demo/tests/integration.rs`)

Seed a user with facts spanning several categories (e.g. `fact`, `preference`, `skill`, `pattern`),
then assert the briefing's surfaced categories per consent:

1. **Minimal grant** → briefing JSON contains only `fact`/`relationship` facts; a seeded `pattern`/
   `skill` fact is absent.
2. **Standard (default, no grant)** → includes `preference`/`skill`/`event`/`decision`; excludes
   `outcome`/`interaction` (history-only) and `pattern`/`workflow`/`strategy`/`communication`
   (patterns-only).
3. **Full grant** → all seeded categories present.
4. **Per-agent grant override** → grant only `patterns`: briefing contains pattern-family facts only,
   and `peer_card` is empty (`""`).
5. **Empty/zero-category grant** → grant an empty scopes array at Minimal... (note: empty scopes in a
   grant falls back to the level's `included_scopes`, so to exercise the true empty path, grant a
   single unrecognized scope like `"bogus"`) → briefing has empty `sections`, empty `peer_card`,
   `node_count` 0, and does not error.
6. **Negative leak check** → a fact whose category is outside the granted scopes is provably absent
   from the serialized briefing blob.

Existing tests `test_briefing_minimal_consent` and `test_generate_briefing` must be reviewed: they may
now assert different (correctly scoped) section contents — update expectations to match enforced
scoping rather than the old all-categories dump.

## 9. Risks

- **Behavior change:** "broad recall" becomes "broad recall within granted scopes." This is the point
  of consent, but it is a deliberate change from today's all-categories briefing — anything relying on
  the old behavior (e.g. the dogfood emit at Standard) should be re-checked; Standard still surfaces
  the high-value identity/preference/project(`fact`)/decision facts, so the dogfood verdict stands.
- **`test_briefing_minimal_consent` already asserts `!sections.contains_key("expertise")`** — under
  enforcement, Minimal won't include skill facts at all, so this assertion still holds (and is
  strengthened). Verify during implementation.

## 10. Definition of done

1. `generate_briefing` filters the candidate (and boost) queries by the categories of the resolved
   scopes; ranking/sections operate on the scoped set.
2. Empty allowed-categories → well-formed empty briefing, never an unfiltered query (the leak is
   closed, with a regression test).
3. Peer card is empty unless the `identity` scope is granted.
4. Integration tests cover minimal/standard/full, per-agent override, the empty-scope short-circuit,
   and a negative leak check; full suite green except the two known pre-existing failures
   (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`).
5. Default consent unchanged; no client changes.
