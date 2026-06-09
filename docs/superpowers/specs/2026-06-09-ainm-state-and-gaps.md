# AINM — State of the Project + Gap Analysis

**Date:** 2026-06-09
**Author:** Lead architect synthesis (14 parallel research/mapping agents: 7 codebase, 3 framework-currency, 4 OSS/research)
**Purpose:** Durable internal decision-making document. Honest, non-promotional. This is the input to a ship roadmap — it does **not** contain the roadmap. The phased plan comes after the founder answers the open product-scope decisions in §6.

---

## 1. Vision

AINM is an open, federated, AI-native messaging protocol and a reference iOS client where humans and agents are equal participants in conversations. The **shippable product** is a hosted messenger app: a user signs up, gets a cryptographic identity, and talks to AI agents (and other humans) in threads — with streaming responses, tool-call visibility, and agent-to-agent coordination. The **moat thesis** is the one thing no incumbent ships today: **portable, user-owned, consent-gated compound memory.** AINM passively compiles a knowledge graph from the user's conversations, the user's own Ed25519 key is the root of trust for that memory, and the user grants scoped, revocable, cryptographically-enforced access to specific agents — with the memory portable across any agent runtime via an open, signed wire format. Every memory vendor (Mem0, Letta, Zep, MemOS) competes on retrieval quality inside a proprietary silo; none ships signed, content-addressed, user-controlled, cross-runtime memory. The hosted runtime is the consent-enforcement point; portable memory is the durable wedge. **Honest caveat:** the moat is currently *designed and partially prototyped, not proven* — the value experiment that would justify the whole thesis (Task 14) has not been run on real data.

---

## 2. Current-State Map

Maturity ratings: **Production** (hardened, tested incl. negative cases) / **Beta** (functional, real but not battle-tested) / **Prototype** (works, not production-hardened) / **Designed-not-built** (spec/plan exists, ~0 code) / **Unproven** (code exists but no test/run exercises the real path).

| Subsystem | Maturity | Most important facts (with file:line evidence) |
|---|---|---|
| **Server (routing/delivery/federation)** | **Beta** | Single `deliver_message` orchestrator does routing, hop-cap, consent gating, and dispatch for both WS and relay handlers (`server.rs:1572`). P1 same-server A2A shipped; P2 consent gate (`thread_invited` + `acceptsA2A`, `routing.rs:112`, `server.rs:1655`) and P3a/P3b cross-server relay are implemented with 2 in-process integration tests. **Critical gaps:** `IdentityStore` is in-memory and lost on restart (`identity.rs:57`); `start()`/`start_with_port()` embed a hardcoded `b"dev-insecure-secret"` TokenSigner (`server.rs:891`); cross-server identity attestation is unsigned — server keypair is not in `ServerState` (`server.rs:860`, `// TODO(B): sign attestation`). |
| **Compound Memory (the moat)** | **Prototype + Designed-not-built** | Capture→compile→graph→briefing pipeline is wired and the schema is live (`002_compound_memory.sql`); `retrieval.rs` is pure with 12 passing unit tests; `briefing.rs` consent scoping is test-enforced (`scope_map_exactly_covers_the_canonical_set`, `briefing.rs:432`). **But:** the Agent Grant layer (verbs, tool grants, write-back, user signature) is **0 tasks executed** and **gated on Task 14** (plan: `docs/superpowers/plans/2026-06-09-agent-grant-server.md`; migration `008` does not exist — confirmed: `server/migrations/` stops at `007`). `ainm.memory.graph.query` has **no consent check** (`server.rs:1731`); contradiction detection is a stub (`compiler.rs:278`); user-identity attribution is a fragile heuristic (`server.rs:1461`). Portability (signed, content-addressed, cross-runtime export) is **not built**. |
| **iOS app (reference client)** | **Beta** | MVVM + `@Observable`, GRDB (9 migrations, per-account isolation, WAL), Ed25519 via CryptoKit + OPRF escrow via UniFFI FFI. Full onboarding/recovery, multi-account, streaming, tool-call UI, A2A display arc (SP1–SP4), memory UI all implemented. IOS-11 verified the live app↔runtime loop on 2026-06-04. **Blocking gap:** no push notifications / background delivery (no `UNUserNotificationCenter`, no APNs) — app goes dark when backgrounded. `eraseDatabaseOnSchemaChange = true` under `#if DEBUG` needs a release-build audit (`AppDatabase.swift:77`). Onboarding UX is unpolished `Form`-based; live A2A visual rendering unverified post-P1. |
| **Connector SDK (`@ainm/connector`)** | **Prototype** | `AINMAgent` class, Ed25519 signing, JSON-RPC loop, streaming, memory context all functional; `ainm.message.send` flat-params shape recently fixed and E2E-tested. **Onboarding friction is the headline:** the server hard-rejects any identity not pre-registered (account + key-bind ceremony required; `server.rs:1280`) — **no instant-token path** like Telegram BotFather. Reference connectors generate **ephemeral keypairs** per run and expose **no `--private-key` CLI flag** (unverified but not found), so a restart breaks the binding. `buildMinimalContext.sendMessage` uses `keypair.publicKey.toString()` as `from` (malformed identity, `agent.ts:381`); `reconnect` config is declared but **not implemented**; `stream().end("")` discards collapsed text (`streaming.ts:45`). |
| **Protocol / Security** | **Production (crypto) / Beta (protocol surface)** | Four Ed25519 signing operations (envelope, init, auth-challenge with `ainm-auth-token|` domain separation, relay) all unit-tested incl. negative cases (`signing.rs`). OPRF escrow (RFC 9497, Argon2id, XChaCha20Poly1305) is TOCTOU-safe and shared verbatim between server and iOS via UniFFI. Auth sessions: hash-only storage, sliding 90-day/365-day-ceiling TTL, revocable. AsyncAPI 3.1.0 + 13 JSON Schemas published. **Gaps:** `ainm.thread.join`/`leave` declared in AsyncAPI but **not dispatched** in server; `ainm.ping`/`pong` keepalive spec-only (unconfirmed handler) — risk for iOS background networking; **key rotation absent** (`key_history` always `[]`, `identity.rs:53`); mTLS federation needs manual pre-shared CA (no automation). |
| **Infra / Deploy** | **Beta (single-node) / Unproven (HA)** | Two-stage non-root Dockerfile, `docker-compose.prod.yml`, SIGTERM graceful drain (`lifecycle.rs`, tested), prod secrets gate (`prod_guard.rs`, tested), DB-aware health endpoints — all production-grade for **single node**. **Blocking gaps:** **no server CI** (no `cargo test`/`clippy` on push — only iOS workflows exist); in-memory WS registries (`server.rs:55,151`) **block multi-replica HA** (split-brain); `AINM_LLM__API_KEY` not gated by `prod_guard` (silent fall to mock LLM, `main.rs:490`); migration path resolves `migrations/` from CWD (silent skip if not `/app`, `db.rs:19`); **no observability backend** (no OTel/Prometheus/metrics); **no rate limiting** on WS/REST; TLS certs are demo self-signed in repo. |
| **Test / Quality** | **Prototype** | Strong unit coverage: signing, JCS (RFC 8785 vectors), escrow round-trip, auth/OTP, GRDB migrations, connector handshake — all production-grade. **But the black-box behavioral proof never runs in CI:** all DB-gated WS/A2A E2E tests (`ws_conversation_e2e`, `a2a_routing_e2e`, `a2a_consent_e2e`, `a2a_spoof_e2e`) **skip silently** without Postgres, and there is **no Rust CI**. Two integration tests are pre-existing failures on a live DB (`test_graph_traversal` `integration.rs:141`, `test_compilation_dedup_updates_confidence` `integration.rs:510`) — not `#[ignore]`-annotated, so they pass vacuously today. Federation mTLS is **runbook-only** (no automated harness). Task 14, AD-12, CAC-7 still manual/pending. |

---

## 3. Framework Currency

Versions verified against crates.io / GitHub on 2026-06-09. `server/Cargo.toml` confirmed: `tokio-tungstenite = "0.26"`, `ring = "0.17"`, `redb = "3.1"`, `sqlx = "0.8"`.

### Server (Rust)

| Dep | Pinned | Latest | Action | Security-relevant? |
|---|---|---|---|---|
| `ring` | 0.17 | 0.17.14 | **Verify resolved ≥0.17.8** (`cargo tree -i ring`); add minimum if older | **YES** — DoS panic advisory in AES (`ring` < 0.17.8); server handles adversarial network input. ~5 min fix. |
| `tokio-tungstenite` | 0.26 | 0.29.0 | **Upgrade before release branch cuts** | No, but breaking: `Message` payload changed `Vec<u8>`→`Bytes`, `Text`→`Utf8Bytes`. Compile-time-safe; do it now to avoid a launch scramble. |
| `redb` | 3.1 | 4.1.0 | **Defer + document** | No. Major; requires a two-step data migration (load via `Legacy`, copy, upgrade). Pin at 3.x for launch, add a migration task. |
| `sqlx` | 0.8 | 0.9.0 | **Defer (post-ship)** | No. 0.9 needs Rust 1.94; breaking changes minor. No security pressure. |
| `axum` 0.8 / `tokio` 1 / `rustls` 0.23 / `reqwest` 0.13 / `tower` 0.5 / `uuid` 1 / `chrono` 0.4 / `rmcp` 1.7 | — | latest | semver-resolves; no action | — |

> Note: `jsonschema` and `jsonrpsee` appear in the tech-stack doc but are **not** in `server/Cargo.toml` — not shipped yet.

### iOS

| Item | Current | Latest | Action | Blocking? |
|---|---|---|---|---|
| **Xcode** | 16.x (objectVersion 77) | 26.6 RC1 | **Migrate to Xcode 26** | **YES — blocks App Store submission.** Apple mandated Xcode 26 + iOS 26 SDK for all uploads since 2026-04-28. Deployment target stays iOS 18. |
| `AinmEscrow.xcframework` | UniFFI 0.31 | 0.31.1 | **Rebuild with Xcode 26 toolchain** | Tied to the above — Mach-O slice compatibility; requires live-device escrow/biometric regression. |
| Swift mode | 6.0 | 6.2 (Xcode 26) | **Audit `nonisolated async` sites** (SE-0461 changes isolation inheritance); do NOT enable `SWIFT_APPROACHABLE_CONCURRENCY` / `-default-isolation MainActor` without team decision | Behavioral, not blocking |
| GRDB.swift | 7.10.0 | 7.11.0 | Bump (one minor, no API breaks) | No |
| KeychainAccess | 4.2.2 | 4.2.2 | None | No |
| Apple CryptoKit | system | system | None (stable iOS 18→26) | No |

### Standards

JSON Schema **2020-12** and AsyncAPI **3.1.0** are both current — AINM's spec files are correct; no migration needed. (AINM's `asyncapi.yaml` header version is `0.1.0` — that's the document version, not the AsyncAPI spec version, which is correctly `3.1.0`.)

**Security flag summary:** the only security-relevant currency item is **`ring` ≥ 0.17.8** — verify immediately. The hardcoded `dev-insecure-secret` TokenSigner (§2 Server) and unsigned attestation are bigger security issues than any dependency lag, but those are codebase gaps, not currency.

---

## 4. OSS & Skills to Adopt

Highest-leverage, with a one-line why and a real URL.

**Memory portability (moat foundation):**
- **PAM / "Portable Agent Memory" design** — BLAKE3 content-addressing + Ed25519 Merkle-DAG signing; the only sound technical treatment, Apache-licensed; reimplement natively in Rust/Swift, don't adopt the Python SDK. https://arxiv.org/abs/2605.11032
- **UCAN 1.0.0-rc.1 (semantics, not wire format)** — delegable, attenuating, Ed25519-signed capability grants with slash-namespaced abilities (`/ainm/memory/read|derive|redact|export`); adopt the grammar serialized as JCS-canonical JSON (not DAG-CBOR). https://github.com/ucan-wg/spec
- **A2A v1.0 Signed Agent Cards (Linux Foundation)** — sign the AgentCard with AINM's existing Ed25519 keys; additive, aligns AINM with a 150-org stable standard. https://a2a-protocol.org/latest/specification/
- **MIF / memorywire (watch, don't adopt)** — the only active interchange attempts; useful as prior art and as import/export targets for AINM's format. https://github.com/agentmemoryprotocol/agentmemoryprotocol

**Memory quality / Task 14:**
- **LongMemEval-V2 + BEAM (via mem0ai/memory-benchmarks)** — rigorous numeric memory-quality scoring; V2's agentic variant maps to AINM's hosted runtime; Mem0's 94.4% is the competitive bar. https://github.com/mem0ai/memory-benchmarks

**Onboarding UX:**
- **A2A AgentCard + `.well-known` + QR/deep-link pattern** — match Telegram BotFather's <60s feel while keeping real Ed25519 (key-gen happens silently during UI animation). https://agent2agent.info/docs/concepts/agentcard/
- **Slack optional-scopes pattern (Mar 2026)** — surface scope prompts only for agents requesting elevated permissions; keep the common path one tap. https://docs.slack.dev/changelog/2026/03/16/optional-scopes/
- **exyte/Chat v2.1.4 (SwiftUI)** — closure-based message cells decoupled from backend; integrates with AINM streaming; saves 3–5 weeks (verify iOS 18 scroll anchoring first). https://github.com/exyte/chat

**Build accelerators:**
- **tower-governor v0.8 / axum-governor** — GCRA rate limiting as a Tower layer with per-tenant keys; drops in above `MeteredLlmClient` to close the missing-rate-limit gap. https://github.com/benwis/tower-governor
- **axum-tracing-opentelemetry v0.33 + Langfuse (self-host)** — per-turn LLM traces via pure OTLP, zero vendor lock-in; closes the observability gap. https://github.com/playbookengineering/axum-tracing-opentelemetry
- **k6 WebSocket testing** — 100-VU sustained WS handshake+round-trip test in CI; bridges `#[tokio::test]` and real load. https://grafana.com/docs/k6/latest/using-k6/protocols/websockets/

**Deployment:**
- **Fly.io + Fly Managed Postgres** — Firecracker microVMs make bubblewrap work structurally (Render confirmed blocked; Railway likely blocked); ~$50/mo single-region beta. https://fly.io/docs/mpg/
- **Hetzner + Docker Compose + AppArmor** — the eventual self-host reference deployment (also the open-standard flywheel); ~$40–55/mo for 3 nodes. https://www.hetzner.com/

**Standards alignment (watch):**
- **MCP 2025-11-25 + SEP-1932/1933** — workload-identity patterns for how AINM's hosted runtime auths to third-party MCP servers; doesn't block memory work. https://modelcontextprotocol.io/specification/2025-11-25

---

## 5. Gap Analysis (Vision − Current State = What's Missing to Ship)

### (a) Moat-critical — portable memory + proof

| Gap | What | Why it blocks shipping | Size |
|---|---|---|---|
| **Task 14: real-data value proof** | Run the briefing A/B (arm C vs B vs A) on real cross-vendor data with live LLM keys at real profile scale (200–500+ facts). Currently `[pending]`; experiment to date used a 20-fact synthesized profile + synthetic noise. | The entire moat thesis is **unproven**. If C does not beat B on real data, the whole compound-memory positioning needs rethinking. It is the explicit gate on the agent-grant build. | **M** (needs human operator + real keys + real data) |
| **Agent Grant layer** | Migration 008, `AgentGrant` (verbs read/derive/redact/export, tool allow/deny, write-back, user signature), CRUD endpoints, verb enforcement. 0 of 9 plan tasks executed; migration 008 confirmed absent. | Without it there is no cryptographically-enforced, user-controlled consent — the moat is just a passive graph. Memory cannot be "user-owned and consent-gated" as claimed. | **L** |
| **`ainm.memory.graph.query` is ungated** | Any connected agent can query any user's memory graph today (`server.rs:1731`). | Directly contradicts the consent-gated claim; a trust/safety hole at the heart of the moat. | **S** (gate exists in `tools_memory.rs`; apply the same check) |
| **Portable signed memory artifact (AMO)** | A canonical, versioned, openly-specified wire format with content-addressing + Ed25519 signing that other runtimes can produce/consume. Today only `ainm.memory.export` exists, not a portable signed artifact. | "Portable across any runtime" is the unique differentiator and it does not exist in code. This is the genuine white space no incumbent fills. | **L** |

### (b) Product-critical — onboarding ease, deploy, polish

| Gap | What | Why it blocks shipping | Size |
|---|---|---|---|
| **iOS push notifications / background delivery** | No APNs, no `UNUserNotificationCenter`, no background refresh. App goes dark when backgrounded. | A messenger that cannot notify or receive while backgrounded is not shippable as a messaging product. | **L** |
| **Connector instant-onboarding** | No instant-token path; full account + key-bind ceremony required; ephemeral keypairs on restart; no `--private-key` CLI flag. | "Universal agent client, no lock-in" requires a developer to point a connector at a server and get a session fast. Today they cannot. | **M** |
| **Hosted deployment pipeline** | No server CI; in-memory registries block HA; LLM key not gated; migration CWD fragility; demo certs in repo. | Cannot operate a hosted multi-tenant beta reliably; regressions ship silently; single-node only. | **M** (CI + LLM-key gate + embedded migrations are small; HA is L and can be v2) |
| **Observability + rate limiting** | No metrics/trace export; no per-account/IP rate limit on WS/REST. | Operating a hosted service blind to p99/error rates, with no abuse protection, is not viable. | **M** |
| **Xcode 26 migration + xcframework rebuild** | Build with Xcode 26 SDK; rebuild escrow xcframework. | Hard App Store submission requirement since 2026-04-28. | **S–M** (mechanical, but needs live-device regression) |
| **Onboarding + memory UI polish** | `Form`-based onboarding, raw semantic colors in memory views, no-op pull-to-refresh, no participant roster sheet. | Not consumer-app quality; first-run impression and the moat's UI surface both look unfinished. | **M** |

### (c) Trust / safety

| Gap | What | Why it blocks shipping | Size |
|---|---|---|---|
| **Hardcoded TokenSigner secret** | `start()`/`start_with_port()` use `b"dev-insecure-secret"` (`server.rs:891,971`). | Any token minted in those paths is forgeable with a known key. Must never reach a hosted deployment. | **S** |
| **Unsigned cross-server attestation** | Server keypair not in `ServerState`; `.well-known` identity attestation unsigned (`server.rs:860`). | Federation identity is asserted, not proven — undermines "cryptographic identity non-negotiable." | **M** |
| **In-memory `IdentityStore`** | Identities lost on restart (`identity.rs:57`). | After restart, remote servers can't re-verify identities without a re-handshake — federation breaks. | **M** |
| **mTLS FAUT-02 warn-not-drop + manual CA** | Unrecognized peers logged, not dropped (`main.rs:769`); no CA provisioning automation. | Federation security relies on best-effort lazy resolution; multi-operator federation is operationally blocked. | **M** |
| **Bubblewrap silent degradation** | MCP tools needing namespace isolation silently fall to `SandboxMode::None` on non-Linux/no-bwrap (`mcp.rs:244`). | An untrusted MCP process running unsandboxed is a security regression; should hard-error. | **S** |
| **Key rotation absent** | `key_history` always `[]`; no rotation endpoint. | A federated protocol needs rotation+history so verifiers can validate prior-key signatures. | **M** |

### (d) Nice-to-have

| Gap | What | Size |
|---|---|---|
| `thread.join`/`thread.leave` dispatch (declared in AsyncAPI, not dispatched) | **S** |
| `ainm.ping`/`pong` keepalive confirmation (matters for iOS background, borderline product-critical) | **S** |
| Backfill on federation join (data structures exist, no delivery code) | **M** |
| Connector defects: `buildMinimalContext` malformed `from`, blank `userIdentity`, `stream().end("")` empty collapsed text, `reconnect` unimplemented | **S each** |
| Contradiction detection (stub never fires) + user-identity attribution heuristic | **M** |
| `get_edges_for_nodes` O(n) fallback + `query_nodes` string-interpolated category filter (SQLi-shaped, mitigated upstream) | **S–M** |
| OPRF attempt-count reset path (exhausted users have no API recovery) | **S** |
| Participant roster sheet; "top of history" sentinel; redb 3→4 and sqlx 0.8→0.9 upgrades | **S–M** |

---

## 6. Open Product-Scope Decisions

These are the questions the ship roadmap depends on that **only the founder can answer.** Listed crisply, deliberately unanswered.

- **Ship definition:** What is the minimum shippable v1? Hosted iOS app + assistant only? Or does v1 require portable memory to be live (i.e., is the moat in v1 or v2)?
- **Hosted vs OSS sequencing:** Do we launch the hosted beta first (Fly.io) and document self-host (Hetzner) in parallel, or hold the hosted launch until the open spec + self-host story are equally ready?
- **Platforms:** iOS-only at launch, or is a web/desktop client (or a published connector-author experience) part of v1?
- **Task 14 gate:** Is the real-data memory value proof a hard go/no-go before any agent-grant / portability build — and who runs it, with which real data and which models? Does a negative result pause the moat or trigger a redesign?
- **Monetization:** Is v1 free beta, or does the hosted runtime carry pricing/quota from day one (the `QuotaConfig` ledger exists but isn't enforced as rate limit)? Does monetization sit on the runtime, on memory portability, or both?
- **Timeline:** What is the target ship window, and which gaps are explicitly deferred to v2 (HA, key rotation, federation-of-memory, web client)?
- **Federation scope at launch:** Is cross-server federation (P3) part of the v1 promise, or is single-home-server the v1 deployment with federation as a documented-but-unlaunched capability?
- **Connector onboarding model:** Do we add an instant-token / passkey-style fast path (relaxing the key-bind ceremony) for v1 developer adoption, or hold the strict cryptographic gate and accept slower connector onboarding?

---

## 7. Skeptical Notes

**Could not be verified by the agents:**
- **Task 14 results do not exist.** All memory A/B numbers (C scoring 5.0/5.0 etc.) are from a **20-fact synthesized profile** + synthetic noise, not real cross-vendor data. The verdict note itself flags that the real test ("seed 200–500 facts and re-run") has not been done. Treat the moat as *promising-but-unproven*.
- **Model IDs** referenced in the value-spike (`gpt-5.5`, `claude-opus-4-8`, `gemini-3.1-pro` via Vercel AI Gateway) are plausible for the date but were not independently verified.
- **`AinmEscrow.xcframework`** is a build artifact not in the repo tree; IOS-3 is marked complete but the framework's presence/integration cannot be confirmed from source.
- **Reference connector `--private-key` flag:** not found in `index.ts` entry points, but the agent flagged this as possibly-incomplete inspection. Confirm before asserting static-key operation is impossible.
- **Federation mTLS** is runbook-only (`docs/runbooks/2026-06-09-federation-a2a-verify.md`) — no automated test ever exercises real two-process TLS. FCP-5 is "completed" by human runbook execution only.
- **Two integration tests pass vacuously** (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`) — they `return` early without a DB and **fail on a real DB**. The suite is not actually green under real conditions.

**Likely-fabricated or low-confidence external claims (per the research agents themselves):**
- The arXiv PAM paper's "100% tamper detection" is against a controlled synthetic benchmark, single-author preprint, N=50 — design sound, evaluation preliminary.
- A2A v1.0 Signed Agent Cards: the curve (Ed25519) is plausible but **not confirmed** from the spec excerpt — it lists signing as a capability without specifying the algorithm.
- UCAN final 1.0.0 has **not** shipped (only `1.0.0-rc.1`).
- MCP `2026-07-28` release candidate date came from search metadata, not the verified document — provisional.
- "OpenAI apps converted to Codex plugins" and the Railway bubblewrap verdict are both explicitly low-confidence / inferred-not-confirmed.

**Biggest risks (ranked):**
1. **The moat is unproven.** Everything strategic (agent grants, portability, hosted-runtime-as-consent-enforcer) is gated on a Task-14 result that does not exist. Building the agent-grant layer before Task 14 would be building on sand.
2. **No server CI + non-green integration suite.** The behavioral proof of the server never runs automatically, and two tests fail on a real DB. Confidence in "it works" is currently based on local runs and manual runbooks.
3. **A messenger that can't notify.** No push/background delivery makes the iOS app non-shippable as a messaging product regardless of moat progress.
4. **Security debt in hosted paths.** Hardcoded TokenSigner secret, unsigned attestation, in-memory identity store, warn-not-drop mTLS — each is individually fixable but collectively undermines the "crypto-first, federation-ready" claim if shipped as-is.
5. **HA is architecturally blocked.** In-memory registries mean the hosted product is single-node until connection state is externalized — acceptable for beta, a wall for scale.
