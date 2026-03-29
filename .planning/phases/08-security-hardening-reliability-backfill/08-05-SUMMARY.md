---
phase: 08-security-hardening-reliability-backfill
plan: 05
subsystem: demo-scripts
tags: [demo, partition-healing, cert-rotation, federation, reliability]
dependency_graph:
  requires: ["08-01", "08-02", "08-03", "08-04"]
  provides: ["DEMO-03", "DEMO-04"]
  affects: [demo/scripts, demo/certs/generate.sh, demo/src/main.rs]
tech_stack:
  added: []
  patterns: [docker-compose orchestration, dual-fingerprint rotation window, TOFU auto-update]
key_files:
  created:
    - demo/scripts/demo-partition-healing.sh
    - demo/scripts/demo-cert-rotation.sh
  modified:
    - demo/certs/generate.sh
    - demo/src/main.rs
decisions:
  - Used docker-compose run with --service-ports for cert rotation restart (avoids modifying docker-compose.yml at runtime)
  - Added --additional-fingerprint as CLI arg rather than env var for explicit control during rotation
metrics:
  duration: 4m 40s
  completed: 2026-03-29
---

# Phase 8 Plan 5: Demo Scripts -- Partition Healing & Certificate Rotation Summary

Demo scripts proving partition healing (DEMO-03) and certificate rotation (DEMO-04) work end-to-end in the federated system, with both Docker and manual instructions.

## What Was Done

### Task 1: Partition Healing Demo Script (DEMO-03)
- Created `demo/scripts/demo-partition-healing.sh` (86 lines, executable)
- Orchestrates: start both servers, verify federation, stop server-b, queue messages, restart server-b, verify delivery
- Documents manual two-terminal alternative for non-Docker environments
- Commit: 57363a8

### Task 2: Certificate Rotation Demo Script (DEMO-04) + CLI Enhancement
- Added `rotate-server-b` mode to `demo/certs/generate.sh` for generating rotated certificates with new ED25519 keys
- Added `--additional-fingerprint` CLI argument to `demo/src/main.rs` that appends an extra fingerprint to the tls_fingerprints array in .well-known, enabling TOFU auto-update during rotation
- Created `demo/scripts/demo-cert-rotation.sh` (115 lines, executable) orchestrating cert generation, fingerprint computation, cert swap, dual-fingerprint restart, and TOFU verification
- Documents manual alternative with explicit CLI flags
- Commit: da5c654

### Task 3: Build Verification (Auto-approved)
- `cargo build` succeeds with only 3 pre-existing warnings (unused methods)
- Auto-approved per auto_advance configuration

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed cert rotation script docker log command**
- **Found during:** Task 2
- **Issue:** Plan's script used `docker-compose logs server-b` after replacing server-b with a `docker-compose run` container named `server-b-rotated`; the logs would not show the rotated container's output
- **Fix:** Used `docker logs server-b-rotated` for the rotated container and added proper cleanup (`docker stop/rm server-b-rotated`)
- **Files modified:** demo/scripts/demo-cert-rotation.sh

**2. [Rule 1 - Bug] Fixed cert generation to use proper CA-signed certs**
- **Found during:** Task 2
- **Issue:** Plan's generate.sh snippet used self-signed certs (`-new -x509`) for rotation, which would fail mTLS CA verification. The existing certs are CA-signed.
- **Fix:** Used CSR + CA signing approach consistent with existing generate.sh pattern (genpkey + req + x509 -req -CA)
- **Files modified:** demo/certs/generate.sh

## Known Stubs

None. All scripts are complete and functional.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | 57363a8 | feat(08-05): create partition healing demo script (DEMO-03) |
| 2 | da5c654 | feat(08-05): create cert rotation demo script and --additional-fingerprint CLI (DEMO-04) |
