# Phase 15: v3.2 Tech Debt + Off-GSD Ratification — Research

**Researched:** 2026-04-18
**Domain:** iOS Keychain hardening, cross-platform Ed25519/JCS conformance, TypeScript SDK ratification, server registry test coverage, XCUITest timing-race remediation
**Confidence:** HIGH (everything in this phase is in-tree code we can inspect; one MEDIUM area is OpenAI Responses cassette coverage which is a Phase 18 concern)

## Phase Intent (Success = TRUE when…)

This is **closure work, not new feature work**. Two parallel programs run inside one phase:

**Program A — v3.2 iOS Tech Debt (DEBT-01..13):** 11 items inherited from `v3.2-MILESTONE-AUDIT.md` are closed. Most are already-shipped iOS code that needs (a) a HUMAN-UAT pass against a live Rust server, (b) a one-line accessibility-class fix, (c) two XCUITest stabilizations, or (d) a Makefile/CI cleanup. Cross-platform Ed25519+JCS round-trip (DEBT-02) is the only item that requires genuine technical research, and the test vectors needed already exist at `tests/vectors/`.

**Program B — Off-GSD Ratification (RAT-01..06):** Four packages built outside GSD (`@ainm/connector`, `@ainm/connector-claude-code`, `@ainm/connector-openai`, server `AgentRegistry`) are pulled into milestone artifacts via an 8-point checklist. The biggest gaps found: `@ainm/connector-openai` has **zero tests** and no test script in package.json; `AgentRegistry` has **zero unit tests** in `demo/tests/`; no package has a `DECISIONS.md`; all packages use floating `^` SemVer ranges.

Phase is done when:
1. iOS handshake against live `ainm-demo --uat` Rust server signs envelopes that Rust verifies, and Rust-signed envelopes that iOS verifies (DEBT-01, DEBT-02, DEBT-03 — verified by recorded HUMAN-UAT run with both keys/timestamps captured).
2. `KeyManager.KeychainStorage.save` uses `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` with `kSecAttrSynchronizable: false`, and a regression test asserts retrieval after an Application Support reset (DEBT-04).
3. `make uat` Makefile derives simulator name from `xcrun simctl list -j devices available`, falling back to a documented constant only if discovery fails; same for `.github/workflows/ios-ci.yml`. (DEBT-05).
4. `DeepLinkUITests.test_deepLink_missingMessage_showsNotFoundToast` is un-`XCTSkip`-ed, root cause documented in a per-test comment, and `SubThreadBranchUITests` reply-badge assertion is hard not soft (DEBT-10, DEBT-11).
5. `make uat` runs to green twice in a row (anti-flake) after every DEBT cluster lands (DEBT-13).
6. `packages/*/dist/` is in `.gitignore`; CI step `git ls-files | grep -E 'packages/.*/dist/' && exit 1 || exit 0` runs in `ios-ci.yml` or a new `connector-ci.yml` (RAT-05).
7. Each of the 4 packages ships a `DECISIONS.md`, exact-pinned dependencies (no `^` ranges), green test runs against a live `ainm-demo` server in CI, structured logging that does not leak `process.env.OPENAI_API_KEY` or `briefing` content beyond a sanitized summary, and an explicit CHANGELOG entry tagged `0.1.1-ratified` (RAT-01..04, RAT-06).

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DEBT-01 | iOS completes live WebSocket handshake against running Rust server | Code is already wired — `AINMClient.connect` (lines 109-183) sends `ainm.initialize`, awaits response, sends `ainm.initialized`. Rust side `ws_upgrade_handler` in `demo/src/server.rs` accepts the handshake. Gap: no recorded run on a real (non-UAT-mode) server. Plan should include a `make handshake-uat` target that boots `ainm-demo` (NOT `--uat` mode), launches iOS, and captures a `tcpdump`/Wireshark trace of the three-way handshake plus a successful signed message. |
| DEBT-02 | Cross-platform Ed25519 + RFC 8785 JCS round-trip (Swift↔Rust) | `tests/vectors/jcs_vectors.json` (4 vectors) and `tests/vectors/signing_vectors.json` (3+ vectors with deterministic seed `9d61b19d…`) already exist. Swift side: `MessageSignerTests.swift` loads them via `repoRoot()` traversal. Rust side: `serde_json_canonicalizer` is the canonicalize crate. **Gap:** TypeScript SDK never loads these vectors — `packages/connector/tests/signing.test.ts` only does internal round-trip. Plan must add `packages/connector/tests/conformance.test.ts` that loads `tests/vectors/jcs_vectors.json` + `signing_vectors.json` and asserts byte-identical canonical output and signature equality. |
| DEBT-03 | Bidirectional message signing exercised in integration test | `packages/connector/tests/e2e-server.test.ts` exists and exercises agent send + human send paths but skips when server unavailable (no CI gate). `MessageSigner.verify` round-trip is unit-tested (`MessageSignerTests.swift::testSignVerifyRoundTrip`). Gap: no test where a human-signed envelope (Swift) is verified by Rust AND an agent-signed envelope (Rust signing or TS signing) is verified by Swift in the same run. Plan must add an integration test that captures one envelope from each direction and verifies it cross-platform. |
| DEBT-04 | Ed25519 keypair persists across iOS launches via Keychain | `KeyManager.swift` (line 63) currently uses `kSecAttrAccessibleAfterFirstUnlock`. Required: `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` plus explicit `kSecAttrSynchronizable: false`. `ThisDeviceOnly` blocks iCloud Keychain backup transfer to a different device — correct posture for a per-device identity key (per Apple Keychain Data Protection guide). One-line code change + new unit test that mocks `kSecAttrSynchronizable=true` rejection. |
| DEBT-05 | CI UAT simulator name discoverable rather than hardcoded | `Makefile` line 12 hardcodes `'iPhone 17, OS=26.2'`. `.github/workflows/ios-ci.yml` lines 30, 39 hardcode `'iPhone 16, OS=18.0'`. Two different hardcoded sims drift. Standard fix: shell wrapper that runs `xcrun simctl list -j devices available` then jq-greps for the newest available iPhone simulator. Document in `docs/testing.md`. |
| DEBT-06 | User can read recent messages offline from GRDB cache | `AppDatabase.swift` already uses GRDB DatabasePool with WAL mode. `ConversationListViewModel.startObserving` uses `ValueObservation` which fires regardless of WebSocket state. Gap is HUMAN-UAT only: enable airplane mode on a populated app, kill WebSocket, verify list + thread detail still render from local DB. No code change needed. |
| DEBT-07 | Conversation list updates in real-time as new messages arrive | `ConversationListViewModel` already subscribes via GRDB `ValueObservation.values(in:)` — write-through cache means any new message inserted by `MessageRepository` triggers a list refresh without pull-to-refresh. Gap is HUMAN-UAT only: send a message from a second human while iOS is foregrounded, verify list updates within 1s. No code change needed. |
| DEBT-08 | Agent identity badge visually distinct from human | `AgentMessageView.swift` lines 17-25 already render `AvatarView` + "Agent" caption when `senderType == "agent"`. Bubble container is `secondarySystemGroupedBackground` (system gray) vs `MessageBubbleView` `Color.accentColor` (blue). Gap is HUMAN-UAT only: verify in Light + Dark mode + Dynamic Type XXL + accessibility text size that the agent badge remains distinguishable. May need a Color contrast check (WCAG AA = 4.5:1). |
| DEBT-09 | Multi-block messages render in order without swallow/duplicate | `ContentBlockRenderer.swift` dispatches all 9 block types via switch. `MessageBubbleView` and `AgentMessageView` use `ForEach(Array(blocks.enumerated()), id: \.offset)` which preserves order and prevents key collisions. Gap is HUMAN-UAT only: send a synthetic text+code+image+file 4-block message and verify all four render in order. No code change. |
| DEBT-10 | `test_deepLink_missingMessage_showsNotFoundToast` passes without XCTSkip | `DeepLinkUITests.swift` line 92 has `try XCTSkipIf(true, ...)`. Root cause documented in code comment: `MessageThreadView.handleScrollToMessage` runs in `onAppear`, but the deep link Task that sets `scrollToMessageId` may fire *after* the view already appeared in UAT bootstrap. Fix approach: change `MessageThreadView` to observe `scrollToMessageId` as a `@State` value with `.onChange(of:)` rather than reading once in `onAppear`; or set `scrollToMessageId` in the launch-arg handler *before* the deep link route is pushed. |
| DEBT-11 | `SubThreadBranchUITests` reply-count badge asserted deterministically | `SubThreadBranchUITests.swift` line 98 has `_ = badge.waitForExistence(timeout: 5)` (soft assertion). Root cause documented at line 89-94: after branching + back navigation the sub-thread cache refresh triggers but `ForEach` in `MessageGroupView` doesn't always re-read `subThreadInfo`. Fix approach: make `subThreadCache` an `@Observable` reference (or `@Published` value) so `ForEach` participates in dependency tracking, then promote the assertion to `XCTAssertTrue(badge.waitForExistence...)`. |
| DEBT-12 | Dependency map produced for DEBT items before execution | This is a planning gate. See **DEBT Cluster Analysis** section below — research has produced the cluster map. The planner must consume it. |
| DEBT-13 | Full XCUITest suite (`make uat`) green after each DEBT cluster | `make uat` (Makefile lines 15-49) is the CI command. Plan must include a verification gate after every DEBT cluster lands. |
| RAT-01 | `@ainm/connector` ratified — coverage, error-paths, version pinning, structured logging, DECISIONS.md | Inventory: 7 test files (`agent`, `e2e-server`, `handshake`, `memory`, `rpc`, `signing`, `streaming`) totaling ~750 lines. **Gaps found:** (1) no DECISIONS.md, (2) `^` ranges on every dep, (3) `console.error` in `agent.ts` line 159 with raw error object — no log scrubber, (4) no error-path tests for handshake timeout, malformed server response, transport disconnect mid-RPC, (5) no cross-platform conformance test against `tests/vectors/`. |
| RAT-02 | `@ainm/connector-claude-code` ratified — E2E vs live CLI, OAuth-vs-bare tradeoff, orphan-process cleanup | Inventory: 5 test files (~1135 lines), `e2e-server.test.ts` skips if `claude --version` fails. **Gaps:** (1) no DECISIONS.md, (2) `--bare` mode tradeoff against OAuth flow not documented anywhere, (3) `currentProc.kill()` (line 194) sends SIGTERM only — no SIGKILL escalation, no detached process group, no leak-detection test (compare to MCP-07 which calls this out for Phase 19), (4) `^` deps. |
| RAT-03 | `@ainm/connector-openai` ratified — pre-upgrade baseline, structured logging, version pinning | **CRITICAL GAP: zero test files.** No `tests/` directory. `package.json` has no `test` script. This is the worst-ratified package. Plan must spec **at minimum**: parse-args test, mock `fetch` happy-path SSE test, mock `fetch` non-OK error path, briefing-injection test, system-prompt-override test. Plus: `OPENAI_API_KEY` is read into `headers["Authorization"]` (line 60) — must verify it never appears in any `console.*` output. |
| RAT-04 | Server `AgentRegistry` hardened — register/cleanup/broadcast tests, race-condition tests, mid-session disconnect | Source: `demo/src/server.rs` lines 127-202 (76 LoC). **Zero unit tests** in `demo/tests/integration.rs` (which is 19 functions all about memory/graph). Specific test gaps: (1) concurrent `register` for same identity → both connections receive `send_to_agent` (multi-instance support), (2) channel cleanup when sender dropped (verify `cleanup` retains correct count), (3) `agent_cards` removed when last connection dies, (4) registration during a `cleanup` → no race on `Arc<Mutex<...>>` (use `tokio::test` with `tokio::join!`). |
| RAT-05 | `dist/` excluded from git via `.gitignore` + CI enforcement | Currently `.gitignore` has no `dist/` rule. `git ls-files packages/` confirms 0 files in `dist/` are tracked, but `git status` shows `packages/*/dist/` as untracked — risk of accidental `git add -A`. Fix: add `packages/*/dist/` (or `packages/**/dist/`) line to `.gitignore`, plus a CI step that fails the build if `git ls-files` ever finds `packages/.*/dist/`. |
| RAT-06 | 8-point ratification checklist applied per package | See **Ratification Approach** section below for the canonical 8-point checklist. |

## Standard Stack

### Cross-Platform Crypto Conformance

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `@noble/ed25519` (TS) | **2.2.0** (current; 3.1.0 available — see migration note) | Ed25519 signing in connector SDK | Audited 4× post-v2 (April 2026 self-audit clean). 0 dependencies. Misuse-resistant. |
| `@noble/hashes` (TS) | 2.2.0 | SHA-512 backing for ed25519 sync API | Required by `@noble/ed25519` for `sha512Sync` setting. |
| `canonicalize` (TS, npm) | **2.0.0** (current 3.0.0 — see migration note) | RFC 8785 JCS in connector SDK | Crypto-safe canonicalization; predictable output. |
| `serde_json_canonicalizer` (Rust) | latest from crates.io | RFC 8785 JCS in `demo/` server | RFC 8785 compatible drop-in for `serde_json::to_string`. Already in `demo/Cargo.toml`. |
| Apple CryptoKit `Curve25519.Signing` (Swift) | iOS 18+ built-in | Ed25519 in iOS reference client | Native, hardware-accelerated, no third-party dep. |
| `JCSCanonicalizer` (Swift, in-tree) | n/a — `ios/AINMMessenger/AINMMessenger/Crypto/JCSCanonicalizer.swift` | RFC 8785 JCS in iOS | Hand-rolled per spec. Verified against `tests/vectors/jcs_vectors.json` in `MessageSignerTests`. |
| `ring::signature::Ed25519KeyPair` (Rust) | 0.17.x | Ed25519 in `demo/` server | Audited, used by rustls. Already in `demo/Cargo.toml`. |

**Migration note on TS deps:** Current TS connector pins `^2.0.0` for `canonicalize` and `^2.2.0` for `@noble/ed25519`. npm registry shows current versions are `canonicalize@3.0.0` and `@noble/ed25519@3.1.0`. **Do not auto-upgrade in Phase 15.** Phase 15 ratification means *exact-pin* the version that current production runs on (drop the `^`), defer the major-version migration to a follow-up. RAT-01 only requires "no `^` ranges" — pinning to `2.2.0` and `2.0.0` is fully acceptable.

**Version verification:** Run `npm view canonicalize@2.0.0 dist-tags` and `npm view @noble/ed25519@2.2.0 dist-tags` to confirm those exact versions still resolve. As of research date both resolve.

### iOS Keychain (DEBT-04)

| Constant | Value | Use For |
|----------|-------|---------|
| `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` | Required for DEBT-04 | Per-device Ed25519 identity key — must NOT migrate via iCloud Keychain to another device |
| `kSecAttrSynchronizable: false` (or omit, since default is false) | Belt-and-suspenders | Explicitly opts out of iCloud Keychain sync |

### XCUITest Race Remediation (DEBT-10, DEBT-11)

The two known-flaky tests share a common root cause: SwiftUI view dependency tracking does not propagate when a value is set on a non-`@Observable`/`@Published` reference type. The fix pattern:

| Symptom | Anti-pattern | Standard pattern |
|---------|--------------|------------------|
| `onAppear` reads a value that's set after appear | Read once in `onAppear` | `.onChange(of: value)` modifier + initial read |
| `ForEach` doesn't re-render when underlying cache mutates | Plain `class` cache referenced from view | `@Observable class` (iOS 17+) so SwiftUI tracks reads |

## Architecture Patterns

### Recommended Phase Structure

```
Wave 0: Inventory + .gitignore + CI Hygiene (parallelizable, foundation)
  - .gitignore for packages/*/dist/
  - CI fail-fast on tracked dist/ files
  - DECISIONS.md scaffolds in 4 packages
  - Pin all `^` deps to exact versions

Wave 1: DEBT Cluster A — Keychain + Crypto (DEBT-02, 03, 04)
  - One-line KeyManager.swift accessibility class fix
  - TS conformance test loading tests/vectors/
  - Bidirectional integration test
  - make uat green-gate

Wave 2: DEBT Cluster B — UAT XCUITest stabilization (DEBT-10, 11)
  - Root-cause and fix MessageThreadView scrollToMessageId race
  - @Observable subThreadCache, hard-assert badge
  - make uat green-gate

Wave 3: DEBT Cluster C — Live Server HUMAN-UAT (DEBT-01, 06, 07, 08, 09)
  - Boot ainm-demo (NOT --uat mode) against fresh PostgreSQL
  - Run handshake, offline read, real-time list, agent badge,
    multi-block scenarios; record HUMAN-UAT.md flips to pass

Wave 4: DEBT Cluster D — CI Hygiene (DEBT-05, 13)
  - Simulator-name discovery in Makefile + ios-ci.yml
  - Anti-flake: make uat run twice in CI

Wave 5 (parallel with Waves 1-4): Off-GSD Ratification (RAT-01..06)
  - 8-point checklist per package, parallelizable across the 4 packages
```

### DEBT Cluster Analysis (DEBT-12)

The 13 DEBT items decompose into 4 clusters by shared surface, with dependencies:

| Cluster | Items | Shared Surface | Owner Code | Order |
|---------|-------|---------------|------------|-------|
| **A. Crypto + Keychain** | DEBT-02, DEBT-03, DEBT-04 | Ed25519/JCS/Keychain | `Crypto/`, `tests/vectors/`, `packages/connector/tests/` | 1st (foundation for HUMAN-UAT) |
| **B. UAT XCUITest stabilization** | DEBT-10, DEBT-11 | SwiftUI dependency tracking + `MessageThreadView` | `ios/AINMMessenger/UI/MessageThread/`, `AINMMessengerUITests/` | 2nd (must be green before HUMAN-UAT can be trusted) |
| **C. Live server HUMAN-UAT** | DEBT-01, DEBT-06, DEBT-07, DEBT-08, DEBT-09 | Real iOS device against `ainm-demo` (non-UAT mode) | `AINMClient`, `ConversationList`, `MessageThread`, `ContentBlockRenderer` | 3rd (depends on Cluster A trust + Cluster B stability) |
| **D. CI hygiene** | DEBT-05, DEBT-13 | `Makefile`, `.github/workflows/` | repo root, `.github/workflows/ios-ci.yml`, `ios-uat.yml` | Parallel with all clusters; final green-gate is DEBT-13 |

DEBT-12 is the gate that says "produce this map before execution." This research has produced it.
DEBT-13 is the gate that says "after each cluster lands, `make uat` must be green." Plan must encode this as a checkpoint.

## Ratification Approach (RAT-06: 8-point Checklist)

The canonical 8-point checklist from REQUIREMENTS.md applied per package. Each row is independently verifiable.

| # | Check | How to Verify | Pass Criterion |
|---|-------|---------------|----------------|
| 1 | Deps pinned (no `^` or `~`) | `cat package.json \| jq '.dependencies + .devDependencies'` | All values are exact strings (e.g. `"2.2.0"`, not `"^2.2.0"`) |
| 2 | Tests green | `npm test` from package dir | Exit 0; non-zero test count; ≥1 test per public export |
| 3 | DECISIONS.md present | `ls packages/<pkg>/DECISIONS.md` | File exists; covers ≥3 architecture choices with rationale |
| 4 | Logs redacted | `grep -RE "console\.(log\|error\|warn)" packages/<pkg>/src/` then audit each | Zero log lines emit `apiKey`, raw `briefing`, raw envelope, or full message content |
| 5 | Errors mapped | Errors thrown have a defined error class (not raw `Error`) | Custom error class exported per error mode (handshake, RPC timeout, transport, etc.) |
| 6 | Versions aligned with SDK parity | All 3 connector packages reference `@ainm/connector` at the same version | `package.json` `dependencies."@ainm/connector"` matches across `connector-claude-code` and `connector-openai` |
| 7 | No blocking TODOs | `grep -RE "TODO\|FIXME\|XXX" packages/<pkg>/src/` | Zero comments tagged TODO/FIXME without an explicit GitHub issue ref |
| 8 | CI integrated | `.github/workflows/connector-ci.yml` (new) or `ios-ci.yml` extended | Each package has a CI job that runs `npm test` on push/PR |

**Per-package gap matrix** (state at research time):

| Check | `@ainm/connector` | `@ainm/connector-claude-code` | `@ainm/connector-openai` | server `AgentRegistry` |
|-------|-------------------|-------------------------------|-------------------------|------------------------|
| 1. Deps pinned | ❌ all `^` | ❌ all `^` | ❌ all `^` | ✅ Cargo.toml exact |
| 2. Tests green | ⚠ green but no `tests/vectors/` conformance | ✅ + e2e-server skips if no CLI | ❌ **zero tests** | ❌ **zero unit tests** |
| 3. DECISIONS.md | ❌ missing | ❌ missing | ❌ missing | ❌ missing |
| 4. Logs redacted | ⚠ `console.error` raw error in agent.ts:159 | ⚠ 6 `console.log` calls — review per call | ⚠ 2 `console.log` — verify no API key | n/a (Rust `tracing`) |
| 5. Errors mapped | ⚠ throws `new Error(...)` strings | ⚠ same | ⚠ returns string from handler on error | ✅ `anyhow::Error` throughout |
| 6. SDK parity | n/a (it IS the SDK) | ✅ `file:../connector` | ✅ `file:../connector` | n/a (Rust) |
| 7. No blocking TODOs | (audit needed) | (audit needed) | (audit needed) | (audit needed) |
| 8. CI integrated | ❌ no `connector-ci.yml` | ❌ same | ❌ same | ⚠ Rust tests run in `cargo test` but no AgentRegistry tests yet |

## Cross-Platform Crypto Verification Approach (DEBT-02 deep dive)

The one place where Phase 15 needs genuine technical research, not closure.

### Current state of test vectors

`tests/vectors/jcs_vectors.json` (4 vectors): `simple_sorted_keys`, `nested_objects`, `boolean_null_types`, `string_escaping`, `message_envelope_shape`. Each vector has `input` (object) + `expected_canonical` (string).

`tests/vectors/signing_vectors.json` (3+ vectors): RFC 8032 Test Vector 1 deterministic seed (`9d61b19d…`). Each vector has `envelope_without_signature`, `expected_canonical`, `expected_signature_base64url`.

### Where each platform stands

| Platform | JCS implementation | Loads test vectors? | Status |
|----------|-------------------|---------------------|--------|
| Rust (`demo/`) | `serde_json_canonicalizer` crate | Yes — generated by `examples/generate_vectors.rs` (vectors authoritative source) | ✅ |
| Swift (`ios/`) | `JCSCanonicalizer.swift` (in-tree) | Yes — `MessageSignerTests.swift` traverses to `tests/vectors/` via `repoRoot()` | ✅ |
| TypeScript (`packages/connector/`) | `canonicalize` npm package | **No** — `signing.test.ts` only does internal round-trip | ❌ **GAP** |

### Plan must add (DEBT-02 closure)

1. **`packages/connector/tests/conformance.test.ts`** — loads `../../../tests/vectors/jcs_vectors.json` and `signing_vectors.json`, asserts:
   - `canonicalize(input) === expected_canonical` for each JCS vector (string equality, not deep equal — must be byte-identical)
   - For each signing vector: derive keypair from seed, run `signEnvelope` on `envelope_without_signature`, assert `signature === expected_signature_base64url`
2. **Wire path** — `tests/vectors/` lives at repo root; from `packages/connector/tests/`, that's `../../../tests/vectors/`. Use `path.resolve(__dirname, '../../../tests/vectors/...')`.
3. **Seed conversion** — `signing_vectors.json` provides `private_key_seed_hex`. `@noble/ed25519` v2 takes raw 32-byte seed: `Buffer.from(hex, 'hex')`. The seed produces the exact same keypair under `ring::signature::Ed25519KeyPair::from_seed_unchecked()` (Rust), `Curve25519.Signing.PrivateKey(rawRepresentation:)` (Swift), and `noble-ed25519.getPublicKey(seed)` (TS). Verified by RFC 8032 Section 7.1 deterministic test vectors.

### Bidirectional integration test (DEBT-03)

A single Vitest scenario that:
1. Boots `ainm-demo` server (already happens in `e2e-server.test.ts`)
2. Connects two SDK agent instances with two different keypairs (A and B)
3. Agent A sends a signed message; Rust server verifies (proven by it broadcasting to A's thread)
4. Captures Rust server's stored envelope, replays it through Swift `MessageSigner.verify` (test vectors fixture)
5. Captures envelope from Rust handshake response signed by server, verifies via TS SDK

If full server-side message capture is too heavyweight, a unit-level alternative: use the `signing_vectors.json` to assert TS signs → Swift verifies (load same vector, sign in TS, hand the signature to a Swift-equivalent `verifyEnvelope` that's been ported to a Node-side mock that uses `tweetnacl` or `@noble/ed25519` directly with the public key from the seed).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| RFC 8785 JCS in TypeScript | Custom serializer | `canonicalize@2.0.0` (already pinned) | Hand-rolling gets ECMAScript number formatting, key sort, and string escapes wrong; existing crate is RFC-conforming |
| Ed25519 in TypeScript | Native crypto bindings | `@noble/ed25519@2.2.0` | Audited 4×; 0 deps; misuse-resistant |
| Simulator name discovery in CI | Hardcoded `'iPhone 17, OS=26.2'` | `xcrun simctl list -j devices available` + jq filter | Sim names drift across Xcode releases; current Makefile and ios-ci.yml already drift apart |
| iOS Keychain wrapper | Custom Keychain manager | The existing `KeyManager.KeychainStorage` (just fix the accessibility constant) | Already in-tree, tested with `InMemoryStorage` mock, just needs DEBT-04 one-liner |
| Process tree cleanup in connector-claude-code | Hand-rolled `kill -9` retry | `detached: true` + `process.kill(-pid, 'SIGTERM')` then `SIGKILL` escalation | Phase 19 MCP-07 has the same need; researching it once for RAT-02 produces the canonical pattern |
| Structured logging in connector packages | `console.log` | Lightweight `pino`-style logger or even a 30-line `log.ts` that scrubs `apiKey`, `signature`, `briefing` fields | Out-of-the-box `console.*` calls leak secrets if a future maintainer adds an object dump |

## Common Pitfalls

### Pitfall 1: HUMAN-UAT theater
**What goes wrong:** Marking DEBT-01/06/07/08/09 "passed" by reading the code and concluding "this should work" without actually running it on a device.
**Why it happens:** All five items have well-structured code already in-tree. The temptation is to skip the live run.
**How to avoid:** Plan must require a recording or screenshot per HUMAN-UAT item, attached to the verification artifact. Particularly DEBT-01 needs a packet capture or server log showing the three-way handshake against a fresh `ainm-demo` (not `--uat`) DB.
**Warning signs:** A wave's verification report says "code review confirms" without a `tmp/uat-*.png` or `tmp/uat-handshake.log` referenced.

### Pitfall 2: Keychain accessibility constant tested only with InMemoryStorage
**What goes wrong:** DEBT-04's one-line change to `KeychainStorage.save` doesn't get exercised by unit tests because tests use `InMemoryStorage`.
**Why it happens:** Existing `KeyManagerTests.swift` uses the in-memory storage to avoid Simulator Keychain entitlement issues (`errSecMissingEntitlement -34018`).
**How to avoid:** Add a new test class that runs only on a real device or a properly entitled UI test target — assert that after `getOrCreateKeypair()`, querying `SecItemCopyMatching` with `kSecReturnAttributes` returns `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` for the stored item. Run on the UAT simulator (which has the right entitlements per Phase 14 work).
**Warning signs:** DEBT-04 closes with only a code change and no new test.

### Pitfall 3: TS conformance test passes locally but the path resolution is wrong in CI
**What goes wrong:** `packages/connector/tests/conformance.test.ts` loads `../../../tests/vectors/...`. If CI checks out the package separately or runs vitest from a different cwd, the relative path breaks silently (test skips with "vector file not found").
**How to avoid:** Use `path.resolve(__dirname, ...)` not relative imports. If file not found, `throw new Error("vectors not found at...")` rather than skipping. Vitest must fail loud, not warn.
**Warning signs:** Conformance test reports green but logs `0 vectors loaded`.

### Pitfall 4: dist/ gitignored only at top level, not enforced in CI
**What goes wrong:** `.gitignore` adds `packages/*/dist/`, but a developer with a stale local clone runs `git add -A` from a subdirectory and ends up adding the dist files (overriding the gitignore via explicit add).
**How to avoid:** CI step `git ls-files | grep -E "packages/.*/dist/" && exit 1` runs on every PR. The CI failure is louder than gitignore.
**Warning signs:** dist files show up in `git diff origin/main` despite the gitignore rule.

### Pitfall 5: Soft-asserted XCUITest "passes" even when the badge never appears
**What goes wrong:** DEBT-11's current `_ = badge.waitForExistence(timeout: 5)` returns false silently. The test reports green even when the bug is reproduced.
**How to avoid:** Once the underlying SwiftUI fix lands, immediately promote the assertion to `XCTAssertTrue(badge.waitForExistence(timeout: 5), "Reply badge did not appear after sub-thread branch+back")`. The promotion is the actual closure of DEBT-11, not the SwiftUI fix alone.
**Warning signs:** PR diff shows the SwiftUI fix but the test file is unchanged.

### Pitfall 6: AgentRegistry race-condition test that doesn't actually race
**What goes wrong:** A `tokio::test` that registers two agents sequentially and asserts both are listed. That's not a race — the actual race is two registers happening *concurrently* on the same identity.
**How to avoid:** Use `tokio::join!(reg.register("a@x"), reg.register("a@x"))` and assert the post-state has 2 channels for `a@x` (multi-instance support intent), or alternatively assert that `cleanup` after one drops leaves the other intact.
**Warning signs:** AgentRegistry tests pass but use single `await`s — no `join!` or `spawn` calls.

### Pitfall 7: Ratifying packages by checking files exist, not content quality
**What goes wrong:** `DECISIONS.md` is created with a 1-line "TBD" placeholder. RAT-06 row 3 says "DECISIONS.md present" — technically passes.
**How to avoid:** RAT-06 verification must include: each DECISIONS.md covers ≥3 architectural choices with rationale (why this lib, why this pattern, why not the alternative). Reuse the format from `.planning/phases/*/CONTEXT.md` "Decisions" sections.
**Warning signs:** DECISIONS.md is < 30 lines.

## Code Examples

### Pattern 1: TypeScript conformance test against shared vectors

```typescript
// packages/connector/tests/conformance.test.ts
// Source: tests/vectors/README.md (D-08, D-09 design decisions)
import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import canonicalize from "canonicalize";
import * as ed from "@noble/ed25519";
import { sha512 } from "@noble/hashes/sha2.js";
import { signEnvelope } from "../src/identity/signing.js";
ed.etc.sha512Sync = sha512;

const VECTORS_DIR = resolve(__dirname, "../../../tests/vectors");

describe("RFC 8785 JCS conformance (cross-platform)", () => {
  const jcs = JSON.parse(readFileSync(resolve(VECTORS_DIR, "jcs_vectors.json"), "utf8"));
  for (const v of jcs.vectors) {
    it(`canonicalize matches expected for ${v.name}`, () => {
      const actual = canonicalize(v.input);
      expect(actual).toBe(v.expected_canonical);  // byte-identical, NOT toEqual
    });
  }
});

describe("Ed25519 signing conformance (cross-platform)", () => {
  const signing = JSON.parse(readFileSync(resolve(VECTORS_DIR, "signing_vectors.json"), "utf8"));
  const seed = Buffer.from(signing.key_pair.private_key_seed_hex, "hex");

  for (const v of signing.vectors) {
    it(`signature matches expected for ${v.name}`, async () => {
      const publicKey = await ed.getPublicKeyAsync(seed);
      const signed = await signEnvelope({ privateKey: seed, publicKey }, v.envelope_without_signature);
      expect(signed.signature).toBe(v.expected_signature_base64url);
    });
  }
});
```

### Pattern 2: KeyManager DEBT-04 fix (one-line + new test)

```swift
// ios/AINMMessenger/AINMMessenger/Crypto/KeyManager.swift, line 63
// Before:
//   kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock,
// After:
let addQuery: [String: Any] = [
    kSecClass as String: kSecClassGenericPassword,
    kSecAttrService as String: KeyManager.keychainService,
    kSecAttrAccount as String: KeyManager.keychainAccount,
    kSecValueData as String: data,
    kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
    kSecAttrSynchronizable as String: false,  // explicit; default is also false
]
```

```swift
// New test in KeychainStorage-only suite (runs on real device or entitled UI test target)
@Test("Stored key is non-synchronizable and ThisDeviceOnly")
func testKeychainAccessibility() throws {
    let storage = KeyManager.KeychainStorage()
    try storage.save(Data(repeating: 0xAB, count: 32))

    let query: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrService as String: "com.ainm.messenger",
        kSecAttrAccount as String: "ed25519.privateKey",
        kSecReturnAttributes as String: true,
    ]
    var result: AnyObject?
    let status = SecItemCopyMatching(query as CFDictionary, &result)
    #expect(status == errSecSuccess)
    let attrs = result as! [String: Any]
    #expect(attrs[kSecAttrAccessible as String] as? String
            == String(kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly))
    #expect((attrs[kSecAttrSynchronizable as String] as? Bool) == false)
}
```

### Pattern 3: Simulator name discovery in Makefile (DEBT-05)

```makefile
# Replace hardcoded UAT_DESTINATION with discovery
UAT_SIM_NAME := $(shell xcrun simctl list -j devices available | \
  jq -r '[.devices | to_entries[] | select(.key | contains("iOS")) | .value[] | \
         select(.name | startswith("iPhone")) | .name] | first // "iPhone 16"')
UAT_SIM_OS   := $(shell xcrun simctl list -j devices available | \
  jq -r '[.devices | to_entries[] | select(.key | contains("iOS")) | .key] | \
         first | split(".") | .[3:] | join(".")')
UAT_DESTINATION := 'platform=iOS Simulator,name=$(UAT_SIM_NAME),OS=$(UAT_SIM_OS)'
```

### Pattern 4: AgentRegistry race-condition test

```rust
// demo/tests/agent_registry.rs (new file)
use ainm_demo::server::AgentRegistry;
use serde_json::json;

#[tokio::test]
async fn test_concurrent_register_same_identity_keeps_both() {
    let reg = AgentRegistry::new();
    let card = json!({"name": "test"});

    // Two concurrent registers for the same identity → both channels live
    let (rx1, rx2) = tokio::join!(
        reg.register("a@x.local", &card),
        reg.register("a@x.local", &card),
    );
    drop(rx1);  // simulate one connection dropping
    drop(rx2);

    reg.cleanup("a@x.local").await;
    let agents = reg.list_agents().await;
    assert!(!agents.contains(&"a@x.local".to_string()),
            "after both connections drop, identity should be cleaned up");
}

#[tokio::test]
async fn test_cleanup_during_active_register_no_panic() {
    let reg = AgentRegistry::new();
    let card = json!({"name": "test"});

    let _rx = reg.register("b@x.local", &card).await;
    let cleanup_task = tokio::spawn({
        let reg = reg.clone();
        async move { reg.cleanup("b@x.local").await }
    });
    let new_register = tokio::spawn({
        let reg = reg.clone();
        let card = card.clone();
        async move { reg.register("b@x.local", &card).await }
    });

    let (_, _) = tokio::join!(cleanup_task, new_register);
    // Should not panic; final state may have 1 or 2 connections depending on timing,
    // but must not deadlock or corrupt internal state.
    let _agents = reg.list_agents().await;
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `kSecAttrAccessibleAfterFirstUnlock` (allows iCloud Keychain backup) | `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` for per-device identity keys | Apple Security Guide 2025 update | Required for DEBT-04; key cannot be migrated to a stolen-and-restored device |
| `@noble/ed25519` v1 (separate sync/async APIs, larger surface) | `@noble/ed25519` v2 (5KB, smaller attack surface) | v2 release, post-audit hardened in April 2026 | Phase 15 stays on v2.2.0 (current pinned `^2.2.0`); v3.1.0 migration deferred |
| `XCTSkipIf` to mask flaky XCUITest | Root-cause SwiftUI dependency tracking, promote to hard-assert | Phase 14 → Phase 15 (per audit recommendation) | DEBT-10, DEBT-11 closure |
| Sequential server bring-up in CI | Hermetic UAT via `make uat` (Phase 14 shipped this) | Phase 14 (2026-04-19) | Phase 15 just hardens it (DEBT-05, DEBT-13) |
| Hardcoded simulator names | `xcrun simctl list -j devices available` + jq | iOS 17 era best practice | DEBT-05 |

**Deprecated/outdated:**
- iOS Simulator hardcoded `iPhone 16, OS=18.0` in `ios-ci.yml` and `iPhone 17, OS=26.2` in `Makefile` — drift, will fail when GitHub Actions image changes
- `console.log` of full `briefing` JSON in `connector-openai/openai-agent.ts:44` — leaks user memory graph contents to stdout

## Open Questions

1. **Does `--bare` mode in Claude Code connector mean "API key only" or "no OAuth at all"?**
   - What we know: `claude-code-agent.ts` accepts `bare?: boolean` and forwards it to `buildClaudeArgs`; `e2e-server.test.ts` runs with `bare: true`.
   - What's unclear: Whether OAuth-mode Claude Code (the default for human users) actually works with the connector, or whether `--bare` is required.
   - Recommendation: RAT-02 closure includes a `DECISIONS.md` entry that documents the OAuth-vs-`--bare` tradeoff explicitly. If OAuth mode does not work, that's a Phase 16 AU-03 dependency to flag.

2. **Should DEBT-04 require key rotation if user previously had a non-`ThisDeviceOnly` key stored?**
   - What we know: One-line change to `KeychainStorage.save` only affects future writes. Existing keys in Keychain retain their old accessibility class until deleted.
   - What's unclear: Whether to force-rotate all existing keys on first launch after the upgrade.
   - Recommendation: Add a migration check in `getOrCreateKeypair`: query the existing item's attributes, if `kSecAttrAccessible != kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`, delete and regenerate. Document in DECISIONS.md.

3. **Does the AgentRegistry need to reject duplicate `agent_card` mutations during register?**
   - What we know: Current `register` (server.rs:141) overwrites `agent_cards` map on every call.
   - What's unclear: If a malicious agent could re-register with a different agent card and silently change its published capabilities mid-session.
   - Recommendation: This is more Phase 16 (`connector_keys` allowlist) than Phase 15. Document as an open question for Phase 16 AU-06.

4. **Do we want a connector-side log scrubber library or hand-roll a 30-line `log.ts`?**
   - What we know: 13 `console.*` calls across 5 source files.
   - What's unclear: Worth pulling in `pino` (heavyweight, structured) vs writing a minimal `log.ts` that wraps console with a redactor for `apiKey`/`signature`/`briefing` paths.
   - Recommendation: Hand-roll for v1 ratification — keeps deps minimal, ratification scope tight. Plan can add `pino` in a later milestone if structured-logs become a real need.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain (cargo) | demo server build for HUMAN-UAT | (assume yes — repo runs Phase 14) | 1.85+ | — |
| PostgreSQL | demo server full mode (DEBT-01 live HUMAN-UAT) | (assume yes — Phase 14 docs reference it) | 16+ | `--uat` mode uses ephemeral DB but does not exercise full handshake path |
| Xcode 16+ | iOS build, XCUITest | (assume yes — Phase 14 shipped XCUITest) | 16+ | — |
| Node.js | TypeScript SDK build + test | (assume yes — packages are built locally) | ≥20 (per package.json `engines`) | — |
| `claude` CLI | RAT-02 e2e-server test | likely no on CI runners | n/a | Test self-skips with `claudeCliAvailable()` check; document as known CI gap |
| `xcrun simctl` | DEBT-05 simulator discovery | yes (macOS only, already used) | n/a | — |
| `jq` | DEBT-05 Makefile snippet | usually yes on macOS-14 GH runners | n/a | Install in CI via `brew install jq` if missing |

**Missing dependencies with no fallback:** None for Phase 15 closure.

**Missing dependencies with fallback:**
- `claude` CLI on CI: RAT-02 e2e tests skip; document as a CI gap with manual-run instructions in `packages/connector-claude-code/DECISIONS.md`.

## Validation Architecture

`workflow.nyquist_validation` is set to `false` in `.planning/config.json` — section omitted per policy.

## Risks and Unknowns

1. **Risk: HUMAN-UAT Cluster C (DEBT-01/06/07/08/09) takes longer than expected because no production-grade Rust server config (PostgreSQL + TLS certs) exists outside the `--uat` ephemeral mode.** Mitigation: Wave 3 starts with provisioning (`docker-compose up -d` for postgres, `./scripts/gen-dev-certs.sh`) before any HUMAN-UAT step. Reuse existing `demo/scripts/` and `demo/certs/` if present.

2. **Risk: TS conformance test against `tests/vectors/` reveals that `canonicalize@2.0.0` produces output that differs from Swift `JCSCanonicalizer` or Rust `serde_json_canonicalizer` for a vector — meaning real cross-platform parity has been broken silently.** Mitigation: Plan must run conformance test FIRST in Wave 1 (before any other work), so a divergence is caught immediately. If divergence found, escalate as a blocker — Phase 17 Python SDK and all later connector work depend on this.

3. **Risk: DEBT-10 root cause is not the SwiftUI `onAppear` race but something deeper (e.g. the deep-link Task being cancelled by SwiftUI on view rebuild).** Mitigation: Plan should budget for an investigation step in Cluster B before committing to a fix — XCUITest under `make uat-debug` plus `os_log` traces on `MessageThreadView.handleScrollToMessage`.

4. **Unknown: Are there OTHER off-GSD packages or off-GSD code paths we haven't catalogued?** All 4 packages mentioned in REQUIREMENTS.md are accounted for. Server `AgentRegistry` is the only Rust-side off-GSD surface called out. No other `packages/*` or `demo/src/*` modules are flagged. Risk is low but plan should run a one-time "off-GSD inventory" verification at end of Wave 0.

5. **Risk: 8-point checklist row 7 ("no blocking TODOs") finds enough TODOs that closure becomes a separate workstream.** Mitigation: Plan an explicit Wave for TODO triage during ratification — for each TODO either fix in-phase, file as deferred GitHub issue with link in code comment, or remove if obsolete.

## Sources

### Primary (HIGH confidence)
- `tests/vectors/README.md` + `jcs_vectors.json` + `signing_vectors.json` — authoritative cross-platform conformance corpus (D-08, D-09)
- `ios/AINMMessenger/AINMMessenger/Crypto/{KeyManager,JCSCanonicalizer,MessageSigner}.swift` — current iOS crypto state (read directly)
- `demo/src/server.rs:39-202` — current AgentRegistry/HumanRegistry implementation
- `demo/src/signing.rs` — Rust JCS+Ed25519 implementation
- `packages/connector/src/{agent,identity/signing,identity/keypair,protocol/handshake}.ts` — TS connector implementation
- `packages/{connector,connector-claude-code,connector-openai}/package.json` — current dependency state
- `.planning/milestones/v3.2-MILESTONE-AUDIT.md` — origin of all 13 DEBT items
- Apple Developer docs: [`kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`](https://developer.apple.com/documentation/security/ksecattraccessibleafterfirstunlockthisdeviceonly) — DEBT-04 authority

### Secondary (MEDIUM confidence)
- npm registry `npm view canonicalize@2.0.0` and `npm view @noble/ed25519@2.2.0` — confirmed exact versions resolve
- WebSearch on `@noble/ed25519` v2 status — reports 4× audited and 2026-April hardened (corroborates HIGH-confidence package metadata)
- WebSearch on Keychain accessibility classes — confirms `ThisDeviceOnly` semantics (cross-checked with Apple official docs)

### Tertiary (LOW confidence)
- None used; this phase is closure work on in-tree code, so all critical claims are verifiable by reading the repo.

## Metadata

**Confidence breakdown:**
- DEBT cluster scope: HIGH — all source code read directly, all referenced tests inspected
- Ratification gap matrix: HIGH — file-system-level audit of each package
- Cross-platform crypto approach: HIGH — vectors exist, only the test wiring is missing
- Open questions: MEDIUM — flagged honestly, planner will need to disposition

**Research date:** 2026-04-18
**Valid until:** 2026-05-18 (30 days — stable in-tree code; flag for refresh if any package's `package.json` changes upstream)

## RESEARCH COMPLETE
