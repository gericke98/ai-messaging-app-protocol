# In-App Compound-Memory Capture/Compilation Layer (MVP) — Design

**Date:** 2026-05-30
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Related:** `2026-05-30-compound-memory-value-spike-design.md` (the throwaway value spike whose `distill`/`reconcile`/`inject` logic this productionizes)

## 1. Goal

Passively turn **in-app conversations** — with any agent (Claude/GPT/Gemini/local/MCP), all routed
through the AINM server — into a **cross-agent, user-owned compound profile**, and inject the
relevant slice into agents so a fresh agent is *visibly* more useful from message one. This is a
**thin, server-first MVP** that doubles as the value test: dogfooding it answers "does compound
cross-agent memory actually help?" on automatic, real in-app data (no manual export).

Core product premise (user-stated): **capturing your data to understand your profile is a first-class
value of the app**, not a deferred detail. Because every agent is used *through* AINM, capturing them
uniformly is a side effect of normal use.

## 2. What already exists (build on, don't rebuild)

From a codebase audit:
- **Tier 0 raw capture already exists.** Every human+agent message flows through the single choke
  point `ainm.message.send` in `demo/src/server.rs` and is persisted to a `messages` table.
- **Schema scaffolding exists:** `messages`, `archival_facts`, `memory_nodes`, `memory_edges`,
  `compilation_passes`, `briefing_log` (migrations `001_base_tables.sql`, `002_compound_memory.sql`).
- **Briefing + consent scaffolding exists:** `demo/src/briefing.rs` (consent levels, scope
  resolution), `demo/src/graph.rs` (node/edge CRUD), and the connector-facing `ainm.memory.briefing`
  API the SDK already calls.

> **Implementation-reality correction (deep grounding, 2026-05-30).** A deeper code audit found that
> far more already exists than first assessed — this re-scopes the MVP and obsoletes two earlier
> decisions:
> - **Capture hook exists:** `store_and_maybe_compile` (`server.rs:530`) — both `ainm.message.send`
>   arms funnel through it and it **already enqueues a compile job** (`compile_tx`) on agent-response
>   turns.
> - **Compile worker exists:** `compiler.rs` (`run_compilation`, `spawn_compilation_worker`) already
>   does LLM extraction → `graph::insert_node`/`supersede_node` via the existing `LlmClient` /
>   `OpenAiClient` (`llm.rs`), on the existing mpsc-worker pattern. Reconciliation exists (heuristic
>   dedup; embeddings explicitly marked "Future"). **An LLM client and HTTP client (`reqwest`)
>   already exist — do NOT add a Python sidecar.**
> - **Storage exists:** `memory_nodes` already has provenance, `valid_from/until`, `superseded_by`.
> - **The actual gap is INJECTION QUALITY.** `generate_briefing` (`briefing.rs`) currently calls
>   `graph::query_nodes(..., text_query=None, ..., limit=200)` — it returns up to 200 category-filtered
>   nodes with **no query-conditioning, no relevance ranking, no peer card, no token budget.** It
>   hands the agent an undifferentiated dump, not the right slice for the current conversation.
>
> **Re-scoped MVP = improve `briefing.rs` retrieval/injection**, reusing capture/compile/storage as-is.

## 3. Architecture — compile at the existing choke point (3-tier, server-first)

```
ainm.message.send ──persist (exists)──> bump per-user turn counter
                                            │ every ~N turns (default 10), debounced
                                            ▼
                                [async compile worker]  (off the response path)
                                read recent window → frontier LLM extract facts
                                → reconcile vs existing (ADD/UPDATE/supersede, recency)
                                → embed (pgvector) → upsert memory_nodes (+ provenance)
                                → refresh tiny "peer card" anchor
                                            │
new agent turn / briefing request ─────────┘
   retrieve: semantic × recency × salience, greedy-fill to token budget
   + always-on peer card  →  inject via existing ainm.memory.briefing API
```

The three tiers (the key inversion vs Hermes' capped files: **eviction lives only at the per-turn
prompt boundary as non-destructive ranking, never at storage**):

- **Tier 0 — Raw corpus** (exists): every message, immutable, never evicted. The asset Hermes lacks.
- **Tier 1 — Distilled facts** (this MVP): unbounded structured facts with provenance + embeddings;
  reconciled by recency with **supersede-not-delete** (keep history, mark `valid_to`).
- **Tier 2 — Working profile** (this MVP): the small, query-conditioned slice injected per agent
  turn, capped by token budget via retrieval + ranking, prepended with a tiny always-on peer card.

## 4. Components (each small, independently testable)

1. **Capture hook** (`server.rs`) — after a message is persisted, increment a per-user turn counter;
   when `counter % N == 0`, enqueue a compile trigger for that user. No new transport; one hook in
   the existing handler. Compilation never blocks message delivery.
2. **Compile worker — REUSE AS-IS** (`compiler.rs`). It already extracts candidate facts via the
   existing `LlmClient`/`OpenAiClient`, reconciles (heuristic dedup, ADD/supersede), and upserts into
   `memory_nodes` with provenance, on the existing async mpsc-worker triggered by
   `store_and_maybe_compile`. **No Python sidecar, no rebuild.** Touch it only if it doesn't already
   populate the categories/confidence the new retrieval needs — verify first, extend minimally if so.
3. **Query-conditioned retrieval — the core new work** (`briefing.rs`). Replace the `text_query=None`,
   flat-200-node call to `graph::query_nodes` with: (a) derive a query string from the thread's recent
   message(s)/topic and pass it as `text_query`, so the **existing Postgres FTS** (`search_vector` /
   `to_tsquery`, already built + GIN-indexed) returns conversation-relevant facts; (b) rank returned
   nodes by relevance × recency (`valid_from`) × confidence; (c) greedy-pack under a token budget.
   **pgvector/embeddings are deferred** — add only if FTS relevance proves too weak.
4. **Peer card** — a tiny, capped identity summary (top identity/preference facts), refreshed on each
   compile, always injected as a continuity anchor. The one Hermes idea retained.
5. **Briefing integration** — assemble `peer card + ranked facts under budget` and return it through
   the existing `ainm.memory.briefing` API, so connectors inject it with no client changes.

## 5. Data model (Tier 1 fact)

Stored in the existing `memory_nodes` table (semantic nodes; no edges in MVP). Each fact:
`{ id, category (identity|preferences|projects|decisions|patterns|relationships), statement,
salience, status (active|superseded), superseded_by?, valid_from, valid_to?, provenance:
[{source_message_id, thread_id, source_agent, ts}], embedding, embedding_model }`.

Provenance is **mandatory** — it is the audit handle (spot-check for hallucinated facts) and the
substrate for the user-ownership/portability moat (exportable, signed history).

## 6. Decisions locked

- **Compute:** server-first hybrid. Tiers 1–2 on the Rust server + Python sidecar compiler. On-device
  (GRDB + sqlite-vec + Apple Foundation Models) is a **later port**, not this MVP — but the design
  stays tiered/retrieval-based so the port is not a rewrite. Research confirms on-device retrieval is
  production-ready (sqlite-vec) and on-device compilation is hybrid-only (Foundation Models: 4096-tok
  context, foreground-only GPU) — both deferred.
- **Trigger:** async after each turn, **debounced to ~N=10 turns** per user (cheaper than per-turn,
  still "live").
- **Profile shape:** **flat semantic facts**, no graph edges (deferred).
- **Bounding:** non-destructive — unbounded Tier-1 storage with supersede-not-delete; cap only the
  per-turn injected slice via retrieval ranking.
- **Consent (MVP):** capture **auto-persists**; the compiled profile is injected to the user's own
  agents. **Per-agent consent scoping (what each agent may see) is explicitly deferred** to the next
  layer — `briefing.rs` consent levels + Phase 16 `connector_keys` are the scaffolding for it. (This
  layer is the eventual moat; the MVP proves value first.)
- **Compiler:** **reuse the existing `compiler.rs` + `llm.rs` (`OpenAiClient`)** — no Python sidecar.
- **Retrieval:** **Postgres FTS** (existing `search_vector`/`to_tsquery`); pgvector/embeddings deferred.
- **Models:** the server's existing `LlmClient` (`OpenAiClient`, env-configured) for extraction; no
  embedding model in the MVP (FTS-based retrieval).

## 7. Validation (the value test, dogfooded)

The MVP *is* the value test on real automatic data. Procedure: for a real task, brief a fresh agent
**with** the compiled profile vs **without** (and optionally vs raw-recent-messages), compare output
quality on the spike's rubric (usefulness from msg one · specificity · correct assumptions · no
hallucinated facts), plus a provenance spot-check of the compiled facts. Verdict: **confirmed / weak
/ killed** — same bar as the spike. This supersedes the spike's manual-export Task 14.

## 8. Out of scope (MVP) + backlog

**Out of scope now:** procedural memory / agent-authored skills + the `active→stale→archived`
curator (**fast-follow #1** — extends memory from declarative facts to learned procedures); graph
edges; on-device compute; per-agent consent ACL; external (outside-AINM) conversation import; the
curator lifecycle.

**Product backlog (recorded, not designed here — from the Hermes video):** BYO-subscription/model as
an onboarding wedge; agents as named participants in shared threads accruing competence; one identity
across many surfaces with interrupt/steer/queue message semantics; per-agent multi-model routing
(reasoning vs research vs delegation); a Kanban/async task surface with human-in-the-loop;
persona/naming at onboarding; reliability-as-moat for non-technical users; identity split
(soul/persona vs user-facts vs environment).

**Explicitly rejected:** Hermes' "trust the model, get out of the way" safety stance — unacceptable
for a multi-tenant hosted product holding many users' memory; we require real guardrails/approval
flows (which is also the consent moat).

## 9. Risks & open questions

- **FTS relevance may be too weak:** keyword `to_tsquery` over `search_vector` may not surface the
  right facts for conversational queries. Mitigation: derive a good query string (key terms from
  recent messages, not raw text); if relevance is poor in the value test, escalate to pgvector.
- **Compiler may under-populate:** the existing `compiler.rs` reconciliation is heuristic; if it
  doesn't yield well-categorized, confident facts the retrieval can rank, extend it minimally.
- **Extraction quality / hallucinated facts:** mitigated by mandatory provenance + spot-audit using
  the existing `LlmClient`.
- **Retrieval tuning:** the semantic×recency×salience weighting will need iteration; start simple,
  tune against the value test.
- **No new LLM/HTTP client on the Rust server today:** the sidecar avoids adding one to Rust for now.

## 10. Definition of done

1. A message sent in-app triggers, every ~N turns, an async compile that writes provenance-tagged
   semantic facts to `memory_nodes` (verified end-to-end against a running server).
2. A fresh agent's briefing returns a peer card + query-conditioned ranked facts under a token budget
   via the existing `ainm.memory.briefing` API.
3. A dogfooded with-vs-without comparison on a real task, scored on the rubric, yielding a written
   confirmed/weak/killed verdict + observed fact-hallucination rate.
