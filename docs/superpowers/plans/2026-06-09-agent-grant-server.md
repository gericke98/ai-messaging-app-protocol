# Agent Grant — Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the per-`(user, agent)` Agent Grant a first-class, host-enforced object on the server — extend the existing `memory_grants` table with verbs/tools/write-back/signature, add a pure `grant` module, enforce the grant on every memory method, and expose user-authenticated grant CRUD (the signed manifest's read/write/revoke surface).

**Architecture:** The `memory_grants` table **already exists** and `briefing::resolve_scopes` already reads `(scopes, consent_level)` per `(user_addr, agent_addr)` and feeds the briefing + hosted runtime + memory tools. This plan *extends* that table (it does not invent a new one) with the grant's new dimensions, adds a pure `grant.rs` (preset→scopes/verbs, `allowed`/`has_verb`/`tool_allowed` matrix, JCS canonicalization for the user signature), and wires verb enforcement onto the **existing** `ainm.memory.*` methods — which maps cleanly: `briefing`→`derive`, `graph.query`→`read`, `graph.contribute`→write-back, `export`→`export` (the export verb even closes a currently *un-gated* read path). Grant CRUD endpoints authenticate via the existing account auth-session bearer (`caller_from_bearer`) and verify the user's Ed25519 signature over the grant using the existing JCS `signing` stack.

**Tech Stack:** Rust 1.85 / Axum 0.8 server (`server/src/`, package `ainm-server`); PostgreSQL via `sqlx`; `ring` Ed25519 + RFC 8785 JCS (`signing`); dual lib/bin module tree (`pub mod` in `lib.rs`, `mod` in `main.rs`).

**Scope (this plan = server only):** migration, pure `grant` module, grant repo, grant resolution + enforcement on existing memory methods, signed grant CRUD endpoints, black-box WS tests. **Out of scope → separate later plans:** iOS preset/override UX + "manage access" screen; the connector reading a grant summary; **D2's *push*-briefing-on-first-turn** (it requires connector-side handling of a pushed briefing notification, so it ships with the connector plan — the server here delivers the *enforcement + lifecycle* half of D2, not the push). L2 memory re-tiering and the L3 skill-accretion loop remain reserved interfaces (the `skill_namespace` column + reserved verbs only).

**Standing constraints:** commits to `main` via `rtk proxy git` (never bare git); push is user-gated separately at the end (do NOT push during execution); pure-logic-first; the black-box WS tests are DB-or-skip (self-skip without Postgres); verbatim-code plan → execution skips the per-task two-stage review and runs ONE holistic opus review at the end. **Build is gated on Task 14** (the compound-memory value go/no-go) — confirm that gate with the user before executing this plan.

---

## Current-State Anchors (verified 2026-06-09)

- `server/migrations/002_compound_memory.sql:81` — `memory_grants(user_addr TEXT, agent_addr TEXT, scopes JSONB DEFAULT '[]', consent_level VARCHAR(20) DEFAULT 'standard', granted_at TIMESTAMPTZ, PRIMARY KEY(user_addr, agent_addr))`. Next migration number is **008**.
- `server/src/briefing.rs:27` `ConsentLevel` (Minimal/Standard/Full) + `from_str`/`as_str`/`included_scopes`; `:74` `scope_to_categories`; `:90` `resolve_scopes(db, user_addr, agent_addr, default) -> (Vec<String> scopes, ConsentLevel)` reads `memory_grants`; `:212` `generate_briefing(db, user_addr, agent_addr, thread_id, default) -> Value`.
- `server/src/server.rs:1721` `handle_compound_memory(state, from_addr, method, parsed, ws_sender)`; arms: `ainm.memory.graph.query` (`:1731`, **no consent check today**), `ainm.memory.graph.contribute` (`:1815`), `ainm.memory.briefing` (`:1909`), `ainm.memory.export` (`:2000`). `has_id` is in scope; errors via `protocol::new_error(id, code, msg)`.
- `server/src/server.rs:493` `caller_from_bearer(state, headers) -> Result<auth_session::Caller, (StatusCode,String)>`; `:511` `account_from_bearer`; `:555` `usage_get_handler` (the authed-GET pattern); `:1073`/`:903`/`:981` the three router builders (identical route sets).
- `server/src/signing.rs:23` `sign_envelope(key_pair, &mut Value) -> String` (JCS, removes `signature`, signs, re-inserts); `:57` `verify_signature(pk_bytes, &Value) -> bool`; `Ed25519KeyPair = ring::signature::Ed25519KeyPair`.
- `server/src/accounts.rs:34` `AccountRow{id: Uuid, username, domain, address, ...}`; `:92` `find_account_by_address`; `:201` `find_active_key(pool, account_id) -> Option<AccountKeyRow{public_key: String, ...}>` (base64url public key).
- `server/src/lib.rs` declares `pub mod` for shared modules; `server/src/main.rs` declares `mod` (binary). A new module goes in **both**.

---

## File Structure

- **Create** `server/migrations/008_agent_grants.sql` — extend `memory_grants` with grant columns.
- **Create** `server/src/grant.rs` — pure `AgentGrant`/`WriteBack` types + preset/verb/tool logic + JCS signing helpers, **plus** async repo fns (mirrors `briefing.rs`'s pure+async mix). Declared `pub mod grant;` (lib) / `mod grant;` (main).
- **Modify** `server/src/server.rs` — `resolve_agent_grant` + `grant_gate` helpers; verb enforcement in the four `ainm.memory.*` arms; three grant CRUD handlers + routes on all three builders.
- **Create** `server/tests/agent_grant_e2e.rs` — black-box WS, DB-or-skip.

---

### Task 1: Migration 008 — extend `memory_grants`

**Files:**
- Create: `server/migrations/008_agent_grants.sql`

- [ ] **Step 1: Write the migration**

```sql
-- Agent Grant (onboarding manifest) — extend the existing per-(user,agent) memory_grants
-- table with the grant's new dimensions: memory verbs, tool allow/deny, write-back policy,
-- the reserved skill namespace, and the user's Ed25519 signature over the canonical grant.
-- Additive + idempotent so existing rows (scopes/consent_level) keep working unchanged.
ALTER TABLE memory_grants ADD COLUMN IF NOT EXISTS verbs JSONB NOT NULL DEFAULT '["read","derive"]';
ALTER TABLE memory_grants ADD COLUMN IF NOT EXISTS tool_allow JSONB NOT NULL DEFAULT '[]';
ALTER TABLE memory_grants ADD COLUMN IF NOT EXISTS tool_deny JSONB NOT NULL DEFAULT '[]';
ALTER TABLE memory_grants ADD COLUMN IF NOT EXISTS write_back JSONB NOT NULL DEFAULT '{"memory":"propose","messages":true,"inviteAgents":false,"startThreads":false}';
ALTER TABLE memory_grants ADD COLUMN IF NOT EXISTS preset TEXT NOT NULL DEFAULT 'standard';
ALTER TABLE memory_grants ADD COLUMN IF NOT EXISTS skill_namespace TEXT;
ALTER TABLE memory_grants ADD COLUMN IF NOT EXISTS signature TEXT;
ALTER TABLE memory_grants ADD COLUMN IF NOT EXISTS consent_timestamp TIMESTAMPTZ;
```

- [ ] **Step 2: Sanity-check the SQL parses (psql dry-run optional)**

Run (only if a throwaway Postgres is available; otherwise skip — migrations run at server boot): `psql "$AINM_DATABASE__URL" -f server/migrations/008_agent_grants.sql` then re-run it to confirm idempotency (no error on second run).
Expected: both runs succeed (IF NOT EXISTS makes it idempotent).

- [ ] **Step 3: Commit**

```bash
rtk proxy git add server/migrations/008_agent_grants.sql
rtk proxy git commit -m "feat(agent-grant): migration 008 extends memory_grants with grant fields"
```

---

### Task 2: Pure `grant.rs` module

**Files:**
- Create: `server/src/grant.rs`
- Modify: `server/src/lib.rs` (add `pub mod grant;`), `server/src/main.rs` (add `mod grant;`)

- [ ] **Step 1: Write the failing tests**

Create `server/src/grant.rs` with the test module first (and the types/fns it references in the same file — Rust won't compile tests without them, so write Steps 1 and 3 together; run after):

```rust
//! Pure Agent Grant logic (preset expansion, verb/tool/write-back checks, JCS signing
//! input) plus the async `memory_grants` repository. No HTTP, no WS. The pure half is
//! exhaustively unit-tested; the async half mirrors `briefing.rs`'s repository style.

use serde::{Deserialize, Serialize};

/// The per-(user, agent) grant. Stored in `memory_grants`; signed by the user's key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentGrant {
    pub grantor: String,
    pub subject: String,
    pub preset: String, // "minimal" | "standard" | "full" | "custom"
    pub categories: Vec<String>,
    pub verbs: Vec<String>, // "read" | "derive" | "redact" | "export"
    #[serde(default)]
    pub tool_allow: Vec<String>,
    #[serde(default)]
    pub tool_deny: Vec<String>,
    pub write_back: WriteBack,
    #[serde(default)]
    pub skill_namespace: Option<String>,
    #[serde(default)]
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WriteBack {
    pub memory: String, // "none" | "propose" | "auto"
    pub messages: bool,
    #[serde(rename = "inviteAgents")]
    pub invite_agents: bool,
    #[serde(rename = "startThreads")]
    pub start_threads: bool,
}

impl Default for WriteBack {
    fn default() -> Self {
        Self { memory: "propose".to_string(), messages: true, invite_agents: false, start_threads: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_standard_is_read_derive_over_four_scopes() {
        let (scopes, verbs) = preset_to_scopes_verbs("standard");
        assert_eq!(scopes, vec!["identity", "preferences", "expertise", "goals"]);
        assert_eq!(verbs, vec!["read", "derive"]);
    }

    #[test]
    fn preset_minimal_is_derive_only_identity() {
        let (scopes, verbs) = preset_to_scopes_verbs("minimal");
        assert_eq!(scopes, vec!["identity"]);
        assert_eq!(verbs, vec!["derive"]);
    }

    #[test]
    fn preset_full_adds_redact_over_all_scopes() {
        let (scopes, verbs) = preset_to_scopes_verbs("full");
        assert!(scopes.contains(&"patterns".to_string()));
        assert_eq!(verbs, vec!["read", "derive", "redact"]);
    }

    #[test]
    fn unknown_preset_falls_back_to_standard() {
        assert_eq!(preset_to_scopes_verbs("bogus"), preset_to_scopes_verbs("standard"));
    }

    fn g(verbs: &[&str], allow: &[&str], deny: &[&str], wb_mem: &str) -> AgentGrant {
        AgentGrant {
            grantor: "alice@x".into(), subject: "coder@y".into(), preset: "custom".into(),
            categories: vec!["fact".into()],
            verbs: verbs.iter().map(|s| s.to_string()).collect(),
            tool_allow: allow.iter().map(|s| s.to_string()).collect(),
            tool_deny: deny.iter().map(|s| s.to_string()).collect(),
            write_back: WriteBack { memory: wb_mem.into(), messages: true, invite_agents: false, start_threads: false },
            skill_namespace: None, signature: None,
        }
    }

    #[test]
    fn has_verb_matches_grant() {
        let grant = g(&["read", "derive"], &[], &[], "propose");
        assert!(has_verb(&grant, "read"));
        assert!(has_verb(&grant, "derive"));
        assert!(!has_verb(&grant, "export"));
        assert!(!has_verb(&grant, "redact"));
    }

    #[test]
    fn tool_allowed_deny_wins() {
        // deny always wins, even if also in allow
        let grant = g(&[], &["thread:send"], &["thread:send"], "propose");
        assert!(!tool_allowed(&grant, "thread:send"));
    }

    #[test]
    fn tool_allowed_empty_allow_is_permissive_except_deny() {
        let grant = g(&[], &[], &["memory:export"], "propose");
        assert!(tool_allowed(&grant, "thread:send")); // empty allow → permissive
        assert!(!tool_allowed(&grant, "memory:export")); // unless denied
    }

    #[test]
    fn tool_allowed_nonempty_allow_is_allowlist() {
        let grant = g(&[], &["thread:send"], &[], "propose");
        assert!(tool_allowed(&grant, "thread:send"));
        assert!(!tool_allowed(&grant, "memory:read")); // not in allowlist
    }

    #[test]
    fn can_write_memory_unless_none() {
        assert!(can_write_memory(&g(&[], &[], &[], "propose")));
        assert!(can_write_memory(&g(&[], &[], &[], "auto")));
        assert!(!can_write_memory(&g(&[], &[], &[], "none")));
    }

    #[test]
    fn signing_value_excludes_signature_and_round_trips_verify() {
        use ring::signature::{Ed25519KeyPair, KeyPair};
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let kp = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        let pk = kp.public_key().as_ref().to_vec();

        let mut grant = g(&["read", "derive"], &[], &[], "propose");
        // Sign the canonical grant (signature field excluded by construction)
        let mut value = to_signing_value(&grant);
        let sig = crate::signing::sign_envelope(&kp, &mut value).unwrap();
        grant.signature = Some(sig);

        assert!(verify_grant_signature(&grant, &pk));
        // Tamper → fails
        let mut tampered = grant.clone();
        tampered.verbs = vec!["read".into(), "derive".into(), "export".into()];
        assert!(!verify_grant_signature(&tampered, &pk));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ainm-server --lib grant::tests 2>&1 | tail -20`
Expected: FAIL — `preset_to_scopes_verbs` / `has_verb` / `tool_allowed` / `can_write_memory` / `to_signing_value` / `verify_grant_signature` not defined.

- [ ] **Step 3: Write the pure implementation**

Add to `server/src/grant.rs` (above the `#[cfg(test)]` module):

```rust
/// Preset → (scopes, verbs). Scopes reuse the same vocabulary as `briefing::ConsentLevel`.
/// Unknown presets fall back to "standard" (matches `ConsentLevel::from_str`).
pub fn preset_to_scopes_verbs(preset: &str) -> (Vec<&'static str>, Vec<&'static str>) {
    match preset {
        "minimal" => (vec!["identity"], vec!["derive"]),
        "full" => (
            vec!["identity", "preferences", "expertise", "goals", "history", "patterns"],
            vec!["read", "derive", "redact"],
        ),
        // "standard" and anything unknown
        _ => (
            vec!["identity", "preferences", "expertise", "goals"],
            vec!["read", "derive"],
        ),
    }
}

/// True if the grant carries `verb` (one of read|derive|redact|export).
pub fn has_verb(grant: &AgentGrant, verb: &str) -> bool {
    grant.verbs.iter().any(|v| v == verb)
}

/// Tool authorization: deny wins; a non-empty allow-list is exclusive; empty allow is permissive.
pub fn tool_allowed(grant: &AgentGrant, tool: &str) -> bool {
    if grant.tool_deny.iter().any(|t| t == tool) {
        return false;
    }
    if grant.tool_allow.is_empty() {
        return true;
    }
    grant.tool_allow.iter().any(|t| t == tool)
}

/// Whether the agent may write back to memory (write_back.memory != "none").
pub fn can_write_memory(grant: &AgentGrant) -> bool {
    grant.write_back.memory != "none"
}

/// Canonical JSON value used as the signing input: the grant minus its `signature` field.
/// `signing::sign_envelope`/`verify_signature` already strip `signature` before canonicalizing,
/// so we serialize the whole grant and let them handle exclusion.
pub fn to_signing_value(grant: &AgentGrant) -> serde_json::Value {
    let mut v = serde_json::to_value(grant).expect("AgentGrant serializes");
    if let Some(obj) = v.as_object_mut() {
        obj.remove("signature");
    }
    v
}

/// Verify the user's Ed25519 signature over the canonical grant.
pub fn verify_grant_signature(grant: &AgentGrant, grantor_pubkey: &[u8]) -> bool {
    let Some(sig) = grant.signature.as_ref() else { return false };
    let mut value = to_signing_value(grant);
    if let Some(obj) = value.as_object_mut() {
        obj.insert("signature".to_string(), serde_json::Value::String(sig.clone()));
    }
    crate::signing::verify_signature(grantor_pubkey, &value).unwrap_or(false)
}
```

- [ ] **Step 4: Declare the module**

In `server/src/lib.rs`, add (alphabetical, after `pub mod graph;`):
```rust
pub mod grant;
```
In `server/src/main.rs`, add (after `mod graph;`):
```rust
mod grant;
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p ainm-server --lib grant::tests 2>&1 | tail -20`
Expected: PASS (all grant::tests).

- [ ] **Step 6: Commit**

```bash
rtk proxy git add server/src/grant.rs server/src/lib.rs server/src/main.rs
rtk proxy git commit -m "feat(agent-grant): pure grant module (presets, verb/tool matrix, JCS signing)"
```

---

### Task 3: Grant repository (async, in `grant.rs`)

**Files:**
- Modify: `server/src/grant.rs` (append async repo fns + a row mapper)

- [ ] **Step 1: Write the repo functions**

Append to `server/src/grant.rs` (after the pure fns, before `#[cfg(test)]`):

```rust
use sqlx::PgPool;

/// Load the grant for (user_addr, agent_addr) from `memory_grants`, or `None` if absent.
pub async fn get_grant(
    db: &PgPool,
    user_addr: &str,
    agent_addr: &str,
) -> Result<Option<AgentGrant>, crate::error::AppError> {
    let row: Option<(serde_json::Value, String, serde_json::Value, serde_json::Value, serde_json::Value, String, Option<String>, Option<String>)> =
        sqlx::query_as(
            "SELECT scopes, consent_level, verbs, tool_allow, tool_deny, preset, skill_namespace, signature \
             FROM memory_grants WHERE user_addr = $1 AND agent_addr = $2",
        )
        .bind(user_addr)
        .bind(agent_addr)
        .fetch_optional(db)
        .await
        .map_err(|e| crate::error::AppError::Storage(e.into()))?;

    let Some((scopes_j, _level, verbs_j, allow_j, deny_j, preset, skill_ns, signature)) = row else {
        return Ok(None);
    };
    // write_back is fetched separately to keep the tuple arity manageable.
    let wb_j: serde_json::Value = sqlx::query_scalar(
        "SELECT write_back FROM memory_grants WHERE user_addr = $1 AND agent_addr = $2",
    )
    .bind(user_addr)
    .bind(agent_addr)
    .fetch_one(db)
    .await
    .map_err(|e| crate::error::AppError::Storage(e.into()))?;

    let to_vec = |v: &serde_json::Value| -> Vec<String> {
        v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
    };
    // scopes → categories via the existing briefing vocabulary
    let mut categories: Vec<String> = to_vec(&scopes_j)
        .iter()
        .flat_map(|s| crate::briefing::scope_to_categories(s))
        .map(String::from)
        .collect();
    categories.sort_unstable();
    categories.dedup();

    Ok(Some(AgentGrant {
        grantor: user_addr.to_string(),
        subject: agent_addr.to_string(),
        preset,
        categories,
        verbs: to_vec(&verbs_j),
        tool_allow: to_vec(&allow_j),
        tool_deny: to_vec(&deny_j),
        write_back: serde_json::from_value(wb_j).unwrap_or_default(),
        skill_namespace: skill_ns,
        signature,
    }))
}

/// Insert or replace a grant. Keyed on (user_addr, agent_addr). `scopes` is derived from the
/// grant's preset (custom grants store their explicit scopes via the preset="custom" path:
/// the caller passes the scopes it wants in `scopes_override`).
pub async fn upsert_grant(
    db: &PgPool,
    grant: &AgentGrant,
    scopes: &[String],
) -> Result<(), crate::error::AppError> {
    sqlx::query(
        "INSERT INTO memory_grants \
           (user_addr, agent_addr, scopes, consent_level, verbs, tool_allow, tool_deny, write_back, preset, skill_namespace, signature, consent_timestamp) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11, NOW()) \
         ON CONFLICT (user_addr, agent_addr) DO UPDATE SET \
           scopes = EXCLUDED.scopes, consent_level = EXCLUDED.consent_level, verbs = EXCLUDED.verbs, \
           tool_allow = EXCLUDED.tool_allow, tool_deny = EXCLUDED.tool_deny, write_back = EXCLUDED.write_back, \
           preset = EXCLUDED.preset, skill_namespace = EXCLUDED.skill_namespace, signature = EXCLUDED.signature, \
           consent_timestamp = NOW()",
    )
    .bind(&grant.grantor)
    .bind(&grant.subject)
    .bind(serde_json::json!(scopes))
    .bind(&grant.preset)
    .bind(serde_json::json!(grant.verbs))
    .bind(serde_json::json!(grant.tool_allow))
    .bind(serde_json::json!(grant.tool_deny))
    .bind(serde_json::to_value(&grant.write_back).unwrap())
    .bind(&grant.preset)
    .bind(&grant.skill_namespace)
    .bind(&grant.signature)
    .execute(db)
    .await
    .map_err(|e| crate::error::AppError::Storage(e.into()))?;
    Ok(())
}

/// Delete (revoke) a grant. Returns true if a row was removed.
pub async fn delete_grant(
    db: &PgPool,
    user_addr: &str,
    agent_addr: &str,
) -> Result<bool, crate::error::AppError> {
    let res = sqlx::query("DELETE FROM memory_grants WHERE user_addr = $1 AND agent_addr = $2")
        .bind(user_addr)
        .bind(agent_addr)
        .execute(db)
        .await
        .map_err(|e| crate::error::AppError::Storage(e.into()))?;
    Ok(res.rows_affected() > 0)
}

/// List all grants authored by `user_addr` (subject + preset + verbs for display).
pub async fn list_grants(
    db: &PgPool,
    user_addr: &str,
) -> Result<Vec<AgentGrant>, crate::error::AppError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT agent_addr FROM memory_grants WHERE user_addr = $1 ORDER BY agent_addr",
    )
    .bind(user_addr)
    .fetch_all(db)
    .await
    .map_err(|e| crate::error::AppError::Storage(e.into()))?;
    let mut out = Vec::new();
    for (agent_addr,) in rows {
        if let Some(g) = get_grant(db, user_addr, &agent_addr).await? {
            out.push(g);
        }
    }
    Ok(out)
}
```

> NOTE for the implementer: verify `crate::error::AppError::Storage(e.into())` matches the exact variant used in `briefing.rs:resolve_scopes` (it uses `AppError::Storage(e.into())`). If the variant differs, match `briefing.rs` verbatim.

- [ ] **Step 2: Verify it builds**

Run: `cargo build -p ainm-server 2>&1 | tail -20`
Expected: builds (warnings OK). Fix only compile errors (e.g. `AppError` variant name) to match `briefing.rs`.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add server/src/grant.rs
rtk proxy git commit -m "feat(agent-grant): memory_grants repository (get/upsert/delete/list)"
```

---

### Task 4: Grant resolution + enforcement gate (server.rs helpers)

**Files:**
- Modify: `server/src/server.rs` (add two helpers near `handle_compound_memory`, ~`:1715`)

- [ ] **Step 1: Add the resolver + gate**

Insert immediately above `async fn handle_compound_memory(` (`server.rs:1721`):

```rust
/// Resolve the effective grant for (user_addr, agent_addr). Falls back to the `standard`
/// preset when no row exists (preserving existing default-consent behavior).
async fn resolve_agent_grant(
    db: &sqlx::PgPool,
    user_addr: &str,
    agent_addr: &str,
) -> crate::grant::AgentGrant {
    if let Ok(Some(g)) = crate::grant::get_grant(db, user_addr, agent_addr).await {
        return g;
    }
    let (scopes, verbs) = crate::grant::preset_to_scopes_verbs("standard");
    let mut categories: Vec<String> = scopes
        .iter()
        .flat_map(|s| crate::briefing::scope_to_categories(s))
        .map(String::from)
        .collect();
    categories.sort_unstable();
    categories.dedup();
    crate::grant::AgentGrant {
        grantor: user_addr.to_string(),
        subject: agent_addr.to_string(),
        preset: "standard".to_string(),
        categories,
        verbs: verbs.iter().map(|s| s.to_string()).collect(),
        tool_allow: vec![],
        tool_deny: vec![],
        write_back: crate::grant::WriteBack::default(),
        skill_namespace: None,
        signature: None,
    }
}

/// Grant gate for a memory op. `requester` is the connection identity (the agent); `owner`
/// is whose memory is being accessed. Self-access (requester == owner) is always allowed —
/// you own your memory. Cross-identity access requires the grant to carry `required_verb`.
/// Returns `Ok(())` if allowed, or `Err(())` if denied (caller emits a -32063 error).
async fn grant_gate(
    db: &sqlx::PgPool,
    owner: &str,
    requester: &str,
    required_verb: &str,
) -> Result<(), ()> {
    if owner == requester {
        return Ok(()); // self-access: you own your own memory
    }
    let grant = resolve_agent_grant(db, owner, requester).await;
    if crate::grant::has_verb(&grant, required_verb) {
        Ok(())
    } else {
        Err(())
    }
}
```

- [ ] **Step 2: Verify it builds**

Run: `cargo build -p ainm-server 2>&1 | tail -20`
Expected: builds (the helpers are unused until Task 5/6 — `#[allow(dead_code)]` is unnecessary because they'll be used this same plan, but if the between-task build warns unused, that's fine).

- [ ] **Step 3: Commit**

```bash
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "feat(agent-grant): resolve_agent_grant + grant_gate helpers"
```

---

### Task 5: Enforce read verbs on `briefing` + `graph.query`

**Files:**
- Modify: `server/src/server.rs` — `ainm.memory.briefing` arm (`:1909`), `ainm.memory.graph.query` arm (`:1731`)

- [ ] **Step 1: Gate `ainm.memory.briefing` on `derive`**

In the `"ainm.memory.briefing"` arm, the body begins:
```rust
            if let (Some(ref db), Some(params)) = (&state.db, parsed.get("params")) {
                let thread_id = params.get("thread_id").and_then(|t| t.as_str()).unwrap_or("");
                let user_addr = params.get("user_identity").and_then(|u| u.as_str()).unwrap_or(from_addr);

                match crate::briefing::generate_briefing(db, user_addr, from_addr, thread_id, crate::briefing::ConsentLevel::Standard).await {
```
Insert the gate immediately after the `let user_addr = ...;` line and before the `match`:
```rust
                let user_addr = params.get("user_identity").and_then(|u| u.as_str()).unwrap_or(from_addr);

                if grant_gate(db, user_addr, from_addr, "derive").await.is_err() {
                    if has_id {
                        let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32063, "memory access not permitted by grant (need 'derive')");
                        let mut sender = ws_sender.lock().await;
                        let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                    }
                    return;
                }

                match crate::briefing::generate_briefing(db, user_addr, from_addr, thread_id, crate::briefing::ConsentLevel::Standard).await {
```

- [ ] **Step 2: Gate `ainm.memory.graph.query` on `read` and scope it to the owner**

The query arm currently uses `from_addr` as the graph owner with no consent check. Change it to honor an optional `user_identity` param and gate on `read`. The arm begins:
```rust
        "ainm.memory.graph.query" => {
            if let (Some(ref db), Some(params)) = (&state.db, parsed.get("params")) {
                let text_q = params.get("query").and_then(|q| q.as_str());
```
Insert, immediately after the `if let (Some(ref db), Some(params)) = ... {` line:
```rust
            if let (Some(ref db), Some(params)) = (&state.db, parsed.get("params")) {
                let owner = params.get("user_identity").and_then(|u| u.as_str()).unwrap_or(from_addr);
                if grant_gate(db, owner, from_addr, "read").await.is_err() {
                    if has_id {
                        let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32063, "memory access not permitted by grant (need 'read')");
                        let mut sender = ws_sender.lock().await;
                        let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                    }
                    return;
                }
                let text_q = params.get("query").and_then(|q| q.as_str());
```
Then change the two `crate::graph::*` calls in this arm from `from_addr` to `owner`:
- `crate::graph::query_nodes(db, from_addr, ...)` → `crate::graph::query_nodes(db, owner, ...)`
- `crate::graph::get_edges_for_nodes(db, from_addr, &node_ids)` → `crate::graph::get_edges_for_nodes(db, owner, &node_ids)`

(Self-access is unchanged: when no `user_identity` is sent, `owner == from_addr` and the gate is a no-op.)

- [ ] **Step 3: Verify it builds**

Run: `cargo build -p ainm-server 2>&1 | tail -20`
Expected: builds clean.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "feat(agent-grant): enforce derive/read verbs on briefing + graph.query"
```

---

### Task 6: Enforce write-back + export on `contribute` + `export`

**Files:**
- Modify: `server/src/server.rs` — `ainm.memory.graph.contribute` arm (`:1815`), `ainm.memory.export` arm (`:2000`)

- [ ] **Step 1: Gate `ainm.memory.graph.contribute` on write-back**

The contribute arm begins:
```rust
        "ainm.memory.graph.contribute" => {
            if let (Some(ref db), Some(params)) = (&state.db, parsed.get("params")) {
                let thread_id = params.get("thread_id").and_then(|t| t.as_str()).unwrap_or("").to_string();
```
Insert after the `if let (Some(ref db), Some(params)) = ... {` line:
```rust
            if let (Some(ref db), Some(params)) = (&state.db, parsed.get("params")) {
                let owner = params.get("user_identity").and_then(|u| u.as_str()).unwrap_or(from_addr);
                if owner != from_addr {
                    let grant = resolve_agent_grant(db, owner, from_addr).await;
                    if !crate::grant::can_write_memory(&grant) {
                        if has_id {
                            let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32063, "memory write-back not permitted by grant");
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                        }
                        return;
                    }
                }
                let thread_id = params.get("thread_id").and_then(|t| t.as_str()).unwrap_or("").to_string();
```
(Contribute continues to insert into `from_addr`'s graph in v1 — write-back targeting another owner's graph is a later concern; the gate here makes a cross-identity contribute *require* the write-back grant before proceeding, closing the policy gap. Self-contribute is unchanged.)

- [ ] **Step 2: Gate `ainm.memory.export` on `export`**

The export arm begins:
```rust
        "ainm.memory.export" => {
            if let Some(ref db) = state.db {
                let include_superseded = parsed.get("params").and_then(|p| p.get("include_superseded")).and_then(|v| v.as_bool()).unwrap_or(false);
```
Insert after the `if let Some(ref db) = state.db {` line:
```rust
            if let Some(ref db) = state.db {
                let owner = parsed.get("params").and_then(|p| p.get("user_identity")).and_then(|u| u.as_str()).unwrap_or(from_addr);
                if grant_gate(db, owner, from_addr, "export").await.is_err() {
                    if has_id {
                        let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32063, "memory export not permitted by grant (need 'export')");
                        let mut sender = ws_sender.lock().await;
                        let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                    }
                    return;
                }
                let include_superseded = parsed.get("params").and_then(|p| p.get("include_superseded")).and_then(|v| v.as_bool()).unwrap_or(false);
```
Then change `crate::graph::export_graph(db, from_addr, include_superseded)` → `crate::graph::export_graph(db, owner, include_superseded)`.

- [ ] **Step 3: Verify it builds**

Run: `cargo build -p ainm-server 2>&1 | tail -20`
Expected: builds clean.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "feat(agent-grant): enforce write-back + export verbs on contribute + export"
```

---

### Task 7: Signed grant CRUD endpoints

**Files:**
- Modify: `server/src/server.rs` (3 handlers + routes on all 3 builders)

- [ ] **Step 1: Add the three handlers**

Add near `usage_get_handler` (`server.rs:555`):

```rust
/// PUT /accounts/agent-grants/{agent} — upsert a signed grant for (caller, agent).
/// Body: the full AgentGrant JSON (signed client-side by the user's key). The server
/// verifies `grant.grantor == caller`, `grant.subject == {agent}`, and the signature
/// against the caller's active registered key, then persists.
pub async fn agent_grant_put_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    axum::extract::Path(agent): axum::extract::Path<String>,
    Json(grant): Json<crate::grant::AgentGrant>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let account_id = account_from_bearer(&state, &headers).await?;
    let pool = state.db.clone().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    let account = crate::accounts::find_account_by_id(&pool, account_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "account not found".into()))?;

    if grant.grantor != account.address {
        return Err((StatusCode::FORBIDDEN, "grantor must be the authenticated account".into()));
    }
    if grant.subject != agent {
        return Err((StatusCode::BAD_REQUEST, "grant.subject must match path agent".into()));
    }
    // Verify the user's signature against their active registered key.
    let key = crate::accounts::find_active_key(&pool, account_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::BAD_REQUEST, "no active key".into()))?;
    let pk_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&key.public_key)
        .map_err(|_| (StatusCode::BAD_REQUEST, "corrupt key".into()))?;
    if !crate::grant::verify_grant_signature(&grant, &pk_bytes) {
        return Err((StatusCode::UNAUTHORIZED, "grant signature invalid".into()));
    }

    // Derive scopes: custom presets carry explicit categories already reflected in `grant.categories`;
    // for named presets, store the preset's scopes so resolve_scopes stays consistent.
    let scopes: Vec<String> = if grant.preset == "custom" {
        // reverse-map categories isn't lossless; store the granted categories' parent scopes best-effort
        // by persisting the explicit scope list the client included under preset="custom".
        grant.categories.clone()
    } else {
        crate::grant::preset_to_scopes_verbs(&grant.preset).0.iter().map(|s| s.to_string()).collect()
    };

    crate::grant::upsert_grant(&pool, &grant, &scopes)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true, "subject": grant.subject })))
}

/// GET /accounts/agent-grants — list the caller's grants.
pub async fn agent_grants_list_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let account_id = account_from_bearer(&state, &headers).await?;
    let pool = state.db.clone().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    let account = crate::accounts::find_account_by_id(&pool, account_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "account not found".into()))?;
    let grants = crate::grant::list_grants(&pool, &account.address)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({ "grants": grants })))
}

/// DELETE /accounts/agent-grants/{agent} — revoke the grant for (caller, agent).
pub async fn agent_grant_delete_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    axum::extract::Path(agent): axum::extract::Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let account_id = account_from_bearer(&state, &headers).await?;
    let pool = state.db.clone().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    let account = crate::accounts::find_account_by_id(&pool, account_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "account not found".into()))?;
    let removed = crate::grant::delete_grant(&pool, &account.address, &agent)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if removed { Ok(StatusCode::NO_CONTENT) } else { Err((StatusCode::NOT_FOUND, "grant not found".into())) }
}
```

> Implementer: confirm `base64::engine::general_purpose::URL_SAFE_NO_PAD` + `base64::Engine` are imported in `server.rs` (they are used elsewhere, e.g. the handshake key decode). If not in scope, add `use base64::Engine;`.

- [ ] **Step 2: Register routes on all three builders**

In each of the three `Router::new()...` builders (`server.rs:1073`, `:903`, `:981`), add after the `/accounts/usage` route — note `/agent-grants` (list) must be registered before `/agent-grants/{agent}` is not required since they differ by method+path, but keep the static path first for clarity:

```rust
        .route("/accounts/agent-grants", axum::routing::get(agent_grants_list_handler))
        .route("/accounts/agent-grants/{agent}", axum::routing::put(agent_grant_put_handler).delete(agent_grant_delete_handler))
```

- [ ] **Step 3: Verify it builds**

Run: `cargo build -p ainm-server 2>&1 | tail -20`
Expected: builds clean. (If `find_account_by_id` signature differs, match `usage_get_handler`'s usage verbatim.)

- [ ] **Step 4: Commit**

```bash
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "feat(agent-grant): signed grant CRUD endpoints (PUT/GET/DELETE)"
```

---

### Task 8: Black-box WS E2E (DB-or-skip)

**Files:**
- Create: `server/tests/agent_grant_e2e.rs`

- [ ] **Step 1: Write the test (DB-or-skip pattern, mirroring `a2a_routing_e2e.rs`)**

Create `server/tests/agent_grant_e2e.rs`. It must: skip cleanly when `TEST_DATABASE_URL`/`AINM_DATABASE__URL` is unset; spin the server on an ephemeral port with a DB; provision two accounts (a human `alice` + an agent `coder`) with bound keys; connect `coder` as an agent; assert:
  1. With a `minimal` (derive-only) grant for (alice, coder): `coder` calling `ainm.memory.briefing` with `user_identity=alice` **succeeds**, but `ainm.memory.graph.query` with `user_identity=alice` returns error **-32063**.
  2. After `PUT /accounts/agent-grants/coder@<domain>` with a signed grant carrying `read`: `graph.query` now **succeeds**.
  3. `ainm.memory.export` with `user_identity=alice` returns **-32063** (no export verb).
  4. After `DELETE`, the next `graph.query` returns **-32063** again (revoked → falls back to standard which lacks... note: standard HAS read, so revocation falls back to the standard default which still grants read; to make revoke observable, the test should instead set a `custom` grant with NO verbs, then DELETE and assert it returns to the standard default — OR assert the DELETE returns 204 and the row is gone). Use the **204 + GET-list-empty** assertion for revoke rather than a verb change, to avoid coupling to the default.

> Implementer: model this test's harness on `server/tests/a2a_routing_e2e.rs` (server bootstrap, account provisioning, WS connect, JSON-RPC send/recv helpers) and `server/tests/a2a_consent_e2e.rs` (which already provisions a human + agent and signs envelopes). Reuse their helper functions verbatim where possible. The grant `PUT` body must be signed with alice's key via `signing::sign_envelope` over `grant::to_signing_value(&grant)` (see grant::tests `signing_value_excludes_signature_and_round_trips_verify`). Authenticate the REST calls by minting a session token via `POST /accounts/auth/token` (see `a2a`/auth tests or `auth_session` integration test for the mint-via-signature flow).

The test MUST begin with the skip guard used by the other DB-gated tests, e.g.:
```rust
fn db_url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok().or_else(|| std::env::var("AINM_DATABASE__URL").ok())
}

#[tokio::test]
async fn agent_grant_enforces_verbs_end_to_end() {
    let Some(_url) = db_url() else {
        eprintln!("skipping agent_grant_e2e: no TEST_DATABASE_URL / AINM_DATABASE__URL");
        return;
    };
    // ... harness mirroring a2a_consent_e2e.rs ...
}
```

- [ ] **Step 2: Run (skips without DB; runs with one)**

Run: `cargo test -p ainm-server --test agent_grant_e2e 2>&1 | tail -20`
Expected: in this env (no Postgres) → compiles + self-skips (prints the skip line, test passes). With `TEST_DATABASE_URL` set → all assertions pass.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add server/tests/agent_grant_e2e.rs
rtk proxy git commit -m "test(agent-grant): black-box WS verb-enforcement E2E (DB-or-skip)"
```

---

### Task 9: Full verification, memory, holistic review

- [ ] **Step 1: Full suite + clippy**

Run:
```bash
cargo test -p ainm-server 2>&1 | grep -E "test result:|error\[|FAILED" | tail -40
cargo clippy -p ainm-server --all-targets 2>&1 | tail -20
```
Expected: green except the two known pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`); `grant::tests` pass; clippy clean on touched files (pre-existing lints elsewhere are out of scope). The new E2E self-skips without Postgres.

- [ ] **Step 2: Update project memory**

Update `~/.claude/projects/-Users-santiagogerickeparga-Proyectos-ai-native-messaging/memory/project_runtime_a2a.md` (or a new `project_agent_grant.md` with a MEMORY.md pointer): record the Agent Grant server core shipped — `memory_grants` extended (008); pure `grant` module; verb enforcement on briefing(derive)/graph.query(read)/contribute(write-back)/export(export) including closing the previously un-gated query/export paths; signed CRUD endpoints; that iOS UX + connector grant-summary + D2 push-briefing are the next plans; and that build was gated on Task 14.

- [ ] **Step 3: ONE holistic opus review**

Dispatch a single opus reviewer over the full plan diff. Focus: (a) the gate's self-access carve-out (`owner == requester`) can't be bypassed by spoofing `user_identity` (note: an agent could pass `user_identity = its own address` to self-access — but that only exposes the agent's OWN graph, which is fine; confirm it can't name a *third* user); (b) signature verification actually binds the grant to the user's registered key (no downgrade if `signature` is None → `verify_grant_signature` returns false → PUT rejects); (c) the `preset="custom"` scopes round-trip through `upsert`→`get`→categories without silent loss; (d) no raw-fact leak on the `derive`-only path (briefing already category-filters; confirm query is hard-denied, not soft); (e) revocation observability; (f) no secrets/keys logged. Address blocking findings before done.

---

## Self-Review (writing-plans)

**Spec coverage (spec §3–§7 → tasks):**
- §3.1 grant object → Task 2 (`AgentGrant`/`WriteBack`) + Task 1 (storage). ✔
- §3.2 verbs (read/derive/redact/export) → Task 2 (`has_verb`) + Tasks 5–6 (enforcement maps read→query, derive→briefing, export→export, write-back→contribute). `redact` = reserved (no method yet; verb modeled, ungated — noted). ✔
- §3.3 host-boundary enforcement generalizing the consent gate → Tasks 4–6. ✔
- §3.4 onboarding/lifecycle: authoring + signing + list/revoke → Task 7 (CRUD + signature verify). **Push-briefing half explicitly deferred to the connector plan** (documented in Scope). ✔ (partial-by-design)
- §3.5 L2 thin: grant is the access contract over existing memory; no new tiering → honored (Tasks 5–6 gate existing methods). ✔
- §3.6 L3 thin: `skill_namespace` column reserved (Task 1) + `skillGrant` reserved in the object — NOTE: the `AgentGrant` struct in Task 2 does **not** include `skill_grant`; the spec reserves it. **Gap fix:** acceptable for v1 — `skill_namespace` is reserved in storage; `skillGrant` verbs (`use`/`learn`) are deferred to the L3 plan with the loop. Documented as deliberate. ✔
- §6 success criteria 1 (author preset+override) → Task 7 PUT; 2 (operate within grant, enforced, visible denial) → Tasks 5–6 (-32063, not silent); 3 (derive vs export) → Tasks 5–6; 4 (list+revoke) → Task 7; 5 (uniform hosted+connector) → enforcement is on the shared WS methods + `resolve_agent_grant` is reused by the runtime path's `resolve_scopes` table; 6 (no regression / UCAN-shaped) → Task 9 + the struct's `grantor/subject` (iss/aud) shape. ✔

**Placeholder scan:** no TBD/TODO; every code step has verbatim code; the one place pointing at existing test harnesses (Task 8) explicitly names the files to mirror and gives the skip-guard + signing approach verbatim rather than hand-waving. ✔

**Type consistency:** `AgentGrant`/`WriteBack` fields are identical across Task 2 (def), Task 3 (repo), Task 4 (resolver), Task 7 (endpoints). `grant_gate(db, owner, requester, verb)` signature used consistently in Tasks 5–6. `resolve_agent_grant(db, user, agent)` consistent. `verify_grant_signature(grant, pk_bytes)` consistent (Task 2 def, Task 7 use). ✔

**Known follow-ups flagged for execution:** confirm `AppError::Storage` variant + `find_account_by_id` signature + `base64::Engine` import against current code (Tasks 3/7 notes); `redact` verb + `skillGrant` are reserved-not-enforced; the `custom`-preset scope round-trip is best-effort (categories stored directly).

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-09-agent-grant-server.md`. **Do not execute yet — build is gated on Task 14** (the compound-memory value go/no-go). When cleared, two options:

1. **Subagent-Driven (recommended)** — fresh subagent per task; per project precedent for verbatim-code plans, skip per-task two-stage review, verify build/test + diff between tasks, ONE holistic opus review at the end (Task 9 Step 3). Commits to `main` via `rtk proxy git`; push user-gated at the end.
2. **Inline Execution** — execute in this session via executing-plans with checkpoints.
