# Memory Briefing Dogfood — Verdict (2026-05-31)

**Branch:** `feat/memory-briefing-retrieval`
**What was tested:** the in-app compound-memory capture layer MVP — specifically the *briefing* upgrade (query-conditioned + ranked + budgeted + peer-card injection) vs. the prior unconditioned category dump.
**Task used:** *"Help me plan what to work on this week."* (a deliberately broad task where the right answer depends on the user's projects/priorities).
**Mode:** lean seeded harness — a ~20-fact synthesized profile (founder building AINM) seeded into Postgres; three briefings emitted via the real `generate_briefing` code path. (`demo/tests/integration.rs::dogfood_emit_briefings`, output at `experiments/compound-memory-spike/out/dogfood_briefings.json`.)

## Arms
- **A — none:** empty context.
- **B — old dump:** `query_nodes` unconditioned, all categories, no ranking, no peer card.
- **C — new briefing:** the implemented query-conditioned + ranked + peer-card path.

## Round 1 finding: C REGRESSED vs B (caught a real bug)
For the broad task, the first implementation used the FTS query as a **filter** (`query_nodes(Some(query), ...)`). Result: only **2 of 20 facts** matched (lexical overlap on "works"); the high-signal project/goal/decision facts ("current milestone v3.3", "Phase 16", "compound-memory capture layer") were **dropped**. `sections` was also empty (section-bucketing recognized only specific hardcoded category names that didn't match stored categories). **B (the dump) was clearly more useful** — it retained all the facts an agent needs to plan a week.

**Root cause (confirmed, matches the spec's flagged risk):** lexical FTS relevance ≠ usefulness for broad tasks. "plan / work / week" shares no keywords with "current milestone v3.3" or "Phase 16", so query-*filtering* starves broad tasks.

## Fix: query as a BOOST, not a filter (commit `e190982`)
- Candidate set = **all** current facts for the user (broad recall; per-agent consent scoping remains deferred for the MVP).
- The FTS query now only **boosts** matched facts (3×) in the recency×confidence ranking — it never excludes.
- `pack_to_budget` keeps the top-ranked facts under a 4000-char budget (everything fits for small profiles → full recall; at scale, ranking decides what's cut).
- Sections are now built **dynamically** from each fact's actual category → never empty.
- Locked by a new test `briefing_includes_salient_unmatched_facts` (a high-salience fact with zero keyword overlap to the query is still injected).

## Round 2 result: C ≥ B
Arm C now returns the same fact set as the dump **plus** structured sections (identity/projects/preferences/decisions/patterns), a peer card ("Name is Santiago Gericke / Founder at TechTorch / Rust, Swift, TypeScript"), and salience+query ranking. `char_budget_used: 1024`. Full suite: 19 passed, only the 2 pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`, which fail on `main` too).

## Round 3: scale test (300 facts) + relevance floor → C is FOCUSED > B
Seeded 10 recent high-confidence important facts + 290 stale noise facts (400–700 days old, `demo/tests/integration.rs::dogfood_scale_test`).

- **Round 3a (commit before floor):** C surfaced all 10 important facts and dropped the *oldest* noise (recency ranking works), but **backfilled the char budget with 52 stale-but-not-oldest noise facts** → 62 facts total. Finding: the packer was budget-*bounded* but not relevance-*floored* — greedy fill backfills leftover budget with marginal items.
- **Round 3b (commit `8cc160e`): added a minimum-score floor to `pack_to_budget`** (drop facts below an absolute relevance threshold regardless of remaining budget; `BRIEFING_MIN_SCORE = 0.05`, ~negligible recency×confidence). Result: **arm C = exactly the 10 important facts, 550 chars, 0 noise leaked**; arm B = 200-fact 13,174-char firehose. Locked by `pack_to_budget_drops_below_floor_even_with_budget_room` (unit) + the scale test asserting `arm_c_fact_count < 25` + all 10 `[IMP]` present + noise excluded.

## Verdict: CONFIRMED at the briefing level
- ✅ Regression fixed (Round 2) and **C demonstrably beats B at scale** (Round 3): under a real budget over 300 facts, C surfaces 100% of the relevant recent facts and 0 noise; B is an unranked 200-fact firehose. C is curated by relevance, not capped by size.
- ✅ The dogfood loop did its job three times: caught the broad-task regression, then the bounded-but-not-focused gap, each fixed cheaply before merge.
- ⚠️ **Downstream answer-quality A/B not run** (no LLM key). The briefing-content evidence is strong, but the final "agent answers better" measurement is still open — run it when a key is available, ideally on the large profile.

## Recommended follow-ups (not done here)
1. **Scale test:** seed 200–500 facts and re-run the dogfood so the budget forces selection — this is the test that actually proves C > B.
2. **Relevance under budget:** when the budget bites at scale, lexical FTS boosting may still mis-rank; that's the trigger for the spec's deferred **pgvector/semantic** escalation.
3. **Category vocabulary:** reconcile the consent `scope_to_categories` names with the categories the compiler actually emits (the empty-sections bug stemmed from this mismatch); needed before per-agent consent scoping is un-deferred.
4. **LLM answer-quality A/B** once a key is available, ideally on the large profile.
