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

## Cross-Milestone Trends

### Process Evolution

| Milestone | Sessions | Phases | Key Change |
|-----------|----------|--------|------------|
| v3.1 | ~5 | 6 | First milestone. Established audit-driven gap closure pattern. |

### Cumulative Quality

| Milestone | Requirements | Coverage | Audit Findings |
|-----------|-------------|----------|----------------|
| v3.1 | 57/57 | 100% | 13 found, 13 resolved |

### Top Lessons (Verified Across Milestones)

1. Audit before shipping -- catches conformance gaps that accumulate across phases
2. Schema definitions force spec clarity -- ambiguous prose becomes unambiguous JSON Schema
