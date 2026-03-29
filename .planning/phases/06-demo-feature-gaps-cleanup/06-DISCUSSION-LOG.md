# Phase 06: Demo Feature Gaps & Code Cleanup - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-29
**Phase:** 06-demo-feature-gaps-cleanup
**Areas discussed:** Memory evict UX, ETag strategy, Dead code scope
**Mode:** Auto (all decisions auto-selected from recommended defaults)

---

## Memory Evict UX

| Option | Description | Selected |
|--------|-------------|----------|
| Evict by key with confirmation | Mirror /memory pattern, evict a previously stored fact | ✓ |
| Interactive evict with list | List stored facts, let user pick which to evict | |
| Batch evict all | Single command clears all archival facts | |

**User's choice:** Evict by key with confirmation (auto-selected)
**Notes:** Follows existing /memory scenario pattern. Simplest approach that proves the evict operation works end-to-end.

---

## ETag Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| SHA-256 content hash | Deterministic from JSON body, no state | ✓ |
| Version counter | Increment on card changes, requires state | |
| Timestamp-based | Use last-modified time | |

**User's choice:** SHA-256 content hash (auto-selected)
**Notes:** Standard HTTP caching pattern. No server-side state needed since the Agent Card is static in the demo.

---

## Dead Code Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Remove server forwarding + evaluate REPL arms | Remove confirmed dead code, investigate REPL reachability | ✓ |
| Remove all tool.call.* match arms everywhere | Aggressive removal across all files | |
| Keep all, add #[allow(dead_code)] | Suppress warnings without removing code | |

**User's choice:** Remove server forwarding + evaluate REPL arms (auto-selected)
**Notes:** Server forwarding arms are confirmed dead (tool events use ainm.message.send wrapper). REPL arms need investigation to confirm reachability before removing.

## Claude's Discretion

- Error message text for evict operations
- Whether to update REPL help text
- Task ordering within plans

## Deferred Ideas

None — all discussion stayed within phase scope.
