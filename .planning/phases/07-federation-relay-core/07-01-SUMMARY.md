---
phase: 07-federation-relay-core
plan: 01
subsystem: demo-cli
tags: [federation, cli, dependencies, cargo]
dependency_graph:
  requires: []
  provides: [federation-deps, cli-flags, mode-branching]
  affects: [demo/Cargo.toml, demo/src/main.rs]
tech_stack:
  added: [reqwest-0.13, rustls-0.23, tokio-rustls-0.26, rustls-pki-types-1.14, clap-4.6, hyper-1, hyper-util-0.1, tower-0.5]
  patterns: [clap-derive-cli, mode-branching]
key_files:
  created: []
  modified: [demo/Cargo.toml, demo/src/main.rs]
decisions:
  - "Used reqwest rustls feature (not rustls-tls) -- feature renamed in reqwest 0.13"
  - "Used rustls-pki-types instead of rustls-pemfile per RUSTSEC-2025-0134 advisory"
  - "Added unknown mode error handling with non-zero exit for robustness"
metrics:
  duration: 5m 11s
  completed: "2026-03-29T05:42:34Z"
  tasks: 2/2
  files_modified: 2
---

# Phase 07 Plan 01: Federation Dependencies and CLI Foundation Summary

Federation dependency setup with clap CLI argument parsing for dual-mode (legacy/federation) binary operation.

## What Was Done

### Task 1: Add federation dependencies to Cargo.toml
- **Commit:** 57ac689
- Added reqwest 0.13 (rustls + json) for server-to-server HTTP relay
- Added rustls 0.23, tokio-rustls 0.26, rustls-pki-types 1.14 for mTLS
- Added clap 4.6 with derive feature for CLI argument parsing
- Added hyper 1.x, hyper-util, tower 0.5 for federation TLS listener
- All dependencies resolve and compile cleanly

### Task 2: Refactor main.rs with clap CLI and mode branching
- **Commit:** a27aad2
- Added `Cli` struct with 8 flags: --mode, --port, --federation-port, --identity, --cert, --key, --ca-cert, --remote
- No flags = run legacy demo (backward compatible per D-06)
- `--mode human|agent` = federation placeholder for plans 02-04
- Extracted `run_legacy_demo()` function for clean code separation
- Added error handling for unknown mode values

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed reqwest feature name for v0.13**
- **Found during:** Task 1
- **Issue:** Plan specified `rustls-tls` feature but reqwest 0.13 renamed it to `rustls`
- **Fix:** Changed feature from `rustls-tls` to `rustls`
- **Files modified:** demo/Cargo.toml
- **Commit:** 57ac689

## Verification Results

1. `cargo build` -- PASSED
2. `ainm-demo --help` shows all 8 flags -- PASSED
3. `ainm-demo` (no flags) starts legacy demo path -- PASSED
4. `ainm-demo --mode human --identity test` prints federation placeholder -- PASSED

## Known Stubs

- `--mode human|agent` prints placeholder message and exits -- intentional, wired in plans 02-04

## Self-Check: PASSED
