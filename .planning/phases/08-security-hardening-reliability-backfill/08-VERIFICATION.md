---
phase: 08-security-hardening-reliability-backfill
verified: 2026-03-29T08:09:18Z
status: human_needed
score: 5/5 must-haves verified
gaps: []
resolved_gaps:
  - truth: "Replay rejection error code"
    resolution: "Changed all 6 occurrences of -32012 to -32026 in federation.rs (commit 40d3efc)"
human_verification:
  - test: "Partition healing end-to-end"
    expected: "Stop server-b, send messages from server-a (they queue), restart server-b, verify queued messages appear in server-b logs as delivered"
    why_human: "Requires running two server instances and observing delivery logs — cannot verify live network behavior programmatically"
  - test: "Certificate rotation end-to-end"
    expected: "Rotate server-b cert, restart with --additional-fingerprint=<old_fp>, federation from server-a continues without error"
    why_human: "Requires live mTLS handshake execution and TOFU pin auto-update verification across real TLS connections"
---

# Phase 8: Security Hardening, Reliability & Backfill Verification Report

**Phase Goal:** Federation is hardened against replay attacks and certificate lifecycle issues, survives server outages with persistent queues, and supports selective thread history backfill with authorization
**Verified:** 2026-03-29T08:09:18Z
**Status:** human_needed (gap fixed, 2 items need live testing)
**Re-verification:** Yes — gap fixed inline (error code -32012 → -32026)

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | A replayed relay payload older than 5 minutes is rejected, and a captured relay cannot be replayed after the deduplication window expires (relay_timestamp + relay_id enforcement) | PARTIAL | Timestamp freshness check exists and dedup via `check_and_insert_relay_id` is wired, but returns error code -32012 instead of spec-defined -32026 |
| 2 | When a server's TLS certificate is rotated, federation continues without downtime because the tls_fingerprints array supports rotation overlap | VERIFIED | `verify_tofu` in identity.rs checks both old+new fingerprints in published array; `--additional-fingerprint` CLI arg adds extra fingerprint to .well-known; docker generate.sh has `rotate-server-b` mode |
| 3 | When a remote server goes offline, messages are persisted to disk and automatically delivered with exponential backoff when it comes back — surviving server restarts | VERIFIED | VecDeque replaced with redb `enqueue_offline`/`drain_offline`; backoff in-memory (resets on restart, correct per D-14); docker-compose named volumes `server-a-data`/`server-b-data` persist redb across restarts |
| 4 | A remote server can request thread history via backfill, receiving only post-join messages by default, with pagination for large threads and authorization that rejects servers without a legitimate participant | VERIFIED | `handle_backfill` dispatched from relay_handler on method=`ainm.backfill`; `is_participant` check returns -32025; `effective_after = after_sequence.max(join_seq)` enforces post-join visibility; `has_more`/`next_cursor` pagination returned |
| 5 | Demo scenarios prove partition healing (stop server, queue, restart, verify delivery) and certificate rotation (rotate cert, verify federation continues) | VERIFIED | `demo-partition-healing.sh` (86 lines, docker-compose stop/start) and `demo-cert-rotation.sh` (138 lines, cert generation + dual-fingerprint restart) both exist and are executable |

**Score:** 4/5 truths verified (Truth 1 is partial due to error code mismatch)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `demo/src/persistence.rs` | redb persistence layer with 4 tables | VERIFIED | All 9 methods present: `check_and_insert_relay_id`, `get_tofu_pin`, `set_tofu_pin`, `enqueue_offline`, `drain_offline`, `queue_len`, `evict_oldest`, `cleanup_expired_queue`, `store_thread_message`, `get_thread_messages`, `pending_destinations` |
| `demo/src/federation.rs` | Replay protection in relay_handler; ainm.backfill handler; join_sequences in ThreadInfo | VERIFIED (with gap) | Replay check at lines 231-297; `handle_backfill` at line 469; `join_sequences: HashMap<String, u64>` at line 32; error code -32012 used instead of -32026 |
| `demo/src/identity.rs` | `verify_tofu` function with TOFU pinning and rotation support | VERIFIED | `pub fn verify_tofu` at line 190; calls `get_tofu_pin`, `set_tofu_pin`, checks both fingerprints in `published.contains` |
| `demo/src/relay.rs` | RelaySender with redb-backed persistent queue; relay_timestamp + relay_id in outbound params | VERIFIED | `enqueue_offline`, `drain_offline`, `evict_oldest` at lines 100-145; `relay_timestamp` and `relay_id` in params at lines 161-162 |
| `demo/src/main.rs` | TOFU verification in TLS loop; hourly cleanup task; --additional-fingerprint CLI | VERIFIED | `identity::verify_tofu` called at line 366; `cleanup_expired_dedup`/`cleanup_expired_queue` at lines 442-449; `--additional-fingerprint` at line 61 |
| `demo/scripts/demo-partition-healing.sh` | Executable partition healing demo (>20 lines) | VERIFIED | 86 lines, uses docker-compose stop/start, documents manual alternative |
| `demo/scripts/demo-cert-rotation.sh` | Executable cert rotation demo (>20 lines) | VERIFIED | 138 lines, generates rotated certs, uses --additional-fingerprint, documents manual alternative |
| `demo/certs/generate.sh` | rotate-server-b mode for generating rotated certs | VERIFIED | `rotate-server-b` case at line 36; uses CSR + CA-signed approach consistent with existing certs |
| `demo/docker-compose.yml` | Named volumes for persistence across restarts | VERIFIED | `server-a-data` and `server-b-data` named volumes; `--data-dir /data` passed to both servers |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `demo/src/relay.rs` | `demo/src/persistence.rs` | `enqueue_offline`, `drain_offline`, `evict_oldest` | WIRED | Lines 102, 114, 145 in relay.rs call persistence methods directly |
| `demo/src/federation.rs` | `demo/src/persistence.rs` | `check_and_insert_relay_id` for dedup | WIRED | Line 284: `state.persistence.check_and_insert_relay_id` |
| `demo/src/main.rs` | `demo/src/persistence.rs` | `get_tofu_pin`/`set_tofu_pin` via `verify_tofu` | WIRED | persistence_for_tls cloned at line 345, passed to `identity::verify_tofu` at line 366 |
| `demo/src/main.rs` | `demo/src/persistence.rs` | hourly cleanup task calling `cleanup_expired_dedup`/`cleanup_expired_queue` | WIRED | Lines 442-449 in tokio interval task |
| `demo/src/federation.rs` | `demo/src/persistence.rs` | `store_thread_message` at origin; `get_thread_messages` for backfill | WIRED | Line 426: `store_thread_message` in relay fan-out; line 567: `get_thread_messages` in `handle_backfill` |
| `demo/src/federation.rs` | `demo/src/federation.rs` | `handle_backfill` dispatched from `relay_handler` on method=`ainm.backfill` | WIRED | Lines 195-197: match dispatch in relay_handler |
| `demo/scripts/demo-partition-healing.sh` | `demo/docker-compose.yml` | docker-compose stop/start commands | WIRED | Lines 49, 63: `docker-compose stop server-b`, `docker-compose start server-b` |
| `demo/scripts/demo-cert-rotation.sh` | `demo/certs/generate.sh` | Certificate generation for rotation | WIRED | Line 58: `bash generate.sh rotate-server-b` |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|--------------|--------|--------------------|--------|
| `federation.rs` relay_handler | `relay_id`, `relay_timestamp` | outbound relay.rs params (lines 161-162) | Yes — uuid::Uuid::now_v7() and chrono::Utc::now() | FLOWING |
| `federation.rs` handle_backfill | messages from `get_thread_messages` | redb thread_messages table via persistence.rs | Yes — DB query with range scan on thread_id:sequence keys | FLOWING |
| `relay.rs` flush_destination | messages from `drain_offline` | redb offline_queue table via persistence.rs | Yes — DB read with prefix scan on destination keys | FLOWING |
| `identity.rs` verify_tofu | pinned fingerprint from `get_tofu_pin` | redb tofu_pins table via persistence.rs | Yes — DB key-value read | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Build succeeds | `cargo build` | `Finished dev profile [unoptimized + debuginfo]` with 3 pre-existing warnings | PASS |
| relay_timestamp in relay params | `grep "relay_timestamp" demo/src/relay.rs` | Found at line 161 in flush_destination JSON params | PASS |
| replay check before signature verify | Pattern: check_and_insert_relay_id before verify_relay_signature | check_and_insert at line 284, verify_relay_signature at line 305+ | PASS |
| TOFU function exists and is called | `grep "verify_tofu" demo/src/identity.rs demo/src/main.rs` | Defined in identity.rs line 190, called in main.rs line 366 | PASS |
| ainm.backfill dispatched | `grep "ainm.backfill" demo/src/federation.rs` | Method dispatch at line 195 | PASS |
| All commit hashes valid | `git log --oneline` on all 9 commits | All 9 commits verified in git log | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|---------|
| FAUT-04 | 08-02 | Server implements TOFU key pinning for remote server certificates | SATISFIED | `verify_tofu` in identity.rs; called in main.rs TLS accept loop |
| FAUT-05 | 08-02 | Relay messages include relay_timestamp and relay_id to prevent replay attacks | SATISFIED | relay.rs lines 161-162; federation.rs validates at lines 231-297 |
| FAUT-06 | 08-02 | Server supports multiple TLS fingerprints (tls_fingerprints array) for cert rotation | SATISFIED | `tls_fingerprints: Vec<String>` in identity.rs; rotation logic in verify_tofu |
| FREL-01 | 08-03 | Server persists offline relay queue to disk (survives server restart) | SATISFIED | redb-backed enqueue_offline/drain_offline in relay.rs; docker volumes in docker-compose.yml |
| FREL-02 | 08-03 | Offline queue retries with exponential backoff when remote server becomes reachable | SATISFIED | BackoffState with 1s base, 60s max in relay.rs; pending_destinations loop in flush task |
| FREL-03 | 08-03 | Offline queue has bounded size and 7-day retention policy | SATISFIED | 10_000 bound with evict_oldest in relay.rs line 113; Duration::days(7) cleanup in main.rs line 448 |
| FREL-04 | 08-03 | Server queues messages during origin-server unavailability and replays on reconnection | SATISFIED | Messages enqueued on connection failure (lines 211-213, 223-225 in relay.rs); drained in flush_destination |
| BKFL-01 | 08-04 | Remote server can request selective thread history via backfill endpoint | SATISFIED | `ainm.backfill` method dispatched in relay_handler; handle_backfill implemented |
| BKFL-02 | 08-04 | Backfill respects visibility levels (post-join messages only by default) | SATISFIED | `effective_after = after_sequence.max(join_seq)` at line 549 in federation.rs |
| BKFL-03 | 08-04 | Backfill requires authorization (requesting server must be a current participant) | SATISFIED | `is_participant` check with -32025 error at lines 525-541 |
| BKFL-04 | 08-04 | Backfill supports pagination for large thread histories | SATISFIED | `has_more` and `next_cursor` returned at lines 580-586 |
| DEMO-03 | 08-05 | Demo tests partition healing (stop one server, queue messages, restart, verify delivery) | SATISFIED | demo-partition-healing.sh (86 lines); uses docker-compose stop/start |
| DEMO-04 | 08-05 | Demo tests certificate rotation (rotate cert, verify federation continues) | SATISFIED | demo-cert-rotation.sh (138 lines); --additional-fingerprint CLI arg added |

All 13 requirement IDs are accounted for and satisfied. No orphaned requirements found.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `demo/src/federation.rs` | 238, 247, 260, 267, 276, 294 | Returns -32012 for all replay rejections, but spec (transport.md) defines -32012 as "Expired key" and -32026 as "ReplayDetected" | Warning | Error code mismatch between implementation and spec. Callers checking for -32026 per spec will not detect replay rejections. Third-party implementers following the spec will send wrong error codes. |

### Human Verification Required

#### 1. Partition Healing End-to-End

**Test:** Run `demo/scripts/demo-partition-healing.sh` (requires Docker). Verify server-b receives queued messages after restart.
**Expected:** After `docker-compose start server-b`, server-b logs show messages enqueued during its downtime are delivered; server-a logs show "relay Successfully" to server-b.
**Why human:** Requires live Docker containers, network connections, and log observation — cannot verify message delivery without running processes.

#### 2. Certificate Rotation End-to-End

**Test:** Run `demo/scripts/demo-cert-rotation.sh` (requires Docker). Verify server-a continues federating with server-b after cert rotation.
**Expected:** After rotation, TOFU pin auto-updates (logs show "[tofu] Cert rotation for server-b"), and federation relay from server-a succeeds.
**Why human:** Requires live mTLS handshake execution, TOFU pin state transitions in redb, and log verification.

### Gaps Summary

One gap was identified: the replay rejection error code in `demo/src/federation.rs` does not match the protocol specification.

**Gap: Error code -32012 used for replay rejection instead of -32026**

The spec (`transport.md`) defines two distinct error codes:
- `-32012`: "Expired key" — for revoked/expired signing keys
- `-32026`: "Replay detected" — for stale timestamps or duplicate relay_id

The implementation (federation.rs) uses `-32012` for all six replay rejection paths. This was noted as a fix in 08-01 SUMMARY (the spec was updated to use -32026), but the implementation in 08-02 was written using the original plan's -32012 value and was not corrected to match the updated spec.

This is a spec-compliance gap: any third-party implementation following the spec to handle `-32026` for replay detection will be broken against this server. The functional behavior (rejection) is correct — only the error code is wrong.

**Fix:** Change all 6 instances of `-32012` in the replay protection block of `federation.rs` (lines 238, 247, 260, 267, 276, 294) to `-32026`.

---

_Verified: 2026-03-29T08:09:18Z_
_Verifier: Claude (gsd-verifier)_
