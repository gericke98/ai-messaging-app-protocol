---
phase: 07-federation-relay-core
plan: 02
subsystem: infra
tags: [rustls, mtls, tls, certificates, ed25519, federation, reqwest]

requires:
  - phase: 01-protocol-kernel
    provides: identity module with Ed25519 keypairs, ring crypto primitives

provides:
  - mTLS ServerConfig builder for inbound federation connections
  - mTLS reqwest Client builder for outbound federation requests
  - Certificate SHA-256 fingerprint computation (FAUT-02)
  - ServerIdentity struct and well-known endpoint handler (RELY-01)
  - RemoteServerCache for resolved server identity caching
  - Pre-signed CA + 2 server certificates for demo federation

affects: [07-03-federation-module, 07-04-wiring-docker]

tech-stack:
  added: [rustls 0.23, rustls-pki-types 1.x, reqwest 0.12 with rustls-tls]
  patterns: [mTLS for federation auth, certificate fingerprint verification, well-known endpoint for server discovery]

key-files:
  created:
    - demo/src/tls.rs
    - demo/certs/generate.sh
    - demo/certs/ca.pem
    - demo/certs/ca-key.pem
    - demo/certs/server-a.pem
    - demo/certs/server-a-key.pem
    - demo/certs/server-b.pem
    - demo/certs/server-b-key.pem
    - demo/certs/README.md
  modified:
    - demo/src/identity.rs
    - demo/src/main.rs
    - demo/Cargo.toml

key-decisions:
  - "Used rustls-pki-types PemObject trait for PEM loading instead of deprecated rustls-pemfile"
  - "Ed25519 for CA and server TLS certificates matching protocol signing algorithm"

patterns-established:
  - "mTLS builder pattern: separate functions for server config vs client config"
  - "Certificate fingerprint format: sha256:<lowercase-hex-no-separators>"

requirements-completed: [RELY-01, FAUT-01, FAUT-02]

duration: 6min
completed: 2026-03-29
---

# Phase 7 Plan 2: TLS & Server Identity Summary

**mTLS configuration module with rustls, ServerIdentity well-known endpoint, and pre-signed CA + 2 server certificates for federation demo**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-29T05:38:00Z
- **Completed:** 2026-03-29T05:44:16Z
- **Tasks:** 3
- **Files modified:** 12

## Accomplishments
- Created tls.rs module with 4 public functions: mTLS server config builder, mTLS client builder, cert fingerprint computation, PEM-to-DER loader
- Extended identity.rs with ServerIdentity struct, builder, RemoteServerCache, and Axum handler for GET /.well-known/ainm/server
- Generated pre-signed Ed25519 CA + 2 server certificates with Docker-compatible SANs

## Task Commits

Each task was committed atomically:

1. **Task 1: Create TLS configuration module (tls.rs)** - `531c014` (feat)
2. **Task 2: Extend identity.rs with ServerIdentity and handler** - `3c3643a` (feat)
3. **Task 3: Generate pre-signed mTLS certificates** - `3b68e3b` (feat)

## Files Created/Modified
- `demo/src/tls.rs` - mTLS ServerConfig builder, reqwest Client builder, fingerprint computation
- `demo/src/identity.rs` - Added ServerIdentity struct, build_server_identity, RemoteServerCache, server_identity_handler
- `demo/src/main.rs` - Added `mod tls;` declaration
- `demo/Cargo.toml` - Added rustls, rustls-pki-types, reqwest dependencies
- `demo/certs/generate.sh` - Certificate generation script using openssl CLI
- `demo/certs/ca.pem` - Self-signed CA certificate (Ed25519, 10-year validity)
- `demo/certs/ca-key.pem` - CA private key
- `demo/certs/server-a.pem` - Server A cert (CN=server-a.local, SANs: server-a.local, server-a, localhost)
- `demo/certs/server-a-key.pem` - Server A private key
- `demo/certs/server-b.pem` - Server B cert (CN=server-b.local, SANs: server-b.local, server-b, localhost)
- `demo/certs/server-b-key.pem` - Server B private key
- `demo/certs/README.md` - Certificate file descriptions and SAN documentation

## Decisions Made
- Used rustls-pki-types PemObject trait for PEM file loading (rustls-pemfile is deprecated/superseded)
- Used Ed25519 for TLS certificate keys matching the protocol's Ed25519 signing algorithm
- Certificate SANs include Docker service names (server-a, server-b) alongside .local domains for container-to-container communication

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added PemObject trait import for rustls-pki-types**
- **Found during:** Task 1 (TLS module creation)
- **Issue:** CertificateDer::pem_file_iter and PrivateKeyDer::from_pem_file require the PemObject trait to be in scope
- **Fix:** Added `use rustls_pki_types::pem::PemObject;` import
- **Files modified:** demo/src/tls.rs
- **Verification:** cargo check passes
- **Committed in:** 531c014 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Trivial import fix. No scope creep.

## Issues Encountered
None beyond the PemObject import deviation above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- tls.rs exports ready for Plan 03 (federation module) to build federation sender/handler with mTLS
- ServerIdentity and RemoteServerCache ready for Plan 03 to use in server discovery
- Certificates ready for Plan 04 (Docker Compose wiring) to mount into containers
- server_identity_handler ready for Plan 04 to mount at /.well-known/ainm/server route

## Self-Check: PASSED

All 10 created files verified present. All 3 task commits verified in git log.

---
*Phase: 07-federation-relay-core*
*Completed: 2026-03-29*
