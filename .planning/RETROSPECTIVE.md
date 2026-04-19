# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v3.1 — AI-Native Messaging Protocol v1 Specification & Demo

**Shipped:** 2026-03-29
**Phases:** 6 | **Plans:** 20 | **Tasks:** 38

### What Was Built
- Complete v1 protocol specification (11 prose docs, 12 JSON Schema files, AsyncAPI 3.1.0)
- Rust CLI demo (single-binary Axum server) proving protocol end-to-end with Ed25519 signing, streaming, interactive content
- Docusaurus spec publication site with GitHub Pages CI/CD
- Full conformance polish: all 13 audit findings resolved, memory/bridge scenarios added, handshake enforcement, zero cargo warnings

### What Worked
- **Protocol-first approach**: Writing spec prose before schemas before demo forced clean design decisions early
- **Coarse granularity**: 3 original phases expanded to 6 via audit-driven gap closure -- right level of planning
- **Milestone audit before completion**: Caught 13 conformance issues that would have shipped as inconsistencies
- **Phase-by-phase execution**: Each phase had clear boundaries and independently verifiable success criteria
- **Research-informed design**: AMP, A2A, A2UI, MCP, Letta research prevented reinventing solved problems

### What Was Inefficient
- **ROADMAP.md progress table drift**: Phases 3-6 showed stale status markers (checkboxes unchecked, "0/N plans") despite all plans being executed. Required manual cleanup.
- **Phase 4-6 were reactive**: Created purely to close audit gaps. Could have caught some issues during Phase 3 execution with tighter validation loops.
- **Demo port documentation**: Documenting port 9090 when demo binds to random port was a trivial error that persisted across multiple phases.

### Patterns Established
- RFC 8785 JCS + Ed25519 as canonical signing approach for messaging protocols
- Declarative UI-as-data model (A2UI-inspired) for agent interactive content
- Three-tier memory model (active/conversation/archival) with structured context frames
- A2A-compatible Agent Cards with namespaced extensions
- JSON-RPC 2.0 over WebSocket as transport layer

### Key Lessons
1. **Audit before milestone completion**: The v3.1 audit found 13 issues that led to 3 additional phases. Build audit into the workflow, not as an afterthought.
2. **Schema-first validation**: JSON Schema definitions forced disambiguation of spec prose that was otherwise ambiguous (e.g., MessageEnvelope types, field names).
3. **Demo as conformance test**: Building the demo revealed every spec-to-implementation gap. The demo is the real test suite for a protocol spec.
4. **Spec prose must be authoritative**: When demo diverged from spec (field names, URL paths), always fix the demo, never the spec.

### Cost Observations
- Model mix: ~90% opus, ~10% sonnet (research agents)
- Sessions: ~5 sessions across 2 days
- Notable: 20 plans in 2 days (~100 minutes total execution across all plans). High velocity due to spec-writing nature (no compilation/testing loops).

---

## Milestone: v3.2 — iOS Reference Client

**Shipped:** 2026-04-19
**Phases:** 5 (10-14) | **Plans:** 24 | **Tasks:** ~60

### What Was Built
- Native SwiftUI iOS app connecting to AINM server via WebSocket + JSON-RPC 2.0 (Phase 10)
- Full content block rendering — text, code, image, file — with offline access (Phase 11)
- Token-by-token streaming + interactive blocks (buttons, forms, tables, approvals) (Phase 12)
- Tool call lifecycle indicators, @-mention autocomplete, agent discovery, memory context sheet with archival facts, sub-thread branching, `ainm://` deep links (Phase 13)
- Hermetic XCUITest harness: `AINMMessengerUITests` target, `SeedLoader` + 6 fixture JSONs, `TestEventInjector` + event scripts, `make uat` orchestrator, 10 tests green (Phase 14)
- 34/34 v1 iOS requirements shipped; 127 accessibility identifier attachments for stable test targeting

### What Worked
- **Locked architecture via discuss-phase**: Phase 14's CONTEXT.md captured 16 decisions (D-01..D-16) that cleanly separated concerns across Wave 1-4. Each plan had unambiguous boundaries.
- **Named TODO anchors for parallel editing**: 14-01 installed `TODO(14-03)` and `TODO(14-04)` markers in AINMMessengerApp.swift, letting 14-03 and 14-04 edit the same file in parallel worktrees without merge conflicts.
- **Research-corrected planning**: The planner was briefed with critical corrections BEFORE writing plans (binary name `ainm-demo` not `ainm-server`; `/health` endpoint wiring; Xcode synchronized-group mechanics; iOS runner can't invoke `Process()`). Saved an entire revision cycle.
- **Checker-verified planning loop**: 2 revision passes caught (a) file-path contradiction between frontmatter and task actions in 14-04, (b) missing scenario-6 test coverage (only 1 of 4 deep-link paths covered), (c) Wave 2 parallel-edit conflict on AINMMessengerApp.swift. All would have been painful to catch during execution.
- **Greppable verification in acceptance criteria**: Every task had `grep -q`/`xcodebuild` verifiable conditions. Executors couldn't fake success.
- **iterate-until-green in Task 5**: The 14-06 agent iterated 8 times on `make uat` failures, fixing real issues each time (identifier mismatches, timing races, ProgressView query, ForEach refresh). The automated loop diagnosed + fixed what humans would have handled the same way.

### What Was Inefficient
- **Worktree merging was manual and error-prone**: 14-01, 14-02, 14-03 required manual `git merge --no-ff` + STATE.md/ROADMAP.md conflict resolution every time. 14-04 and 14-05 bypassed worktrees entirely (committed to main directly), highlighting that the worktree isolation model isn't consistently enforced.
- **STATE.md counter drift**: Progress percentages and phase counts drifted during merges; auto-updated fields from parallel agents conflicted with orchestrator's manual edits. Recounted at milestone close.
- **Milestone version string mismatch**: STATE/ROADMAP used `v3.2`, PROJECT.md + research docs used `v3.3`. The drift wasn't caught by any gate and surfaced only in the milestone audit. A lint pass on version consistency would have caught it.
- **Cross-plan ID drift in fixtures/scripts**: 14-03 seeded `fact-coffee-01`/`fact-meetings-01`; 14-04 event script referenced `fact-coffee`/`fact-meetings` (no suffix). Caught during merge spot-check but only because I grep'd for cross-plan references. A declarative contract between fixtures and event scripts (shared type defs) would have caught it at compile time.
- **Human-UAT asymmetry**: Phase 14 automated Phase 13's HUMAN-UAT (flipped to `complete`) but Phase 11's HUMAN-UAT remains `partial`. No systematic driver to bring Phase 10/11 UAT up to the same automation level.
- **SourceKit diagnostic noise**: IDE integration constantly reported "No such module 'GRDB'" / "No such module 'XCTest'" during this milestone — real builds passed, but diagnostics created signal/noise confusion during review.

### Patterns Established
- **Discuss → Research → Plan → Checker → Execute** chain with auto-advance gates between each; any gate can pause for user input. This is the default GSD cadence going forward.
- **Worktree-isolated parallel agents + named TODO anchors** for coordinating multi-agent edits on shared files.
- **Launch-argument-driven test bootstrap** (`-UITestSeed`, `-UITestEvents`, `-UITestDeepLink` under `#if DEBUG`) as the hermetic-testing pattern for SwiftUI apps.
- **GRDB-direct fixture seeding** (not full-protocol seeding) for UAT — trades some end-to-end fidelity for deterministic, fast, hermetic runs.
- **Shared `UITestIdentifiers.swift` enum namespace** (imported by both app target and UI test target) as the stable bridge between Swift view code and XCUITest queries.
- **Deterministic Ed25519 keypair from fixed 32-byte seed** for reproducible identity in tests (signatures still use randomized nonces, so sign+verify round-trip asserts instead of byte equality).

### Key Lessons
1. **Checker found 3 blockers + 6 warnings on first pass, 1 new blocker on second pass** — the gsd-plan-checker earned its compute budget. Never skip the checker loop for multi-plan phases.
2. **Research corrections beat CONTEXT.md when they diverge** — the researcher caught that `ainm-demo` (not `ainm-server`) was the real binary name. Brief downstream agents with corrections explicitly; don't rely on them to re-derive.
3. **Cross-plan ID coordination needs a formal contract** — fixtures and event scripts both reference the same IDs but have no shared schema. A Codable protocol (`FixtureID`/`EventRef`) or a generated ID constants file would catch drift statically.
4. **Worktree merging is the weak link in parallel orchestration** — when it works (14-01/02/03), parallelism is a big win; when it doesn't (14-04/05 silently bypassing), orchestration rules aren't enforced. Either make worktree-or-not explicit per agent, or unify on single-tree-with-locking.
5. **HUMAN-UAT automation needs its own phase pattern** — Phase 14 was the first phase to retire an upstream HUMAN-UAT. That pattern should be a first-class GSD command (`/gsd:automate-uat {phase}`) so Phase 11 (and future pending UATs) can follow the same playbook without bespoke planning.
6. **SourceKit diagnostics ≠ compile errors on Swift projects** — when working in a folder not opened in Xcode, SourceKit fails to resolve packages. Treat IDE diagnostics as advisory, not authoritative; always defer to `xcodebuild build`/`xcodebuild test`.
7. **Auto-approve checkpoints with pre-authorization, not auto-advance config** — the human-verify checkpoint in 14-06 was bypassed by orchestrator pre-authorization in the agent prompt, which is cleaner than toggling `auto_advance` config. Reuse for future checkpointed plans in auto-chains.

### Cost Observations
- Model mix: ~85% opus (researcher, planner, executors, verifier), ~10% sonnet (plan-checker, integration-checker), ~5% inline orchestration
- Sessions: 1 long session (discussion → execute → audit → complete) spanning ~3 hours
- Notable: The discuss/plan/check/revise loop took ~30 min and saved what likely would have been 1-2 hours of rework during execution. Plan investment paid back 3-4×.
- Worktree cleanup overhead: ~30+ stale worktrees accumulated during execution. `git worktree prune` + directory cleanup was a milestone-end chore.

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Sessions | Phases | Plans | Key Change |
|-----------|----------|--------|-------|------------|
| v3.1 | ~5 | 6 | 20 | First milestone. Established audit-driven gap closure pattern. |
| v3.2 | ~1 long | 5 | 24 | First use of full discuss → plan → check → execute auto-chain. First cross-plan HUMAN-UAT automation (Phase 14 retired Phase 13's manual UAT). |

### Cumulative Quality

| Milestone | Requirements | Coverage | Audit Findings |
|-----------|-------------|----------|----------------|
| v3.1 | 57/57 | 100% | 13 found, 13 resolved |
| v3.2 | 34/34 | 100% | 0 blocking, 11 tech debt items carried forward |

### Top Lessons (Verified Across Milestones)

1. **Audit before shipping** — catches conformance gaps that accumulate across phases (confirmed in both v3.1 and v3.2)
2. **Schema definitions force clarity** — ambiguous prose becomes unambiguous types (v3.1 via JSON Schema, v3.2 via Swift Codable models + XCUITest identifier enum)
3. **Checker loops earn their compute budget** — plan-checker and integration-checker both caught issues that would have been painful to discover during execution (v3.2 first saw this at scale)
4. **Research briefings must surface corrections explicitly** — downstream agents won't re-derive things the researcher already figured out (v3.2 `ainm-demo` binary name correction is canonical example)
