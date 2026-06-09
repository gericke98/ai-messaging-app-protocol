# AINM — Definitive Public-Launch Ship Roadmap

**Date:** 2026-06-09
**Author:** Lead architect synthesis (4 candidate roadmaps × 3 adversarial judge panels)
**Input of record:** [`2026-06-09-ainm-state-and-gaps.md`](./2026-06-09-ainm-state-and-gaps.md) — all gap/risk citations below trace to its §5 (gap tables, sizes S/M/L) and §7 (ranked risks).
**Status:** This is the **parent** roadmap. Each phase (M1…Mn) gets its own spec + plan via `superpowers:brainstorming` → `superpowers:writing-plans` before execution.

This roadmap takes **critical-path** as the structural base (DAG-first, explicit slack/owner attribution), grafts the **product-spine-first** v1 cut-line (AMO/key-rotation/federation debt all deferred), the **moat-led** FAIL-branch collapse table, and the **risk-retire** "Task 14 before any code investment" intent. Where a draft idea is rejected, the rejection is stated in one line.

---

## 1. Launch definition

**AINM v1 is publicly launched when** a person can download the native iOS app from the **App Store**, sign up and get a cryptographic identity, talk to AI agents and other humans in threads with streaming responses and tool-call visibility, **receive push notifications when backgrounded**, and use the product **free** by bringing their own LLM key — all served by a **single hosted server we operate** (Fly.io, real TLS, rate-limited, observable), with a **public landing page** carrying the launch headline, and with **Terms of Service, a Privacy Policy, and an accurate App Store privacy nutrition label** filed. Self-hosting is documented in parallel but is **not** a launch gate. Whether the launch headline is *"portable, consent-gated, user-owned memory"* or *"the open messenger where humans and AI agents are equal participants"* is decided by the **Task-14 gate (M0)**, not assumed.

---

## 2. Locked decisions

1. **Ship target = PUBLIC LAUNCH.** App Store submission + landing page/marketing. Highest bar: review, push, legal (ToS/privacy/nutrition label), polish, support.
2. **Posture = HOSTED-FIRST.** We run one hosted server; self-host is documented in parallel, not a launch gate.
3. **Platform = iOS ONLY.** Existing native SwiftUI app. No web/Android in v1.
4. **Moat gate = RUN TASK 14 FIRST.** Hard go/no-go at step zero. PASS → build Agent Grant + portable memory; the moat is the headline. FAIL → launch the messenger, redesign the moat. **Agent Grant (0/9, plan `docs/superpowers/plans/2026-06-09-agent-grant-server.md`) MUST NOT be built before Task 14 passes.**
5. **Monetization = FREE BETA, BYO-KEY.** Users bring their own LLM key; BYO path + quota ledger (sub-project C) exist. **No billing/IAP to build for v1.**
6. **Federation = SINGLE-HOME-SERVER v1.** No live cross-server federation at launch. **All federation-gated security debt deferred to v2:** unsigned cross-server attestation, IdentityStore→DB for *federation* re-verification, mTLS CA automation + warn-not-drop, key rotation/`key_history`.
7. **Connector onboarding = INSTANT-TOKEN FAST PATH.** BotFather-style; working session in <60s; Ed25519 key-gen hidden during a UI/CLI animation; relaxes the strict account+key-bind ceremony. Net-new, part of the launch story.

---

## 3. The Task-14 gate (M0 / step zero)

Task 14 runs **before any feature code is written**, honoring locked decision 4 and the risk-retire intent that no engineering investment precedes the verdict. **Resolution of the judge-flagged contradiction** (risk-retire ran it on no infrastructure; critical-path ran it after a full hosted deploy): Task 14 needs a *running briefing pipeline against real data with live LLM keys* — it does **not** need the hosted public deployment, real TLS, CI, or rate limiting. It runs against a **local, throwaway instance** (`docker-compose` + a real Postgres + real LLM keys), operated by a human. The one security fix it depends on is the **hardcoded `dev-insecure-secret` TokenSigner** only if the local instance is exposed; for a localhost-only run it is not even required. Therefore Task 14 truly goes first, with zero idle time.

**What it runs** (§5a *Task 14 real-data value proof*, M; §7 risk #1 *the moat is unproven*):
- The compound-memory briefing A/B: **arm C** (compound briefing) vs **arm B** (keyword/raw retrieval) vs **arm A** (no memory), using the existing `experiments/compound-memory-spike/` harness.
- Seed **200–500+ real cross-vendor facts** (not the 20-fact synthesized profile that §7 flags as the only run to date), live LLM keys, a **production-grade judge model distinct from the briefing model**, logged and reproducible.

**Pass / fail criteria:**
- **PASS:** Arm C mean briefing score ≥ **4.2 / 5.0** *and* beats Arm B by ≥ **0.5** at ≥ 200-fact real-profile scale, judged by a distinct production model, reproducibly logged. Compare against the Mem0 94.4% competitive bar (§4 LongMemEval-V2 / BEAM) for honesty, not as a pass condition.
- **FAIL:** Anything short of the above, or the experiment cannot be run on real data.
- **Measurement rigor (added 2026-06-09 — a 0.5 margin on a 5-point LLM-judge scale is inside single-run noise):** ≥ 20 probe questions per arm; ≥ 3 independent runs (vary probe order / judge seed via prompt variation); judge scores arms **pairwise per-probe, blind to arm labels, with a pinned rubric**; report mean ± a dispersion measure across runs. The 0.5 margin must hold on the cross-run mean, not a single roll. Also note what the gate does and does not test: it proves/refutes the **compound-briefing quality claim** (arm C vs B); the consent/portability layer is gated on it by the locked binary decision, not because the experiment measures consent value.

**The branch (both branches are shown wherever they diverge below):**
- **PASS → MOAT TRACK.** Build the Agent Grant (M3) and a minimal signed memory export (M6). The moat becomes the launch headline: *"The only messenger where your memory follows you — portable, signed, and you control which agents see what."* Critical path lengthens by M3.
- **FAIL → MESSENGER TRACK.** Agent Grant and AMO **do not execute**. Memory UI is labeled "coming soon," no broken/user-visible memory path. Launch headline becomes *"The open messenger where humans and AI agents are equal participants — cryptographic identity, BYO-key."* Critical path shortens (skip M3). Moat is re-specified post-launch (LongMemEval-V2 benchmarking, higher-quality seeds, new Task-14-equivalent before any Agent Grant build is re-committed).

> Rejected (risk-retire): running Task 14 as M0 with exit criteria requiring real data but *before* a runnable pipeline — there was no instance to run against. Resolved by a local throwaway instance.

---

## 4. Phased roadmap

### Dependency DAG

```
              ┌─ [M0 Task-14 gate] ── PASS ─┐                     FAIL ─┐
              │   (step zero, local)        │                          │
              ▼                             │                          │
   M1 Foundation: Security + CI ────────────┼──────────────────────────┤
        │                                   │                          │
        ├──▶ M2 Hosted Deploy (Fly) ──┐     │                          │
        │      + Observ + Rate-limit  │     │                          │
        │                             │     ▼                          │
        ├──▶ M4 iOS Push + Xcode 26 ──┤   M3 Agent Grant (PASS only) ──┤
        │      (parallel, iOS owner)  │     │  (server + iOS grant UX)  │
        │                             │     │                          │
        └──▶ M5 Connector Fast-Path ──┤     │                          │
               (slack; after M2)      │     │                          │
                                      ▼     ▼                          ▼
                            M6 Moat Export + Memory UI (PASS) / skip (FAIL)
                                      │
                                      ▼
                            M7 iOS Polish + Account Deletion
                                      │
                                      ▼
                            M8 Legal + App Store Submission ──▶ PUBLIC LAUNCH
```

**Critical path (PASS):** `M0 → M1 → M2 → Task-14-already-done → M3 → M6 → M7 → M8 → LAUNCH`
**Critical path (FAIL):** `M0 → M1 → M2 → M7 → M8 → LAUNCH` (M3, M6 dropped)
**Always-parallel after M1:** M4 (iOS push + Xcode 26, iOS owner) runs alongside M2/M3/M5 — different owner, no shared state.
**Genuine slack:** M5 connector fast-path (developer adoption, not an App Store gate — can slip post-launch if needed); legal copy drafting (can start day one).

---

### M0 — Task-14 Go/No-Go Gate `[Moat]`

**Goal:** Decide the entire product story before spending engineering budget.
**Gaps/risks closed:** §5a Task 14 real-data value proof (M); §7 risk #1.
**Exit criteria:** Pass/fail per §3, numeric result logged and reproducible; founder decision documented (PASS → moat headline + M3/M6 enabled; FAIL → messenger headline + M3/M6 dropped, moat re-spec scheduled post-launch).
**Size:** M (human operator; real keys; real data; local instance).
**Dependencies:** none.
**Track:** Moat. **Parallelizes:** no — everything bifurcates on this; it is step zero.

---

### M1 — Foundation: Critical Security Floor + Server CI `[Server / Infra]`

**Goal:** No code reaches a hosted deployment that carries a forgeable secret or cannot be automatically verified.

> **2026-06-09 — EXECUTED (quick-wins slice) + claim corrections.** Every item below was
> verified against code before fixing; ~half of the original list did not survive contact.
> Full claim-by-claim record: [`2026-06-09-m1-security-floor-ground-truth.md`](./2026-06-09-m1-security-floor-ground-truth.md).

**Gaps/risks closed (as corrected by ground truth):**
- §5c Hardcoded `dev-insecure-secret` TokenSigner (S) — ✅ DONE. Was only in the legacy `start()`/UAT paths (prod path was already `prod_guard`-gated), but legacy mode at `main.rs:1376` could ship it in production. Replaced with shared env-or-random helper; literal gone.
- §5b/§5c `AINM_LLM__API_KEY` not gated by `prod_guard` (S) — ✅ DONE (TDD).
- §5c Bubblewrap "silent `SandboxMode::None` degradation" — **claim was FALSE**: the launcher already fail-closes; servers were warn+*skipped*, never run unsandboxed. ✅ Shipped the real fix: in production, a configured MCP server that fails to start aborts boot (`build_providers_checked(strict)`); dev keeps warn+skip (bwrap doesn't exist on macOS — the originally-prescribed unconditional hard-error would have broken local dev).
- §5b *No server CI* (M, the CI portion) — ✅ DONE: fmt --check + clippy `-D warnings` (workspace made clippy-clean) + tests with Postgres 16 service container + a grep-guard that fails CI if DB tests self-skip.
- §3 `ring` ≥ 0.17.8 DoS advisory (S) — ✅ retired by verification (0.17.14).
- §5a `ainm.memory.graph.query` ungated — **claim was MISDIAGNOSED**: `query_nodes` is owner-scoped to the WS-handshake-authenticated caller; no cross-user read exists. ✅ Isolation invariant pinned by regression test. Consent-*verb* enforcement on self-reads is exactly M3's job (a blanket gate now would break the legitimate connector self-read path).
- §3/§5b embedded migrations fix (S) — ✅ DONE; was *worse* than claimed (silent skip when CWD ≠ `server/`).
- §7 risk #2 *non-green integration suite* — **the `#[ignore]` prescription was wrong and would have buried two real bugs**: against a live Postgres both tests FAIL (pre-existing). ✅ Root-caused and fixed: (1) `run_compilation` reported deduped-to-existing candidates as `nodes_created` (result + audit row both wrong); (2) `test_graph_traversal` used direction `"outbound"` — not in the schema enum — silently traversing `"both"`, with assertions contradicting the deliberate include-start-node subgraph contract.

**Exit criteria (verifiable):**
- ✅ No `dev-insecure-secret` anywhere in `server/src`; `prod_guard` rejects missing TokenSigner secret and missing LLM key in production.
- ⬜ `cargo test -p ainm-server` + `cargo clippy` + `rustfmt --check` run on every push to `main`; CI badge green; no silent skips. *(workflow committed; first run needs push)*
- ✅ `cargo tree -i ring` resolves ≥ 0.17.8 (0.17.14).
- ✅ Production boot aborts when a configured MCP server cannot start sandboxed; nothing ever runs unsandboxed (pre-existing fail-closed launcher + new strict mode).
- ✅ Cross-identity graph isolation pinned by regression test. *(Grant-verb denial codes are an M3 criterion — note: the `-32063` code originally specified here does not exist; the family uses `-32062`. M3 must allocate a real code.)*
- ✅ Migrations embedded via `sqlx::migrate!`; binary carries its schema.
- ✅ 327 workspace tests green against live Postgres, clippy clean, workspace rustfmt'd.

**Size:** S+S+S+M+S+S+S = **M.**
**Dependencies:** M0 verdict (so we know whether M3 is on the path), but the work itself depends on nothing.
**Track:** Server/Infra. **Parallelizes:** none — it is the gate everything else builds on. iOS track (M4) can start its Xcode 26 work in parallel (compiler-only, no server dependency).

> Note: §5b *No server CI* is split — the CI wiring lands here (M1); the Fly.io deploy/observability/rate-limit portion is M2.

---

### M2 — Hosted Deployment + Observability + Rate Limiting `[Infra / Server]`

**Goal:** A reliable, observable, abuse-resistant single-node hosted service, reachable by real users without manual intervention.
**Gaps/risks closed:**
- §5b Hosted deployment pipeline (M, the deploy portion) — Fly.io + Fly Managed Postgres; real TLS (Fly-managed, **not** the repo's demo self-signed certs); SIGTERM graceful drain exercised by rolling deploy.
- §5b Observability (M) — `axum-tracing-opentelemetry` + self-hosted Langfuse; p99 + error-rate dashboards.
- §5b Rate limiting (M) — `tower-governor` GCRA per-account/IP on WS + REST.
- §3 `tokio-tungstenite` 0.26 → 0.29 (compile-time-safe; do it before the release branch to avoid a launch scramble).
- §7 risk #5 (partial) — single-node beta is **accepted**; what closes here is operational blindness and the unprotected abuse surface. HA/multi-replica is explicitly v2.

**In-scope clarification (single-node reliability, NOT federation):** §5c In-memory `IdentityStore` → Postgres persistence. This is included **for single-node restart reliability only** (identities survive a deploy/restart so re-auth works), **not** for cross-server federation re-verification, which remains v2. This is the one piece of "identity store" work in v1; the federation re-verification semantics are deferred.

> Rejected (product-spine-first / critical-path ambiguity): framing IdentityStore persistence as federation debt. It is dual-purpose; the single-node-reliability slice ships now, the federation slice is v2. Stated explicitly to avoid the contradiction both panels flagged.

**Exit criteria (verifiable):**
- `fly deploy` from CI succeeds on merge to `main`; `/health` returns 200; migrations run from the embedded binary on a fresh Fly Managed Postgres.
- TLS is Fly-managed (no self-signed repo cert in the running path).
- `tower-governor` returns HTTP 429 under a k6 100-VU WS burst.
- OTel traces visible in Langfuse for a live LLM turn; p99/error-rate dashboards exist.
- `IdentityStore` survives a server restart (Postgres-backed) on the live instance.
- Smoke test passes on the live instance: register → connect agent → exchange streaming messages.

**Size:** M+M+M (+ S tungstenite, + M IdentityStore) = **L.**
**Dependencies:** M1 (no hosted deploy before the secrets gate and CI are green).
**Track:** Infra/Server. **Parallelizes:** with M4 (iOS) and, on PASS, with M3 server work once stable.

---

### M3 — Agent Grant Core + Grant UX `[Server / iOS — PASS BRANCH ONLY]`

**Goal:** Per-agent, user-signed, consent-scoped memory grants — live, enforced, user-controlled. The moat made real.
**Branch:** **Executes only if Task 14 (M0) PASSED.** On FAIL this milestone does not exist; the path skips to M7.
**Gaps/risks closed:**
- §5a Agent Grant layer (L) — all 9 tasks of `docs/superpowers/plans/2026-06-09-agent-grant-server.md`: migration 008, pure `grant.rs`, verb enforcement (read/derive/redact/export) on all four memory methods, signed CRUD endpoints.
- §5a `ainm.memory.graph.query` (S) — already gated in M1; now enforced through grants (`read` verb), removing the temporary blanket gate.
- §7 risk #4 (the moat-relevant slice) — eliminates the un-enforced memory surface.

**Exit criteria (verifiable):**
- Migration 008 applied; `grant::tests` pass; black-box `agent_grant_e2e.rs` compiles, self-skips without Postgres, passes on CI with the Postgres service.
- An agent with `minimal` preset is blocked from `graph.query`; `export` denied by default; `briefing` gated on `derive`.
- `PUT/GET/DELETE /accounts/agent-grants/{agent}` authenticate via bearer + verify the user's Ed25519 signature.
- iOS: agent-add flow shows a one-tap preset picker (+ override); "Manage agent access" screen lists and revokes per-agent grants; the grant is signed client-side with the user's Keychain key and uploaded via PUT; revocation takes effect on the next memory operation.
- No regression in existing suites (pre-existing `#[ignore]` tests excluded).

**Size:** L (server, tasks 1–9) + M (iOS grant UX) = **L–XL.**
**Dependencies:** M0 PASS; M1 (CI must run the new tests); M2 (live server to enforce against).
**Track:** Server + iOS. **Parallelizes:** iOS grant UX builds in parallel with server tasks once the API contract is frozen (after plan Task 7). Server M3 work overlaps M4 (iOS push) and the tail of M2.

> Rejected (product-spine-first): treating Agent Grant as a parallel track that "merges if proven in time / else v1.1." That softens the locked binary go/no-go into a timeline question and invites speculative pre-proof building. M3 is a hard branch, not a race.

---

### M4 — iOS Push Notifications + Xcode 26 Migration `[iOS]`

**Goal:** Make the app a real, App-Store-submittable messenger that notifies users in the background, built on the mandated toolchain.
**Gaps/risks closed:**
- §5b iOS push notifications / background delivery (L) — APNs, `UNUserNotificationCenter`, background refresh, server-side push on thread activity.
- §5d `ainm.ping`/`pong` keepalive (S) — confirm/wire the handler; required for iOS background WS correctness.
- §5b/§3 Xcode 26 migration + `AinmEscrow.xcframework` rebuild (S–M) — Apple mandate since 2026-04-28; rebuild the UniFFI xcframework with the Xcode 26 toolchain; audit `nonisolated async` sites for SE-0461.

**Exit criteria (verifiable):**
- App builds and archives cleanly under **Xcode 26 / iOS 26 SDK** (deployment target iOS 18); no upload-blocking deprecation warnings.
- `AinmEscrow.xcframework` rebuilt with the Xcode 26 toolchain; **live-device** escrow + biometric recovery regression passes on a physical device.
- First-launch `UNUserNotificationCenter` permission prompt; APNs device token registered server-side per account.
- A message to a backgrounded device delivers a push within 5s on a **physical device**; tapping deep-links to the correct thread.
- `ainm.ping`/`pong` round-trip confirmed; WS survives background entry.

**Size:** L (push) + S (keepalive) + S–M (Xcode 26) = **L.**
**Dependencies:** M1 (push tokens must register against a non-forgeable-secret server). Xcode 26 work has **no** server dependency — start at M1 kickoff.
**Track:** iOS. **Parallelizes:** fully, with M2/M3/M5 — different owner.

> Decisive call: Xcode 26 is folded into M4 (the iOS track) rather than deferred to the final submission milestone (risk-retire/critical-path double-booked it in M6/M7). Push entitlements require a real provisioning profile on the Xcode 26 toolchain, so the migration must precede push device testing. It lands once, here.

---

### M5 — Connector Instant-Token Fast Path `[Server / SDK]`

**Goal:** A developer points a connector at the hosted server and gets a working session in **<60s**, with a persistent identity across restarts.
**Gaps/risks closed:**
- §5b Connector instant-onboarding (M) — BotFather-style CLI; instant-token path (relaxes the §2 strict account+key-bind ceremony, `server.rs:1280`); Ed25519 key-gen hidden during a CLI/UI animation; `--private-key` flag for static identity; persistent keypair stored to disk/keychain.
- §5d connector defects (S each) — `buildMinimalContext` malformed `from` (`agent.ts:381`) fixed to a valid `user@domain`; `stream().end("")` no longer drops collapsed text (`streaming.ts:45`); `reconnect` implemented (≥3 retries, exponential backoff).

**Exit criteria (verifiable):**
- `npx @ainm/connector init --server wss://<hosted>` completes registration and sends a test message in **≤60s** on a clean machine (timed).
- Restart with the same `--private-key` rejoins the same session without re-registration.
- `from` is a well-formed AINM address; `stream().end("")` after a non-empty stream sends no blank collapsed message; `reconnect` retries with backoff before surfacing an error.
- Connector test suite green.

**Size:** M (instant path) + 3–4×S (defects) = **M.**
**Dependencies:** M1 (server must accept instant tokens); M2 (live endpoint to test the <60s path end-to-end).
**Track:** Server/SDK. **Parallelizes:** yes; **has genuine slack** — it is a developer-adoption feature, not an App Store gate, and can slip post-launch without delaying submission. Per locked decision 7 it is part of the launch *story*, so we hold it in v1 unless the schedule forces a cut.

---

### M6 — Minimal Signed Memory Export + Memory UI Polish `[Server / iOS — PASS BRANCH ONLY]`

**Goal:** Make "user-owned, portable" *demonstrably true at launch* without putting a multi-month research format on the critical path.
**Branch:** **PASS only.** On FAIL, skipped entirely (memory UI is "coming soon" per §3).
**Scope decision (the key sequencing call):** ship a **minimal, openly-specified, Ed25519-signed, content-addressed export bundle (AMO v0)** that a standalone verifier validates against the user's public key **without contacting the server**. The **full cross-runtime AMO wire format + foreign-runtime import** (BLAKE3 Merkle-DAG, the §5a "L" deliverable that no external runtime can yet consume) is **v2**. This honors the moat headline truthfully — the user can export a signed bundle anyone can verify — while refusing to gate App Store submission on a format with no day-one consumers.
**Gaps/risks closed:**
- §5a Portable signed memory artifact (the *verifiable-export* slice of the L) — `ainm.memory.export` gated on the `export` verb emits a signed, content-addressed bundle; AMO v0 schema published under `specs/` and versioned in AsyncAPI; a standalone verifier (Rust CLI/test) validates it offline.
- §5b Memory UI polish (part of the M) — memory views use the Editorial Warm SP1 design tokens (no raw semantic color literals); pull-to-refresh wired; participant roster sheet.

**Exit criteria (verifiable):**
- `ainm.memory.export` (with `export` grant) returns a content-addressed, Ed25519-signed bundle; the standalone verifier validates it offline against the sender's public key.
- AMO v0 schema published and versioned; clearly marked v0 (export+verify only; cross-runtime import is v2).
- Memory views render with SP1 tokens; pull-to-refresh works; roster sheet opens from the thread header listing all participants with avatars.

**Size:** M (AMO v0 export+verifier — *not* the full L) + M (UI) = **M–L.**
**Dependencies:** M0 PASS; M3 (the `export` verb and grant enforcement must exist).
**Track:** Server + iOS. **Parallelizes:** AMO v0 (server) and memory UI (iOS) run as two sub-tracks in parallel.

> Rejected (moat-led / risk-retire): the full BLAKE3 Merkle-DAG cross-runtime AMO with foreign-runtime import inside v1. Public-launch-scope-realism judge correctly flagged this as a multi-month research deliverable with zero day-one consumers that would sit on the critical path and kill the submission date. Rejected (critical-path): a "first-pass" so thin it isn't independently verifiable. AMO v0 is the honest middle: export + offline verification ships; import + the open interchange format are v2.

---

### M7 — iOS Consumer Polish + Account Deletion `[iOS]`

**Goal:** Consumer-app quality first-run, and clear the known App Store guideline 5.1.1 account-deletion rejection vector.
**Gaps/risks closed:**
- §5b Onboarding + memory UI polish (the onboarding M) — replace the `Form`-based onboarding with a designed SwiftUI flow against the Editorial Warm system; ≤4 taps from launch to first message.
- **Account deletion (App Store guideline 5.1.1)** — for an app with cryptographic identity this is non-trivial: in-app account deletion that performs key/Keychain teardown, server-side token invalidation, memory graph wipe, and APNs token deregistration.
- **UGC safety controls (App Store guideline 1.2)** — *added 2026-06-09; same hidden-rejection-vector class as account deletion.* A messenger with human participants is a user-generated-content app: review requires a method to **report objectionable content**, **block abusive users**, and published moderation/contact info. ✅ **Server side DONE 2026-06-10** (migration 009, bearer-authed block/report REST, delivery-time enforcement via `send_to_thread_except`). Remaining: iOS block/report UX (S sheet + settings list) + moderation contact in M8 legal copy.
- §5c OPRF attempt-count reset path (S) — exhausted users recover via the account-reset flow without contacting support.
- §5d `ainm.thread.join`/`leave` dispatch (S) — server dispatches them; round-trips to the iOS client.
- §2 `eraseDatabaseOnSchemaChange` release-build audit — confirmed `false` or `#if DEBUG`-gated and **absent** from the release build.

**Exit criteria (verifiable):**
- Onboarding completes in ≤4 taps on a fresh install; passes design review against the Editorial Warm system.
- In-app "Delete account" fully tears down: keys/Keychain cleared, server tokens invalidated, memory wiped, APNs token deregistered — verified end-to-end.
- An OPRF-exhausted user recovers via account reset with no support contact.
- `ainm.thread.join`/`leave` dispatched server-side and round-trip to iOS.
- `AppDatabase.swift:77` `eraseDatabaseOnSchemaChange` not active in the Release config.
- Report-content and block-user flows work end-to-end (server enforcement: blocked sender's messages are not delivered; report lands somewhere a human reads). *(guideline 1.2, added 2026-06-09)*

**Size:** M (onboarding) + S–M (account deletion) + S (OPRF) + S (join/leave) + S (audit) = **M.**
**Dependencies:** M4 (Xcode 26 is the build target before shipping UI for the store; push must work before a meaningful TestFlight); on PASS, M6 (memory UI direction depends on whether grant/export UX is included).
**Track:** iOS. **Parallelizes:** design work can start early; partially overlaps M6.

> Grafted (public-launch-scope-realism judge): account deletion was a hidden rejection vector listed only as an M8 checkbox in critical-path with zero implementation coverage. Scoped and placed here in M7 where the work actually happens.

---

### M8 — Legal + App Store Submission `[iOS / Legal / Marketing]`

**Goal:** Clear every hard external gate and get into review. This is the launch gate.
**Gaps/risks closed (non-code external gates):** Privacy nutrition label; ToS + Privacy Policy live URLs; App Store metadata (screenshots, description, keywords, age rating, support URL); push entitlement provisioning (no sandbox/production mismatch); landing page with the branch-determined headline.
**Exit criteria (verifiable):** see §6 — every line in the App Store gate checklist is true; app submitted and in "Waiting for Review."
**Size:** M (legal + metadata) + S (store assets) = **M.**
**Dependencies:** M7 (polished app), M2 (real TLS server), M4 (push). On PASS, M6 ships the moat headline; on FAIL, the messenger headline.
**Track:** iOS/Legal/Marketing. **Parallelizes:** legal copy and landing-page drafting can start day one, in parallel with all engineering.

---

## 5. v1 launch cut-line vs v2 deferred

### v1 (must ship before App Store submission)

| Milestone | Track |
|---|---|
| M0 Task-14 go/no-go gate | Moat |
| M1 Security floor + server CI + `ring` + ungated-query gate + bubblewrap hard-error | Server/Infra |
| M2 Fly.io hosted deploy + real TLS + observability + rate limiting + IdentityStore→DB (single-node reliability) + tungstenite 0.29 | Infra/Server |
| M3 Agent Grant core + grant UX **(PASS only)** | Server/iOS |
| M4 iOS push + background delivery + Xcode 26 + xcframework rebuild + ping/pong | iOS |
| M5 Connector instant-token fast path + connector defects **(slack; held in v1)** | Server/SDK |
| M6 AMO v0 signed export + offline verifier + memory UI polish **(PASS only)** | Server/iOS |
| M7 Onboarding polish + account deletion + OPRF reset + join/leave dispatch + erase-DB audit | iOS |
| M8 Legal (ToS/Privacy/nutrition label) + App Store submission + landing page | iOS/Legal |

### v2 (explicitly deferred)

- **All federation-gated security debt (locked decision 6):**
  - Unsigned cross-server attestation hardening (§5c) — server keypair in `ServerState`, signed `.well-known`.
  - IdentityStore **federation re-verification** semantics (§5c) — single-node persistence ships in M2; the cross-server re-verify path is v2.
  - mTLS CA automation + FAUT-02 warn-not-drop → drop (§5c).
  - Key rotation + `key_history` endpoint (§5c). *(Rejected from v1: moat-led's "launch-required for the cryptographic-identity claim" justification — a single-home-server has no cross-server consumer of `key_history`, so rotation buys nothing at launch and contradicts locked decision 6.)*
- **HA / multi-replica** (§7 risk #5) — externalize in-memory WS registries to Redis/Postgres; architectural, accepted single-node beta risk.
- **Full AMO cross-runtime portability** — BLAKE3 Merkle-DAG wire format + foreign-runtime import + UCAN-token emission (§5a L remainder). AMO v0 export+verify ships in v1; the open interchange format is v2.
- **Live cross-server federation (P3)** — single-home-server v1.
- **Web / Android client** — iOS only (locked decision 3).
- **Billing / IAP** — free BYO-key beta (locked decision 5).
- **Dependency upgrades with no security pressure** — `redb` 3→4 (two-step data migration), `sqlx` 0.8→0.9 (needs Rust 1.94).
- **Memory depth** — contradiction detection (stub never fires) + attribution heuristic (§5d); L2 sleep-time consolidation; L3 skill-accretion loop.
- **Other §5d nice-to-haves** — backfill on federation join; `get_edges_for_nodes` O(n) fallback + `query_nodes` category filter hardening; "top of history" sentinel; exyte/Chat evaluation.

---

## 6. App Store gate checklist

All must be **true** to submit (hard external gates):

1. **Xcode 26 + iOS 26 SDK build** uploads without errors (Apple mandate since 2026-04-28). — M4
2. **`AinmEscrow.xcframework` rebuilt** with the Xcode 26 toolchain; live-device escrow + biometric regression passes. — M4
3. **Push notifications work end-to-end** on ≥2 physical devices; push entitlement provisioned with no sandbox/production mismatch (a messenger without background delivery fails review on completeness). — M4 / M8
4. **Privacy nutrition label** complete and accurate: account identifier, messages, device ID for push, Ed25519 identity, and — on PASS — memory data; declared correctly as linked/not-tracking (guideline 5.1.1 rejection risk if wrong). — M8
5. **ToS + Privacy Policy** live at a permanent URL, linked from app metadata. — M8
6. **In-app account deletion** works (key teardown, token invalidation, memory wipe, APNs deregistration) — guideline 5.1.1. — M7
7. **`eraseDatabaseOnSchemaChange`** confirmed `false`/`#if DEBUG`-gated and absent from the Release build (data-safety rejection vector). — M7
8. **No hardcoded secret** in any shipped/served path; production server refuses to start without injected secrets. — M1
9. **Real TLS** on the hosted server (not the repo's demo self-signed cert). — M2
10. **TestFlight green** on ≥2 physical devices including the push flow; no guideline 2.1 crashes, no private-API usage. — M7 / M8
11. **UGC safety controls** (guideline 1.2): in-app report-content + block-user flows work end-to-end; moderation/contact method published in metadata. — M7 / M8 *(added 2026-06-09)*

---

## 7. Immediate next 3 actions

1. **Run Task 14 (M0) now.** Stand up a local throwaway instance (`docker-compose` + real Postgres + live LLM keys); seed **200–500+ real cross-vendor facts**; run arm C vs B vs A with a production-grade judge model distinct from the briefing model; log the numeric result. Apply the pass/fail in §3 and **document the founder go/no-go decision** — this determines whether M3/M6 are on the path. Begin operator/data prep immediately; there is no dependency to wait on.
2. ~~**In parallel, start M1's quick wins**~~ — ✅ **DONE 2026-06-09** (with claim corrections + two real pre-existing bugs found and fixed; see [`2026-06-09-m1-security-floor-ground-truth.md`](./2026-06-09-m1-security-floor-ground-truth.md)). Remaining M1 tail: push to origin so the CI workflow gets its first green run.
3. **Kick off the iOS track (M4) in parallel** — start the Xcode 26 migration and `AinmEscrow.xcframework` rebuild (compiler-only, no server dependency), so push work is unblocked the moment M1's secret gate lands. **Practical prerequisite: free disk space first** — the machine sat at ~93% used (2026-06-09) and an Xcode 26 install + iOS 26 SDK/simulators needs tens of GB. Concurrently, **non-engineering**: begin drafting ToS, Privacy Policy, and the landing page (headline placeholder pending the M0 verdict).

**Per-phase specs/plans to write next** (this roadmap is the parent; each phase gets its own via `superpowers:brainstorming` → `superpowers:writing-plans`):
- **M1** — `2026-06-XX-foundation-security-ci.md` (spec + plan).
- **M2** — `2026-06-XX-hosted-deploy-observability.md` (spec + plan), incl. the IdentityStore→DB single-node slice.
- **M3** — execute the existing `docs/superpowers/plans/2026-06-09-agent-grant-server.md` (PASS only) + a new iOS grant-UX spec.
- **M4** — `2026-06-XX-ios-push-xcode26.md` (spec + plan).
- **M5** — `2026-06-XX-connector-instant-onboarding.md` (spec + plan).
- **M6** — `2026-06-XX-amo-v0-signed-export.md` (spec + plan) — explicitly scoped to export+verify, not cross-runtime import (PASS only).
- **M7** — `2026-06-XX-ios-polish-account-deletion.md` (spec + plan).
- **M8** — `2026-06-XX-launch-legal-appstore.md` (checklist-driven; mostly non-code).
