# Cross-Platform Test Vectors

Shared test vectors for validating cryptographic operations across Rust and Swift implementations.

## Files

- `jcs_vectors.json` -- RFC 8785 JCS canonicalization vectors. Both Rust (`serde_json_canonicalizer`) and Swift (`JCSCanonicalizer`) must produce identical canonical output for each input.
- `signing_vectors.json` -- Ed25519 signing vectors using a fixed test keypair. Both Rust (`ring`) and Swift (Apple CryptoKit) must produce identical signatures for the same canonical input signed with the same private key seed.
- `generate_vectors.rs` -- Rust program that generates both vector files. Run via `cargo run --example generate_vectors` from the `server/` directory.

## Key Pair

The test vectors use the RFC 8032 Section 7.1 Test Vector 1 private key seed. The seed is deterministic, so both `ring::signature::Ed25519KeyPair::from_seed_unchecked()` and Apple CryptoKit's `Curve25519.Signing.PrivateKey(rawRepresentation:)` derive the same keypair.

**This key pair is for testing only. Never use it in production.**

## Usage

### Rust (server project)
The server project's test suite validates against these vectors automatically.

### Swift (iOS app)
`MessageSignerTests.swift` loads these JSON files and validates that:
1. `JCSCanonicalizer.canonicalize()` matches `expected_canonical` for each JCS vector
2. `MessageSigner.sign()` produces signatures matching `expected_signature_base64url`
3. `MessageSigner.verify()` accepts signatures from the vectors

## Regenerating

If the signing procedure changes, regenerate vectors:
```bash
cd server/
cargo run --example generate_vectors
```
Then copy the JSON output to update the vector files.

## Design Decision (D-08, D-09)

Per project context decisions:
- D-08: Shared test vectors for cross-platform crypto validation
- D-09: Vectors live at `tests/vectors/` at repo root, shared across Rust and Swift
