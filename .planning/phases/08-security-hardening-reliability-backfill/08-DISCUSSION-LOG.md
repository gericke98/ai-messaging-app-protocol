# Phase 8: Security Hardening, Reliability & Backfill - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-28
**Phase:** 08-security-hardening-reliability-backfill
**Areas discussed:** Replay protection strategy, Certificate rotation model, Persistent queue & partition behavior, Backfill endpoint design

---

## Replay Protection Strategy

### Replay rejection window

| Option | Description | Selected |
|--------|-------------|----------|
| 5-minute window | Reject payloads older than 5 minutes. Tolerates minor clock drift. relay_id dedup cache expires after 5 min. | ✓ |
| Configurable window | Default 5 minutes but allow server operators to tune it via config parameter. | |
| Strict 60-second window | Tighter security but requires good NTP sync between servers. | |

**User's choice:** 5-minute window (Recommended)
**Notes:** Standard for federation protocols.

### TOFU key pinning model

| Option | Description | Selected |
|--------|-------------|----------|
| Pin on first contact | SSH-style. First contact pins TLS fingerprint. Future connections must match. Warning on mismatch, connection rejected. | ✓ |
| Pin with admin override | Same as above but add admin endpoint to manually update pinned fingerprints. | |
| Pin with expiry | Pins expire after N days, forcing re-verification. | |

**User's choice:** Pin on first contact (Recommended)
**Notes:** Simple, proven model.

### Dedup cache and TOFU storage

| Option | Description | Selected |
|--------|-------------|----------|
| redb for both | Persist dedup cache and TOFU pins in redb. Survives restarts. One storage engine for all Phase 8 persistence. | ✓ |
| redb for TOFU, in-memory for dedup | TOFU pins must survive restarts. Dedup cache is ephemeral. | |
| Both in-memory | Simplest but weakest security guarantees. | |

**User's choice:** redb for both (Recommended)
**Notes:** None.

### Relay ID format

| Option | Description | Selected |
|--------|-------------|----------|
| UUIDv7 | Sender generates unique relay_id per payload. Time-ordered for efficient cleanup. | ✓ |
| SHA-256 of canonical payload | Deterministic. Catches content-level duplicates. More expensive. | |
| UUIDv7 + content hash | Both for dedup and integrity. Belt and suspenders. | |

**User's choice:** UUIDv7 (Recommended)
**Notes:** None.

---

## Certificate Rotation Model

### Fingerprint overlap window

| Option | Description | Selected |
|--------|-------------|----------|
| Array with max 2 entries | tls_fingerprints: [new, old]. Receiving server accepts either. Old removed after rotation completes. | ✓ |
| Array with max 3 entries | Allow 3 fingerprints for complex multi-stage rotations. | |
| Single fingerprint + rotation endpoint | Keep single fingerprint, add ainm.rotate-cert JSON-RPC method. | |

**User's choice:** Array with max 2 entries (Recommended)
**Notes:** Simple, covers the common case.

### Backwards compatibility

| Option | Description | Selected |
|--------|-------------|----------|
| Replace with array | Change tls_fingerprint to tls_fingerprints (array). Clean break. | ✓ |
| Support both fields | Add array alongside existing singular field. | |
| Keep singular, add rotation_fingerprint | Add second optional field for new cert during rotation. | |

**User's choice:** Replace with array (Recommended)
**Notes:** Phase 7 was demo-only, no deployed servers to break.

### TOFU + rotation interaction

| Option | Description | Selected |
|--------|-------------|----------|
| Auto-update pin | If new fingerprint appears alongside pinned one in array, auto-update pin. Known fingerprint vouches for new one. Log rotation. | ✓ |
| Require manual approval | Flag rotation, reject until admin approves. | |
| Accept both, pin newest | Accept either fingerprint, pin the newest (first in array). | |

**User's choice:** Auto-update pin (Recommended)
**Notes:** None.

---

## Persistent Queue & Partition Behavior

### Queue size bound

| Option | Description | Selected |
|--------|-------------|----------|
| Count-based: 10,000 messages per destination | Simple, predictable. ~10MB per destination at ~1KB/msg. | ✓ |
| Size-based: 50MB per destination | Caps disk usage directly. Harder to predict message counts. | |
| Count + age: 10K messages or 7 days | Dual bound, evict whichever triggers first. | |

**User's choice:** Count-based: 10,000 messages per destination (Recommended)
**Notes:** None.

### Queue full behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Drop oldest, log warning | FIFO eviction. Log warning with dropped count. Sender gets success. | ✓ |
| Reject with backpressure | Return error to origin server. Risks cascade failures. | |
| Drop oldest silently | No logging. Simplest but harder to debug. | |

**User's choice:** Drop oldest, log warning (Recommended)
**Notes:** None.

### Partition healing

| Option | Description | Selected |
|--------|-------------|----------|
| Flush queue in order | Drain persistent queue FIFO on reconnection. Reset backoff. | ✓ |
| Flush + request backfill | Drain queue AND request backfill from recovered server. | |
| Negotiate sync point | Exchange last-seen sequence numbers, replay only missed. | |

**User's choice:** Flush queue in order (Recommended)
**Notes:** Keeps queue and backfill features decoupled.

### Retention cleanup

| Option | Description | Selected |
|--------|-------------|----------|
| Periodic cleanup every hour | Background task scans redb hourly, removes entries older than 7 days. | ✓ |
| Lazy cleanup on write | Check/evict old entries only on write. Bursty. | |
| On-demand cleanup | Only clean up when queue reaches size bound. | |

**User's choice:** Periodic cleanup every hour (Recommended)
**Notes:** UUIDv7 keys make age-based cleanup efficient.

---

## Backfill Endpoint Design

### Method and endpoint

| Option | Description | Selected |
|--------|-------------|----------|
| ainm.backfill on relay endpoint | Reuse existing /ainm/v1/relay POST with JSON-RPC method "ainm.backfill". Same mTLS auth. | ✓ |
| Separate /ainm/v1/backfill endpoint | Dedicated GET endpoint. Clearer REST semantics. | |
| ainm.backfill on new endpoint | New POST endpoint, still JSON-RPC. Separates traffic. | |

**User's choice:** ainm.backfill on relay endpoint (Recommended)
**Notes:** Keeps one federation endpoint.

### Pagination approach

| Option | Description | Selected |
|--------|-------------|----------|
| Cursor-based with sequence numbers | after_sequence + limit (default 50, max 200). Response: messages + has_more + next_cursor. | ✓ |
| Offset-based | offset + limit. Simple but fragile during concurrent writes. | |
| Time-based | after_timestamp + limit. Natural but timestamps can collide. | |

**User's choice:** Cursor-based with sequence numbers (Recommended)
**Notes:** Sequence numbers from thread registry make natural cursors.

### Authorization model

| Option | Description | Selected |
|--------|-------------|----------|
| Check thread registry | Origin server checks ThreadRegistry — requesting server must have participant in thread. Already tracked. | ✓ |
| Require participant identity proof | Signed assertion from participant authorizing backfill. | |
| Server-level ACL | Per-thread access control list of allowed servers. | |

**User's choice:** Check thread registry (Recommended)
**Notes:** No new state needed.

### Visibility levels

| Option | Description | Selected |
|--------|-------------|----------|
| Post-join only, no pre-join option | Return only messages after earliest participant joined. Strongest privacy. Pre-join deferred to v2. | ✓ |
| Post-join default, flag for pre-join | Default post-join, origin can grant pre-join via permission flag. | |
| Full history always | Return all messages. Weakest privacy. | |

**User's choice:** Post-join only, no pre-join option (Recommended)
**Notes:** Simplest, strongest privacy default.

---

## Claude's Discretion

- redb table/key design for offline queue, dedup cache, and TOFU pin store
- Exact cleanup task scheduling mechanism
- Internal module organization for persistence layer
- Spec prose wording for new transport.md sections
- Demo scenario scripts for DEMO-03 and DEMO-04

## Deferred Ideas

None — discussion stayed within phase scope
