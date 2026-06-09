# Agent Grant (Onboarding Manifest) — Design

**Date:** 2026-06-09
**Status:** Approved (design); pending implementation plan
**Scope:** L1 — the per-agent grant manifest, in full, with thin *governed interfaces* to L2 (tiered memory) and L3 (skill accretion). L2/L3 depth are separate later specs.
**Build gate:** This design may be approved now, but **building is gated on Task 14** (the real-data compound-memory value experiment, `experiments/compound-memory-spike/`). Design now; build after the go/no-go.

---

## 1. Problem & Goal

AINM's defined moat is **portable, user-owned, consent-gated compound memory** — *"the accumulated cross-vendor understanding of the user, that onboards any agent"* (see [[product-pivot-memory-wedge-may-2026]], [[Compound Memory Vision]]). To date we've built the *engine* (capture → LLM-compile → typed graph → token-budgeted briefing) and the *enforcement boundary* (per-agent consent scoping inside the hosted runtime). Two legs are missing: a **portable/capability-gated** interface, and **proof the payload is worth defending** (Task 14).

This design addresses the first by reframing the moat around its **delivery surface**: the moment an agent connects, *the platform and the user define what that agent is endowed with* — which memory it receives (and how), which tools/skills it may use, what it may write back. That act is simultaneously the **wedge** (frictionless, attractive onboarding — "easier than Telegram") and the **moat** (a consent-scoped, signed endowment a pure-SDK competitor structurally cannot hand out, because it requires our hosting + consent + memory). This is the proven platform pattern of Slack-app-install / OAuth-scope-grant / object-capability tokens, applied to *memory + skills*.

**Goal:** A first-class **Agent Grant** — a per-`(user, agent)`, user-authored, host-enforced manifest declaring memory scope (with verbs), tool/skill grant, and write-back policy; signed by the user; resolved at connect; enforced on every call; listable and revocable. Connecting an agent becomes one tap (preset) with full per-agent control (override), and the agent arrives already knowing the user within its grant.

**Non-goals (YAGNI for this spec):** the deep L2 memory re-tiering / async consolidation (Karpathy "sleep-time agent"); the L3 skill-accretion *loop* (Voyager/Hermes verified-success skill library); emitting live UCAN tokens for cross-server consumption; a marketplace of agents. Each is a later spec; this design *reserves the interfaces* so they slot in without schema breakage.

## 2. Research grounding (2026-06-09 fan-out)

- **Karpathy** — the context window is the *only activation surface*; memory must be tiered (episodic → semantic/compound → procedural) and is inert until paged in; his Apr-2026 *llm-wiki* gist mirrors our compiler; "context improves over time" = consolidation quality, not raw volume. → **D2 (push the briefing in)**, and the L2 deepening note.
- **Hermes Agent / Voyager (NeurIPS'23) / Agent Workflow Memory (ICML'25)** — skills compound via a *verified-success* skill library, per-identity, retrieved by recency×relevance×success-rate; reflection promotes episodic→semantic→procedural. → the **L3 reserved interface** (skill-grant verbs + namespace).
- **OSS memory survey** (Mem0 / Letta / Graphiti / cognee / A-MEM / MemoryOS / MemOS) — *none* has signed/content-addressed/portable memory or real per-agent capability gating; all use mutable per-user stores. → confirms the white space. Mine: Graphiti **bi-temporal edges** + superseding, Mem0 multi-signal retrieval, cognee **permission-check-second** pipeline ordering (L2 spec).
- **Capability/manifest specs** — align identity+signing with **A2A AgentCard** (we already extend it; Ed25519 card signature), capability *token* shape with **UCAN** (DID `iss/aud`, `cmd`=verbs, `exp`, attenuation, revocation — matches our Ed25519/JCS stack), capability *negotiation* style with **MCP** `initialize` (declared, mutually-honored, here extended with explicit deny). The memory-verb grant (`read/derive/redact/export`) has **no precedent — we invent it.**

> Skeptic's note: a few fan-out specifics (e.g. "Hermes Agent 175k stars," some 2026 arXiv IDs) were **unverified** and are treated as leads, not facts. The load-bearing patterns (Voyager, AWM, A2A, MCP, UCAN, Graphiti) were URL-verified.

## 3. Architecture

### 3.1 The Agent Grant object

Keyed by `(grantor_account, subject_agent)`. Authored by the user at agent-add time (preset, optionally overridden), signed by the user's key.

```jsonc
AgentGrant {
  grantor:  "alice@ainm.local",        // whose context        (UCAN-shaped: iss)
  subject:  "coder@acme.dev",          // grantee agent        (UCAN-shaped: aud)
  preset:   "standard",                // minimal|standard|full|custom

  memoryGrant: {
    categories: ["identity","preferences","expertise","goals"],  // preset-expanded; override-able
    verbs:      ["read","derive"]                                 // see §3.2
  },
  toolGrant:   { allow: ["thread:send","memory:derive"], deny: ["memory:export"] },  // deny wins
  writeBack:   { memory: "propose", messages: true, inviteAgents: false, startThreads: false },
                                       // memory: "none" | "propose" (user confirms) | "auto"
  skillNamespace: "grant://alice@ainm.local/coder@acme.dev/skills",  // L3 — RESERVED, not built
  skillGrant:  { use: [], learn: false },                            // L3 — RESERVED verbs

  lifecycle:   { createdAt, expiresAt: null, revokedAt: null },
  provenance:  { userConsentTimestamp, signature }   // JCS + Ed25519 over the canonical grant
}
```

### 3.2 The memory verbs (the moat primitive)

| Verb | Meaning |
|------|---------|
| `read` | raw facts in the granted categories |
| `derive` | **only** LLM-derived summaries/briefing — never raw source facts (privacy-preserving tier: agent learns *"Alice prefers X"*, not the source messages) |
| `redact` | may request forget/deletion of facts |
| `export` | may pull a portable signed bundle (bulk egress) |

Presets (reusing the existing `ConsentLevel`→category mapping):
- **minimal** → `derive` on `identity` only
- **standard** (default) → `read+derive` on `identity, preferences, expertise, goals`; no `redact`/`export`
- **full** → `read+derive` on all categories; `redact` allowed; `export` still off by default
- **custom** → user-picked categories × verbs

### 3.3 Enforcement — the host boundary (the moat made real)

The grant is stored server-side and **enforced on every memory/tool operation.** The existing consent gate (which already resolves per-requester scopes for the briefing + memory tools) **generalizes** to: resolve the `AgentGrant` for `(grantor, subject)` → enforce `categories × verbs` on (a) the injected briefing, (b) every `ainm.memory.*` call, (c) tool dispatch / hosted-runtime tool registry. Because we own the loop, an agent **cannot** exceed its grant. Revocation (`revokedAt`) takes effect on the next operation — immediate, server-enforced.

The grant is **JCS-signed by the user's key** (reusing `signing`) → tamper-evident + auditable. Fields are **UCAN-shaped** (`iss/aud/cmd/exp`) so the *same object* can later be emitted as a portable UCAN capability token for a foreign/federated agent without our host in the path (D1).

### 3.4 Onboarding & lifecycle flow (the wedge)

```
add agent (existing AgentDiscovery) ─▶ pick preset (1 tap)
                                            │  └─▶ [optional override: categories/verbs/tools/write-back]
                                            ▼
                                   AgentGrant signed by user's key, persisted
                                            │
   connect / first-address ─▶ server resolves grant ─▶ (a) GRANT SUMMARY to the agent in the
   initialized handshake (MCP-style "here is your scope")  + (b) PUSHED scoped briefing on the
   first turn  (Karpathy: page context in; don't wait for a pull)
                                            │
   "Manage agent access" UI (mirrors Active Sessions) ─▶ list / edit / REVOKE per agent (immediate)
```

### 3.5 Governed L2 interface (thin)

The grant is the *access contract* over the memory tiers we already have (episodic logs → compound graph → token-budgeted briefing). **No new tiering is built here.** The Karpathy/Graphiti deepening — bi-temporal edges, async "sleep-time" consolidation, heat-based GC — is a **separate later spec** that the grant will keep governing unchanged.

### 3.6 Governed L3 interface (thin)

- `toolGrant` (allow/deny over current server tools / memory verbs / curated MCP) is **real now**.
- `skillNamespace` + `skillGrant` (`use`, `learn`) are **reserved** so the Hermes/Voyager accretion loop (verified-success → skill doc → success-weighted retrieval, scoped per-`(user,agent)`) slots in later **without a schema change.** The loop itself is a later spec.

## 4. Components & File Map (indicative; finalized in the plan)

**Server (Rust, `server/src/`):**
- New migration — `agent_grants` table (grantor, subject, preset, memory_categories, memory_verbs, tool_allow, tool_deny, write_back, skill_namespace, lifecycle, signature; unique on `(grantor, subject)`).
- New pure module (e.g. `grant.rs`) — preset→(categories×verbs) expansion; verb/category enforcement decision (`allowed(grant, category, verb) -> bool`, deny-wins); UCAN-shaped canonicalization for signing. Exhaustively unit-tested.
- Generalize the consent gate: the briefing (`briefing.rs` / `generate_briefing`), the memory-method handlers (`server.rs` `ainm.memory.*`), and the hosted runtime tool registry (`runtime_server.rs` / `tools_memory.rs`) resolve + enforce the `AgentGrant` instead of a bare `ConsentLevel`. (`ConsentLevel` becomes the *preset*, not the whole story.)
- Grant CRUD REST endpoints (authenticated via the account auth-session token, like the C/AT endpoints) + grant-summary in the `ainm.initialized` response + push-briefing on first turn.
- Signing: reuse `signing` (JCS + Ed25519) for the user-signed grant.

**iOS (`ios/`):** preset picker + optional override at agent-add time (extends AgentDiscovery); a "Manage agent access" screen (mirrors the Active Sessions list/revoke UX); store/display the grant. **The grant is canonicalized + signed client-side** (the user's Ed25519 private key lives in the iOS Keychain via `KeyManager`), then uploaded over an authenticated session; the server stores + enforces but the signature is the user's (tamper-evidence + the UCAN-emittable path). A server-side authenticated edit path may set/replace a grant without a fresh client signature for non-crypto fields (documented as a trust trade-off in the plan).

**Connector SDK / reference connectors:** no behavioral requirement, but the connector should **read** the grant-summary from the handshake (so an author can see its scope) — additive, mirrors how it reads `senderType`/`verified`.

## 5. Testing Strategy

- **Pure units (carry the weight):** preset→categories×verbs expansion; `allowed()` enforcement matrix across `read/derive/redact/export × granted/not-granted × deny-wins`; grant canonicalization + signature round-trip.
- **Server integration (black-box WS, DB-or-skip):** an agent with `standard` gets a *derive-only* briefing and is blocked from reading raw facts; `redact`/`export` denied unless granted; revocation immediately blocks the next memory call; override narrows/widens correctly. Reuse `ConstantMockLlmClient` / `AINM_LLM__MOCK`.
- **iOS:** preset+override produces the right grant object; "manage access" lists + revokes.
- **Out of scope:** the two known pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`).

## 6. Success Criteria

1. A user adds an agent with one tap (preset) **or** a per-agent override (categories × verbs × tools × write-back); the grant is signed by the user's key and persisted.
2. On connect, the agent receives a grant summary and a **pushed** scoped briefing; it operates **only** within its grant — every memory/tool call is host-enforced against the grant; over-grant attempts are denied (visible, not silent).
3. The `derive` verb yields summaries without exposing raw facts; `export`/`redact` are gated and off by default.
4. The user can list and **revoke** per-agent access; revocation is immediate.
5. The grant applies **uniformly** to the hosted assistant and third-party connector agents.
6. No regression in existing suites; the grant object is UCAN-shaped so a later portable-token spec needs no schema change; L2/L3 deepenings slot into the reserved interfaces.

## 7. Decisions (locked)

| # | Decision | Rationale |
|---|---|---|
| **Scope** | Manifest-centered (L1 full; thin governed L2/L3 seams) | Ships the wedge+moat object fastest; L2 partly exists; L2/L3 deepen later |
| **Authoring** | Presets + per-agent override | One-tap wedge + bespoke "we define exactly what THIS agent gets" |
| **D1 token** | Server-enforced JCS-signed grant **now**, UCAN-*emittable* later | We own the host → server enforcement suffices + is simplest; UCAN-shape avoids a future rewrite for cross-server/portable tokens |
| **D2 delivery** | **Push** scoped briefing at connect/first-turn + grant summary to agent | Karpathy: context is inert until paged in; push makes "arrives knowing the user" real |
| **D3 target** | Grant uniform across hosted + connector; validate on hosted, win is connector | Connector is the network/moat surface; hosted is the easy harness |
| **D4 verbs** | `read / derive / redact / export`; standard = `read+derive` | `derive` is the privacy-preserving, still-useful default |

## 8. Risks & Open Questions

- **Override UX friction vs control.** Per-agent overrides must not turn every add into a chore; the preset one-tap path is the default and must stay frictionless. (Mitigation: override is progressive disclosure behind the preset.)
- **`derive`-only enforcement is only as private as the briefing.** A `derive`-scoped agent must never see raw facts leaking through the briefing text. (Mitigation: the briefing already runs through the consent filter; the grant tightens it; add negative leak tests, as the per-agent consent scoping work did.)
- **Grant ≠ proof of value.** The manifest governs a payload whose real-data value is still unproven — **Task 14 gates the build.** An exquisitely-scoped grant over thin memory is a neat trick, not a moat.
- **UCAN-later, not now.** We sign server-side; if a partner needs offline-verifiable delegation before the portable-token spec lands, this is the first thing to pull forward.
- **Write-back `auto` is dangerous.** Default to `propose` (user confirms); `auto` memory write-back is opt-in and should be visible/audited.
