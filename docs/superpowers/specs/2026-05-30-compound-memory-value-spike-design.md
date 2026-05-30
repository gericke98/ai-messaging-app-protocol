# Compound Memory Value Spike — Design

**Date:** 2026-05-30
**Status:** Approved design (throwaway spike)
**Author:** brainstorming session (Santiago + Claude)

## 1. Why this exists

The project is re-prioritizing from "open federated protocol + reference iOS client" toward a
**product**: an agent-native messaging app whose defensible moat is **portable, user-owned,
consent-gated compound memory** — a continuously-distilled, cross-agent model of the user that
makes any new agent instantly useful and travels across rival vendors.

That moat rests on one existential assumption: **that compound cross-agent memory actually makes a
fresh agent visibly better.** If it doesn't, the crypto, capability tokens, consent UX, hosted
runtime, and marketplace are all moot. This spike attacks that single assumption with the least
possible code, and is expected to be **thrown away** after it returns a verdict.

This is *not* the production memory layer. v3.1/v3.2 already shipped protocol-level memory
(archival facts, provenance, iOS memory UI) and Phase 20 specs the production groundwork. This
spike deliberately reuses none of that machinery — it answers a yes/no question fast.

## 2. Hypothesis & decision rule

> **H:** A distilled, cross-agent compound-memory profile makes a brand-new agent visibly more
> useful on a real task than the same agent with (a) no memory and (b) raw transcript excerpts —
> *at equal or smaller context-token budget.*

Three arms, judged on **one real task chosen at run time**:

| Arm | New agent's injected context | Tests |
|-----|------------------------------|-------|
| **A — Cold** | nothing | baseline |
| **B — Raw** | relevant raw transcript excerpts (budget = N tokens) | "does context help" |
| **C — Compound** | distilled compound profile (budget ≤ N tokens) | "does *compounding* help" |

- **Thesis CONFIRMED:** C is visibly more useful than B at equal-or-smaller budget, across both
  consumer models. → proceed to invest in the memory layer as the wedge.
- **Thesis WEAK/KILLED:** C ≈ B (distillation adds nothing over raw context), or C only beats A
  (any context helps, compounding doesn't). → rethink the moat before building further.

The budget constraint is the teeth. "More context helps" (C or B > A) is trivially true and is
**not** the thesis. The signal that proves the moat is **C > B** — that distilled understanding is
more information-dense than raw history.

## 3. Experiment structure (the validity core)

**Consumer is the outer loop; memory is the inner variable.**

```
for consumer in [GPT-5.5, Gemini 3.1 Pro]:        # cross-vendor: neither is a source vendor
    for arm in [A_cold, B_raw, C_compound]:        # only the injected context varies
        run(task, consumer, arm, same params)      # budget counted in consumer's own tokenizer
        record(output, input_tokens, arm_id_hidden)
```

- **Why two non-Claude consumers:** the source history includes Claude transcripts. A Claude
  consumer would measure intra-vendor familiarity ("Claude reading Claude"), not portability.
  The thesis is *cross-vendor* memory, so consumers must be vendors that contributed no source
  data, and two vendors are needed to show the gain isn't model-specific.
- **Why Claude distills:** the distiller is *not* a consumer, so using the strongest summarizer to
  build the profile introduces zero "model uses memory it authored itself" bias.
- **Budget parity:** for each consumer, count B's and C's context tokens in *that consumer's own
  tokenizer*; enforce `C_tokens ≤ B_tokens`. Cross-model token differences never enter a single
  comparison (they'd be a confound), because all three arms for a given comparison run on the same
  consumer model.

## 4. Architecture / pipeline

```
ChatGPT export (conversations.json) ─┐
Claude export   (conversations.json) ─┴─► ingest ──► normalized message log
                                                      {conv_id, msg_id, role, text, ts, source}
                                                          │
                                                          ▼
                                            distill (Claude Opus 4.8, map-reduce)
                                                          │  Compound Profile (JSON, provenance per fact)
                                                          ▼
                              ┌──────────── harness (per consumer × 3 arms) ───────────┐
   task (picked at runtime) ─►│  A: no ctx │ B: raw excerpts ≤N │ C: profile ≤N tokens │
                              └───────────────────────────────────────────────────────┘
                                                          │  outputs + token logs (arm labels hidden)
                                                          ▼
                                          judge: blind side-by-side + rubric (human)
```

## 5. Components (each a small, independent module)

- **`ingest`** — parse ChatGPT `conversations.json` + Claude export into one normalized message
  log. Every message tagged `source` (`chatgpt`/`claude`), `conv_id`, `msg_id`, `ts`. Pure data,
  no LLM.
- **`distill`** — map-reduce over the log → **Compound Profile**. Map: chunk each conversation
  into ~4–8K-token windows on turn boundaries, extract candidate facts (temp 0), each with
  mandatory provenance. Reduce: one reconcile pass dedupes/merges, resolves contradictions by
  recency (mark superseded, never delete). Output is one JSON fact list.
- **`harness`** — runs the chosen task through each consumer × {A,B,C}; enforces per-consumer
  token-budget parity *before* the call; writes outputs behind randomized arm IDs.
  - **Budget N:** a single fixed ceiling chosen at run time (~3–4K context tokens — a generous but
    realistic memory injection). Arm B fills up to N; arm C must use ≤ N.
  - **Arm B excerpt selection:** deliberately naive — keyword overlap with the task + recency,
    truncated to N. B represents "just dump relevant raw chat." Sophisticated selection is exactly
    the value arm C is supposed to provide, so B must *not* get it, or the test is rigged in C's
    favor.
- **`judge`** — emits a blind side-by-side table; reveals the rubric; records human scores; then
  unblinds the mapping.

## 6. Compound Profile shape (right-sized for a spike)

Flat list of atomic facts — LLM-friendly to emit and inject. Provenance is **mandatory** (it is
the hallucination-audit handle).

```json
{
  "subject": "user",
  "facts": [
    {
      "id": "f_017",
      "category": "identity|preferences|projects|decisions|patterns|relationships",
      "statement": "Prefers Rust over Go for backend services",
      "status": "active",            // active | superseded
      "superseded_by": null,         // fact id, if superseded
      "first_seen": "2025-11-02",
      "last_seen": "2026-04-18",
      "provenance": [
        { "source": "chatgpt", "conv_id": "c_44", "msg_id": "m_310", "ts": "2026-04-18" }
      ]
    }
  ]
}
```

## 7. Distillation pipeline — core vs deferred

The production design (embedding clustering, NLI entailment verification, full bi-temporal
`valid_at/invalid_at`, relevance-scored greedy packing) is **too much for a throwaway whose judge
is the user.** Keep the cheap, high-leverage parts; defer the rest until v0 proves murky.

**Core (build now):**
1. Map-reduce extraction (chunk → per-chunk fact extraction, temp 0).
2. **Mandatory per-fact provenance** — enables eyeball hallucination audit (substitutes for an
   automated entailment pass at spike stage).
3. Single reconcile pass — recency-wins; mark `superseded`, don't delete.
4. **Injection:** if the active profile fits the budget, inject it whole as terse grouped bullets
   (strip ids/timestamps/provenance from the injected text — keep them only in the store). Add
   relevance selection only if it doesn't fit.

**Deferred (only if v0 is ambiguous):** embedding-based clustering/relevance scoring, NLI
cited-span entailment verification, full bi-temporal validity axes.

## 8. Judging protocol & rubric

- Same model, temperature, task, and system scaffold across arms — only injected context differs.
- Outputs presented in **randomized, unlabeled order**; human scores before unblinding.
- **Rubric (1–5 each):** usefulness from message one · specificity to the user · correctness of
  assumptions made · absence of hallucinated/wrong "facts" about the user.
- **Provenance audit:** spot-check whether `distill` fabricated any facts by following each cited
  `msg_id` back to the source span. Fabrication rate is itself a key learning.

## 9. Fairness controls (where spikes lie to you)

- Budget parity B vs C (C ≤ B), counted in the consumer's own tokenizer.
- Single variable per comparison: only injected context changes within a consumer.
- Blind, randomized output order at judging time.
- Provenance-backed hallucination check on the profile.
- Count input/context tokens for the parity gate; log output tokens for cost transparency only.

## 10. Stack decisions

- **Language:** Python, hand-rolled ~150–200 line script. **No eval framework** (promptfoo /
  DeepEval / Inspect are overhead for a one-shot, human-judged A/B). Don't fork mem0/Zep — they
  target live turn-by-turn ingestion, not batch distillation of static exports.
- **Libraries:** `litellm` (one call across all three vendors), `tiktoken` + Anthropic & Gemini
  `count_tokens` APIs (per-vendor budget parity), `pandas` (blind side-by-side table). An
  embedding model only if the deferred relevance-selection path is needed.
- **Models:** **Claude Opus 4.8** distills → **GPT-5.5** + **Gemini 3.1 Pro** consume. *Competitor
  model versions are current-as-of-research (2026-05-30) and will be confirmed against live API
  availability when keys are wired.*

## 11. Out of scope (YAGNI for a value spike)

Crypto / signing, capability tokens, consent UI, the iOS app, the Rust server, the AINM protocol,
RAG / embeddings infra (unless the deferred path triggers), live capture, the marketplace,
multi-task scoring, hosted runtime / sandboxing. If the spike confirms value, *those* become the
next sub-projects — each with its own spec.

## 12. Dependencies & open items (resolved at run time)

- **User provides:** a ChatGPT data export + a Claude data export (account → export).
- **API keys:** Anthropic (distill) + OpenAI (consumer) + Google (consumer).
- **The real task:** chosen together at run time — should be something the user actually does,
  where context genuinely changes the output (not a recall quiz).

## 13. Definition of done

A single results artifact (table + the saved outputs) for one real task, across two consumer
models and three arms at budget parity, with the human rubric scored and a written one-paragraph
verdict: **confirmed / weak / killed**, plus the observed profile-hallucination rate.
