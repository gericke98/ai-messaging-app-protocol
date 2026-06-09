# M1 Security Floor — Ground-Truth Verification & Execution Record

**Date:** 2026-06-09
**Status:** Executed (quick-wins slice). This doc is the M1 spec/plan record required by the
parent roadmap, plus the claim-by-claim verification that reshaped it.
**Method:** Every M1 claim in `2026-06-09-ainm-ship-roadmap.md` was verified against code
before any fix. Roughly half did not survive contact.

## Claim-by-claim verdicts

| Roadmap claim | Verdict | Ground truth | Action taken |
|---|---|---|---|
| Hardcoded `dev-insecure-secret` is a forgeable hosted secret (S) | **Overstated, but real** | Literal lived only in `server::start()`/`start_with_port()`. Production path (`main.rs`) already env-gated via `prod_guard`. BUT legacy mode (`main.rs:1376`) uses `start()` — so `AINM_ENV=production` + legacy mode still shipped the literal. | Shared `auth_token::secret_from_env()` (pure core, TDD'd); literal removed from the codebase. |
| `AINM_LLM__API_KEY` not gated by prod_guard (S) | **TRUE** | `prod_guard::validate_with` gated token secret, encryption key, DB URL, mock-LLM flag, email — but not the LLM key; absence silently degrades to mock LLM in production. | Added to `validate_with` (TDD: red→green). |
| Bubblewrap silently degrades to `SandboxMode::None` (S) | **FALSE** | `sandbox::launcher_for()` already hard-errors (fail-closed); `mcp.rs` then warn+**skips** the server. Nothing ever runs unsandboxed. Real risk: silent feature loss in prod. A naive "hard-error when bwrap absent" would have broken macOS dev entirely. | `mcp::build_providers_checked(strict)`: in production a configured server that fails to start aborts boot. Dev keeps warn+skip. |
| `ainm.memory.graph.query` ungated — "any agent can query any user's graph" (S) | **FALSE (misdiagnosed)** | `query_nodes` is `WHERE user_addr = $1` bound to the **authenticated** caller (WS handshake verifies the claimed address against the registered Ed25519 key when a DB is present; graph methods require a DB). No cross-user read exists. Residual: no consent-*scope* check on self-reads — that is exactly what M3 grant verbs enforce. | Regression test pins the isolation invariant (`test_query_nodes_never_leaks_other_identities`). Blanket gate **rejected** — it would break the legitimate connector self-read path (`e2e_compound_memory.rs`). Grant-verb enforcement stays in M3. |
| Gated methods return `-32063` | **FALSE** | The family uses `-32062` ("Compound memory not supported"). `-32063` exists nowhere. | M3 exit criteria must pick a real code; roadmap corrected. |
| Embedded-migrations fix (S) | **TRUE — worse than claimed** | `db.rs` resolved `Path::new("migrations")` from CWD and **silently skipped** migrations when absent. Migrations live in `server/migrations/`, so running from the repo root skipped them. | `sqlx::migrate!` compile-time embed; missing-schema now impossible, failure is hard. |
| Two "vacuously passing" tests → annotate `#[ignore]` (S) | **WRONG FIX — they were failing, not vacuous-but-fine** | Both self-skip without Postgres like 20+ sibling tests. Against a live Postgres both **FAIL** (pre-existing, confirmed on clean HEAD). `#[ignore]` would have buried two real bugs. | Root-caused and fixed (below). CI Postgres service + skip-guard so DB tests can never silently skip again. |
| `ring` ≥ 0.17.8 (S) | **Already satisfied** | `cargo tree -i ring` → 0.17.14. | Retired by verification. |
| No server CI (M) | **TRUE** | Only iOS + docs workflows existed. | `rust-ci.yml`: fmt --check, clippy `-D warnings` (workspace made clippy-clean), tests with Postgres 16 service + "Skipping DB test" grep-guard. |

## The two real bugs the "vacuous tests" were hiding

1. **`compiler::run_compilation` reported wrong audit counts.** `dedup_and_integrate_nodes`
   returned the full label→id map (existing + new); `nodes_created` was its `len()`, so a
   candidate deduped onto an existing node was counted as *created* — in the result AND in
   the `record_compilation` audit row. Fixed: separate `created` counter.
2. **`test_graph_traversal` used direction `"outbound"`,** which is not in the schema enum
   (`compound-memory.schema.json`: `["outgoing","incoming","both"]`) — `traverse()`'s
   catch-all silently treated it as `"both"`. Its assertions also contradicted the
   deliberate contract that the returned subgraph includes the start node. Test corrected
   to the schema and the actual contract.

**Follow-up (small, unscheduled):** the `traverse()` catch-all accepts any garbage string as
`"both"`; the WS handler should reject non-enum `direction` values per schema.

## Verification

- `cargo test --workspace`: **327 passed, 0 failed, 3 ignored** (pre-existing ignores),
  against live Postgres — the formerly-failing two now run red→green for real.
- `cargo clippy --workspace --all-targets`: clean.
- `cargo fmt --check`: clean (one-time mechanical format commit).
- `grep -r "dev-insecure-secret" server/src`: empty.

## Remaining M1 exit criteria (need push / remote)

- CI badge green on GitHub (workflow committed; first run happens on push).
