# Accounts & Identity Binding (iOS) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Execution environment note:** this plan touches Rust cross-compilation for iOS, an XCFramework, and a SwiftUI app. Tasks 1–4 (Rust + FFI) are verifiable with `cargo test` on the host. Tasks 5+ require **Xcode 16+ on macOS** (`xcodebuild` + a simulator) to compile/test, and the final wave needs a running dev server. Subagents without a working Xcode/simulator cannot fully verify the Swift tasks — for those, "verify it compiles via `xcodebuild build`" is the gate, and the human runs the simulator E2E.

**Goal:** Ship the minimal iOS signup + recovery flow (email→code→username→PIN→FaceID) so a real person can create and recover an AINM account end-to-end against the now-enforced server, with the OPRF escrow done by the *same* `voprf` crate the server uses (via a Rust→Swift FFI), so recovery is wire-compatible by construction.

**Architecture:** Extract the server's pure escrow crypto into a new workspace crate `ainm-escrow` (no DB/HTTP deps) consumed by both the server (`server/`, crate `ainm-server`) and a new UniFFI crate `ainm-escrow-ffi`. The FFI crate is cross-compiled to an `AinmEscrow.xcframework` and added to the iOS app as a binary SwiftPM target. On the Swift side: add the `timestamp` field the enforced handshake now requires; add an `AccountService` REST client; extend `KeyManager` to export/import the 32-byte key; add an `EscrowClient` (wraps the FFI: enroll→upload, recover→blind/fetch/finalize→restore) and a `BiometricManager`; and a multi-step onboarding + recovery SwiftUI flow wired into `AppState`.

**Tech Stack:** Rust (voprf 0.5, argon2, hkdf, sha2, chacha20poly1305), UniFFI 0.31 (`#[uniffi::export]`, proc-macro/no-UDL), Cargo workspace, XCFramework; Swift 6 / SwiftUI iOS 18+, CryptoKit (Curve25519), URLSession, LocalAuthentication, GRDB/KeychainAccess (already present).

**Depends on:** the merged server work (commit `01b79ec` on `main`) — endpoints `POST /accounts/{signup,verify,keys,escrow,recover/start,recover/verify,escrow/fetch}`, the enforced handshake requiring `timestamp` in `ainm.initialize` params, and `server/src/escrow.rs` (the crypto being extracted).

---

## Frozen contracts (from the shipped server — the iOS client MUST match these)

- **Handshake:** Ed25519 over UTF-8 `"{identity}|{timestamp}"`, base64url-no-pad signature. `params` carries `protocolVersion`, `identity`, `timestamp` (RFC3339), `signature`, `capabilities`. Server freshness window ±300s.
- **OPRF:** RFC 9497 `OPRF(ristretto255, SHA-512)` via `voprf` 0.5 ciphersuite `Ristretto255`. (FFI uses the same crate → guaranteed match.)
- **KDF / wire (from `escrow.rs`):**
  - OPRF input = `Argon2id(pin_utf8, salt)` → 32 bytes; `kdf_params` JSON = `{"salt": "<base64url>", "m_cost": 19456, "t_cost": 2, "p_cost": 1}`.
  - `wrapping_key` = `HKDF-SHA256(ikm=oprf_output_64, salt=b"ainm-escrow-v1", info=b"wrapping-key")` → 32 bytes.
  - Sealed blob = `nonce(24) || XChaCha20Poly1305(wrapping_key).encrypt(nonce, secret32)`.
  - `oprf_key` stored server-side = `OprfServer::<Ristretto255>::serialize()` bytes (the client generates this at enrollment and uploads it).
  - Wire encodings: blinded/evaluated elements and ciphertext/oprf_key are base64url-no-pad; `kdf_params` is JSON.
- **REST request/response shapes (server handlers in `server/src/server.rs`):**
  - `POST /accounts/signup {email, username}` → `200 {address}` | `409`.
  - `POST /accounts/verify {email, code}` → `200 {enrollment_token}` | `401`.
  - `POST /accounts/keys {enrollment_token, public_key, device_label?}` → `204` | `401`/`409`.
  - `POST /accounts/escrow {enrollment_token, ciphertext_b64, kdf_params, oprf_key_b64}` → `204` (also flips account `active`).
  - `POST /accounts/recover/start {email}` → `204` (always; no existence leak).
  - `POST /accounts/recover/verify {email, code}` → `200 {recovery_token}` | `401`.
  - `POST /accounts/escrow/fetch {recovery_token, blinded_point}` → `200 {evaluated_point, ciphertext_b64, kdf_params}` | `410 Gone` (destroyed) | `404`.
  - `public_key` is base64url-no-pad of the 32-byte Ed25519 public key (matches `KeyManager.publicKeyBase64url`).

---

## File structure

**Rust (new Cargo workspace layout):**
| Path | Responsibility |
|------|----------------|
| `Cargo.toml` (repo root, NEW) | Workspace manifest listing members `server`, `crates/ainm-escrow`, `crates/ainm-escrow-ffi` |
| `crates/ainm-escrow/` (NEW) | Pure escrow crypto extracted from `server/src/escrow.rs` (no DB/HTTP). Public: `KdfParams`, `Enrolled`, `enroll`, `BlindHandle`, `client_blind`, `client_finalize_and_open`, `server_evaluate` |
| `crates/ainm-escrow-ffi/` (NEW) | UniFFI wrapper exposing the client-side ops to Swift; builds `AinmEscrow.xcframework` |
| `server/src/escrow.rs` (MODIFY) | Re-export the pure fns from `ainm-escrow`; keep the DB fns (`store_escrow`, `fetch_and_evaluate`) here |

**Swift (`ios/AINMMessenger/AINMMessenger/`):**
| Path | Responsibility |
|------|----------------|
| `Models/InitializeParams.swift` (MODIFY) | Add `timestamp` field |
| `Networking/AINMClient.swift` (MODIFY) | Pass `timestamp` into `InitializeParams` |
| `Networking/AccountService.swift` (NEW) | URLSession REST client for the 7 account endpoints |
| `Crypto/KeyManager.swift` (MODIFY) | Add `exportPrivateKeyBytes()` and `importKeypair(from:)` |
| `Crypto/EscrowClient.swift` (NEW) | Wraps the FFI: `enrollAndUpload`, `recover`; bridges to `AccountService` |
| `Crypto/BiometricManager.swift` (NEW) | LocalAuthentication FaceID/Touch gate |
| `UI/Onboarding/*` (NEW) | Multi-step signup + recovery SwiftUI views |
| `App/AppState.swift` (MODIFY) | Account/onboarding state + `signUp`/`recover` actions |
| `App/AINMMessengerApp.swift` (MODIFY) | Route to onboarding vs connected |
| `AINMMessenger.xcodeproj` (MODIFY) | Add `AinmEscrow` binary SwiftPM target + `Argon2`/local FFI package |

---

## WAVE 0 — Shared Rust escrow crate + workspace

### Task 1: Extract pure escrow crypto into `crates/ainm-escrow`

**Files:**
- Create: `Cargo.toml` (repo root workspace), `crates/ainm-escrow/Cargo.toml`, `crates/ainm-escrow/src/lib.rs`
- Modify: `server/Cargo.toml`, `server/src/escrow.rs`
- Test: `crates/ainm-escrow/src/lib.rs` (`#[cfg(test)]`)

- [ ] **Step 1: Create the workspace manifest** at the repo root `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["server", "crates/ainm-escrow", "crates/ainm-escrow-ffi"]
```

(If `server/` already has `[workspace]` or the repo root has no Cargo.toml, create it. Confirm `server` still builds as a workspace member: `cargo build -p ainm-server`.)

- [ ] **Step 2: Create `crates/ainm-escrow/Cargo.toml`**

```toml
[package]
name = "ainm-escrow"
version = "0.1.0"
edition = "2021"

[dependencies]
voprf = { version = "0.5", features = ["ristretto255"] }
argon2 = "0.5"
hkdf = "0.12"
sha2 = "0.10"
chacha20poly1305 = { version = "0.10", features = ["alloc"] }
base64 = "0.22"
rand = "0.8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 3: Move the pure crypto into `crates/ainm-escrow/src/lib.rs`**

Move the pure functions and types VERBATIM from `server/src/escrow.rs` (everything EXCEPT the DB functions `store_escrow`/`fetch_and_evaluate`/`EscrowFetch`/`FetchError` and the `use crate::error::StorageError; use sqlx::PgPool;` imports): `KdfParams`, `pin_to_input`, `wrapping_key`, `Enrolled`, `enroll`, `BlindState`, `client_blind`, `server_evaluate`, `client_finalize_and_open`, and the round-trip `#[cfg(test)]` test. Keep `KdfParams` deriving `Serialize, Deserialize`. Remove the `#[allow(dead_code)]` attributes (these are now public API of a library crate, legitimately exported).

Add the module doc comment at the top. The file is otherwise identical to the pure portion of `server/src/escrow.rs`.

- [ ] **Step 4: Rewrite `server/src/escrow.rs` to depend on the shared crate**

Replace the moved pure code with re-exports, keeping the DB layer:

```rust
//! Server-side escrow persistence. Pure crypto lives in the `ainm-escrow` crate
//! (shared verbatim with the iOS FFI client so recovery is wire-compatible).

pub use ainm_escrow::{
    client_blind, client_finalize_and_open, enroll, server_evaluate, BlindState, Enrolled, KdfParams,
};

use crate::error::StorageError;
use sqlx::PgPool;

// ... keep store_escrow, EscrowFetch, FetchError, fetch_and_evaluate EXACTLY as they are now ...
```

Add `ainm-escrow = { path = "../crates/ainm-escrow" }` to `server/Cargo.toml` `[dependencies]`. Remove the now-unused `voprf`/`argon2`/`hkdf`/`chacha20poly1305` deps from `server/Cargo.toml` ONLY IF nothing else in `server/` uses them (grep first: `rg "voprf|argon2::|hkdf::|chacha20poly1305" server/src`); if `escrow.rs` was the only user, remove them; otherwise leave them.

- [ ] **Step 5: Verify the shared crate's test + the server still passes**

Run: `cargo test -p ainm-escrow` → the round-trip test passes.
Run: `cargo test -p ainm-server --test integration test_escrow_store_and_fetch -- --nocapture` and `test_account_lifecycle_e2e` → still PASS (the server now calls into `ainm-escrow` for the same behavior).
Run: `cargo build` (whole workspace) → clean.

- [ ] **Step 6: Commit**

```bash
rtk proxy git add Cargo.toml crates/ainm-escrow server/Cargo.toml server/Cargo.lock server/src/escrow.rs Cargo.lock
rtk proxy git commit -m "refactor(escrow): extract pure crypto into shared ainm-escrow crate"
```

---

### Task 2: UniFFI crate `ainm-escrow-ffi`

**Files:**
- Create: `crates/ainm-escrow-ffi/Cargo.toml`, `crates/ainm-escrow-ffi/src/lib.rs`, `crates/ainm-escrow-ffi/build.rs`, `crates/ainm-escrow-ffi/uniffi.toml`
- Test: `crates/ainm-escrow-ffi/src/lib.rs`

The Swift client needs three operations. Enrollment runs the whole OPRF locally (generating the server key) and returns the three blobs to upload. Recovery is two-phase: blind (returns a handle Swift holds across the network round-trip) then finalize+open. Expose them via UniFFI.

- [ ] **Step 1: `crates/ainm-escrow-ffi/Cargo.toml`**

```toml
[package]
name = "ainm-escrow-ffi"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["staticlib", "cdylib", "lib"]
name = "ainm_escrow_ffi"

[dependencies]
ainm-escrow = { path = "../ainm-escrow" }
uniffi = { version = "0.31", features = ["cli"] }

[build-dependencies]
uniffi = { version = "0.31", features = ["build"] }
```

- [ ] **Step 2: `crates/ainm-escrow-ffi/build.rs`**

```rust
fn main() {
    uniffi::generate_scaffolding("src/ainm_escrow.udl").ok(); // no-op if using proc-macro only
    uniffi_build_setup();
}
fn uniffi_build_setup() { uniffi::generate_scaffolding_for_crate(); }
```

NOTE for the implementer: UniFFI 0.31's proc-macro mode does not require a `.udl` file. If the above helper names differ in 0.31, follow the current UniFFI 0.31 "proc-macro / library mode" docs (https://mozilla.github.io/uniffi-rs/latest/) — the build.rs may simply be `fn main() { uniffi::setup_scaffolding!(); }` invoked from lib.rs instead. Use the `ianthetechie/uniffi-starter` template as the reference for the exact 0.31 wiring. The contract that matters is the exported API below.

- [ ] **Step 3: `crates/ainm-escrow-ffi/src/lib.rs` — the exported API**

```rust
//! UniFFI surface over `ainm-escrow` for the iOS client. Same crate the server uses.
uniffi::setup_scaffolding!();

use ainm_escrow::{client_blind, client_finalize_and_open, enroll, BlindState, KdfParams};

#[derive(uniffi::Record)]
pub struct EnrollResult {
    pub oprf_key_b64: String,    // upload as oprf_key_b64
    pub ciphertext_b64: String,  // upload as ciphertext_b64
    pub kdf_params_json: String, // upload as kdf_params (JSON object)
}

#[derive(uniffi::Error, Debug, thiserror::Error)]
pub enum EscrowFfiError {
    #[error("{0}")]
    Crypto(String),
}

/// Enrollment: derive wrapping key + seal the 32-byte secret. Returns the 3 blobs to upload.
#[uniffi::export]
pub fn escrow_enroll(secret: Vec<u8>, pin: String) -> Result<EnrollResult, EscrowFfiError> {
    let secret: [u8; 32] = secret.try_into().map_err(|_| EscrowFfiError::Crypto("secret must be 32 bytes".into()))?;
    let e = enroll(&secret, &pin).map_err(EscrowFfiError::Crypto)?;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    Ok(EnrollResult {
        oprf_key_b64: URL_SAFE_NO_PAD.encode(&e.oprf_key),
        ciphertext_b64: URL_SAFE_NO_PAD.encode(&e.ciphertext),
        kdf_params_json: serde_json::to_string(&e.kdf_params).map_err(|err| EscrowFfiError::Crypto(err.to_string()))?,
    })
}

/// Opaque handle holding the blind state between blind() and finalize().
#[derive(uniffi::Object)]
pub struct EscrowRecovery {
    inner: std::sync::Mutex<Option<BlindState>>,
    kdf_json: String,
    pin: String,
}

#[uniffi::export]
impl EscrowRecovery {
    /// Step 1: blind the PIN. `kdf_params_json` comes from the server's escrow/fetch response.
    #[uniffi::constructor]
    pub fn begin(pin: String, kdf_params_json: String) -> Result<std::sync::Arc<Self>, EscrowFfiError> {
        let kdf: KdfParams = serde_json::from_str(&kdf_params_json).map_err(|e| EscrowFfiError::Crypto(e.to_string()))?;
        let blind = client_blind(&pin, &kdf).map_err(EscrowFfiError::Crypto)?;
        Ok(std::sync::Arc::new(Self {
            inner: std::sync::Mutex::new(Some(blind)),
            kdf_json: kdf_params_json,
            pin,
        }))
    }

    /// The base64url blinded point to POST as `blinded_point`.
    pub fn blinded_point(&self) -> Result<String, EscrowFfiError> {
        let g = self.inner.lock().unwrap();
        g.as_ref().map(|b| b.blinded_b64.clone()).ok_or(EscrowFfiError::Crypto("already finalized".into()))
    }

    /// Step 2: finalize with the server's evaluated point + ciphertext; returns the 32-byte secret.
    pub fn finalize(&self, evaluated_point: String, ciphertext_b64: String) -> Result<Vec<u8>, EscrowFfiError> {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        let blind = self.inner.lock().unwrap().take().ok_or(EscrowFfiError::Crypto("already finalized".into()))?;
        let kdf: KdfParams = serde_json::from_str(&self.kdf_json).map_err(|e| EscrowFfiError::Crypto(e.to_string()))?;
        let ct = URL_SAFE_NO_PAD.decode(&ciphertext_b64).map_err(|e| EscrowFfiError::Crypto(e.to_string()))?;
        let secret = client_finalize_and_open(&self.pin, &kdf, &blind, &evaluated_point, &ct).map_err(EscrowFfiError::Crypto)?;
        Ok(secret.to_vec())
    }
}
```

Add `thiserror = "1"` to `ainm-escrow-ffi/Cargo.toml` deps (for the error derive). If `client_finalize_and_open` takes `&BlindState` (it does), the `.take()` move above must be adapted: bind `let blind = ...take()...; client_finalize_and_open(&self.pin, &kdf, &blind, ...)`. (Pass `&blind`.)

- [ ] **Step 4: Rust round-trip test through the FFI surface**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ffi_enroll_then_local_recover() {
        // Enroll
        let secret = vec![5u8; 32];
        let pin = "314159".to_string();
        let e = escrow_enroll(secret.clone(), pin.clone()).unwrap();

        // Simulate the server evaluation locally using the uploaded oprf_key.
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        let oprf_key = URL_SAFE_NO_PAD.decode(&e.oprf_key_b64).unwrap();

        let rec = EscrowRecovery::begin(pin, e.kdf_params_json.clone()).unwrap();
        let blinded = rec.blinded_point().unwrap();
        let evaluated = ainm_escrow::server_evaluate(&oprf_key, &blinded).unwrap();
        let recovered = rec.finalize(evaluated, e.ciphertext_b64).unwrap();
        assert_eq!(recovered, secret);
    }
}
```

- [ ] **Step 5: Verify**

Run: `cargo test -p ainm-escrow-ffi -- --nocapture` → PASS (enroll → blind → server_evaluate → finalize recovers the secret). `cargo build` whole workspace clean.

- [ ] **Step 6: Commit**

```bash
rtk proxy git add Cargo.toml crates/ainm-escrow-ffi Cargo.lock
rtk proxy git commit -m "feat(ffi): UniFFI escrow client (enroll/blind/finalize) over ainm-escrow"
```

---

### Task 3: Build `AinmEscrow.xcframework` + generate Swift bindings

**Files:**
- Create: `crates/ainm-escrow-ffi/rust-toolchain.toml`, `scripts/build-escrow-xcframework.sh`
- Output (gitignored or committed per team pref): `ios/Frameworks/AinmEscrow.xcframework`, generated `ios/.../Generated/ainm_escrow_ffi.swift`

**This task requires a macOS host with Xcode + Rust iOS targets. A subagent without them should report BLOCKED and leave the script for the human to run.**

- [ ] **Step 1: Pin the toolchain** — `crates/ainm-escrow-ffi/rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
targets = ["aarch64-apple-ios", "aarch64-apple-ios-sim", "x86_64-apple-ios"]
```

- [ ] **Step 2: Build script** `scripts/build-escrow-xcframework.sh` (chmod +x). Follow the UniFFI 0.31 + uniffi-starter pattern:

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
CRATE=ainm-escrow-ffi
LIB=libainm_escrow_ffi.a
OUT=ios/Frameworks
GEN=ios/AINMMessenger/AINMMessenger/Generated

rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios

# 1. Build static libs for each target
cargo build -p "$CRATE" --release --target aarch64-apple-ios
cargo build -p "$CRATE" --release --target aarch64-apple-ios-sim
cargo build -p "$CRATE" --release --target x86_64-apple-ios

# 2. Generate Swift bindings from the built lib (UniFFI 0.31 library mode)
mkdir -p "$GEN"
cargo run -p "$CRATE" --release --bin uniffi-bindgen generate \
  --library target/aarch64-apple-ios/release/$LIB \
  --language swift --out-dir "$GEN"
# (If the crate has no uniffi-bindgen bin, add one per the uniffi-starter template:
#  src/bin/uniffi-bindgen.rs => fn main() { uniffi::uniffi_bindgen_main() } )

# 3. Lipo the two simulator arches together
mkdir -p target/ios-sim-universal/release
lipo -create \
  target/aarch64-apple-ios-sim/release/$LIB \
  target/x86_64-apple-ios/release/$LIB \
  -output target/ios-sim-universal/release/$LIB

# 4. Assemble the XCFramework (headers = the generated modulemap + .h)
HEADERS=target/uniffi-headers
mkdir -p "$HEADERS"
cp "$GEN"/*.h "$HEADERS"/ 2>/dev/null || true
cp "$GEN"/*.modulemap "$HEADERS"/module.modulemap 2>/dev/null || true

rm -rf "$OUT/AinmEscrow.xcframework"
mkdir -p "$OUT"
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/$LIB -headers "$HEADERS" \
  -library target/ios-sim-universal/release/$LIB -headers "$HEADERS" \
  -output "$OUT/AinmEscrow.xcframework"

echo "Built $OUT/AinmEscrow.xcframework and Swift bindings in $GEN"
```

NOTE: exact bindgen invocation/flags vary by UniFFI 0.31 patch — the implementer MUST verify against the installed UniFFI version and the uniffi-starter template; the script is the scaffold, the contract is "an `AinmEscrow.xcframework` + a generated `ainm_escrow_ffi.swift` that exposes `escrowEnroll`, `EscrowRecovery.begin`, `.blindedPoint()`, `.finalize(...)`".

- [ ] **Step 3: Run the script + sanity-check the generated Swift**

Run: `bash scripts/build-escrow-xcframework.sh`. Confirm `ios/Frameworks/AinmEscrow.xcframework` exists and `ios/.../Generated/ainm_escrow_ffi.swift` declares the exported symbols.

- [ ] **Step 4: Add the framework to the Xcode project**

In Xcode: drag `AinmEscrow.xcframework` into the app target's "Frameworks, Libraries, and Embedded Content" (Embed & Sign), and add the `Generated/ainm_escrow_ffi.swift` to the target. (Or edit `project.pbxproj` accordingly.) Confirm the app target builds: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'generic/platform=iOS Simulator' build` succeeds.

- [ ] **Step 5: Commit** (decide framework-in-git vs build-on-CI with the team; default: commit the script + generated bindings, gitignore the .xcframework binary and document the build step)

```bash
rtk proxy git add scripts/build-escrow-xcframework.sh crates/ainm-escrow-ffi/rust-toolchain.toml ios/AINMMessenger/AINMMessenger/Generated .gitignore
rtk proxy git commit -m "build(ffi): AinmEscrow.xcframework build script + Swift bindings"
```

---

## WAVE 1 — Swift plumbing (handshake fix + REST + key export)

### Task 4: Add `timestamp` to the handshake (unblocks connecting to the enforced server)

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/Models/InitializeParams.swift`, `ios/AINMMessenger/AINMMessenger/Networking/AINMClient.swift`

- [ ] **Step 1: Add the field** in `InitializeParams.swift` (struct currently `protocolVersion, identity, signature, capabilities`):

```swift
struct InitializeParams: Codable, Sendable {
    let protocolVersion: String
    let identity: String
    let timestamp: String   // RFC3339; server reconstructs "{identity}|{timestamp}" to verify
    let signature: String
    let capabilities: ClientCapabilities
}
```

- [ ] **Step 2: Pass it through** in `AINMClient.swift` where `InitializeParams(...)` is built (the `timestamp` local already exists, currently only used for signing) — add `timestamp: timestamp,` to the initializer call.

- [ ] **Step 3: Verify it builds**

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'generic/platform=iOS Simulator' build` → succeeds. (No behavior test here; the connect path is exercised in Wave 4 against a dev server.)

- [ ] **Step 4: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Models/InitializeParams.swift ios/AINMMessenger/AINMMessenger/Networking/AINMClient.swift
rtk proxy git commit -m "fix(ios): send timestamp in ainm.initialize params (enforced handshake)"
```

---

### Task 5: `AccountService` REST client

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/Networking/AccountService.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/AccountServiceTests.swift` (codable round-trip; if a test target exists — check the project)

Mirror the `AgentCardService` actor pattern (URLSession async/await, typed Codable, `LocalizedError` enum).

- [ ] **Step 1: Write `AccountService.swift`**

```swift
import Foundation

actor AccountService {
    private let session = URLSession.shared
    private let baseURL: URL
    init(baseURL: URL) { self.baseURL = baseURL }

    enum AccountError: Error, LocalizedError {
        case http(Int, String)
        case decoding
        case escrowDestroyed
        var errorDescription: String? {
            switch self {
            case .http(let c, let m): return "Server error \(c): \(m)"
            case .decoding: return "Could not read the server response."
            case .escrowDestroyed: return "Recovery is no longer possible (too many attempts)."
            }
        }
    }

    // Request/response DTOs
    struct SignupReq: Encodable { let email: String; let username: String }
    struct SignupResp: Decodable { let address: String }
    struct VerifyReq: Encodable { let email: String; let code: String }
    struct VerifyResp: Decodable { let enrollment_token: String }
    struct BindKeyReq: Encodable { let enrollment_token: String; let public_key: String; let device_label: String? }
    struct KdfParams: Codable { let salt: String; let m_cost: UInt32; let t_cost: UInt32; let p_cost: UInt32 }
    struct StoreEscrowReq: Encodable { let enrollment_token: String; let ciphertext_b64: String; let kdf_params: KdfParams; let oprf_key_b64: String }
    struct RecoverStartReq: Encodable { let email: String }
    struct RecoverVerifyReq: Encodable { let email: String; let code: String }
    struct RecoverVerifyResp: Decodable { let recovery_token: String }
    struct EscrowFetchReq: Encodable { let recovery_token: String; let blinded_point: String }
    struct EscrowFetchResp: Decodable { let evaluated_point: String; let ciphertext_b64: String; let kdf_params: KdfParams }

    private func post<Req: Encodable, Resp: Decodable>(_ path: String, _ body: Req) async throws -> Resp {
        var req = URLRequest(url: baseURL.appendingPathComponent(path))
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try JSONEncoder().encode(body)
        let (data, resp) = try await session.data(for: req)
        guard let http = resp as? HTTPURLResponse else { throw AccountError.decoding }
        if http.statusCode == 410 { throw AccountError.escrowDestroyed }
        guard (200..<300).contains(http.statusCode) else {
            throw AccountError.http(http.statusCode, String(data: data, encoding: .utf8) ?? "")
        }
        if Resp.self == EmptyResp.self { return EmptyResp() as! Resp }
        return try JSONDecoder().decode(Resp.self, from: data)
    }
    struct EmptyResp: Decodable {}

    func signup(email: String, username: String) async throws -> String {
        let r: SignupResp = try await post("accounts/signup", SignupReq(email: email, username: username)); return r.address
    }
    func verify(email: String, code: String) async throws -> String {
        let r: VerifyResp = try await post("accounts/verify", VerifyReq(email: email, code: code)); return r.enrollment_token
    }
    func bindKey(enrollmentToken: String, publicKeyB64: String, deviceLabel: String?) async throws {
        let _: EmptyResp = try await post("accounts/keys", BindKeyReq(enrollment_token: enrollmentToken, public_key: publicKeyB64, device_label: deviceLabel))
    }
    func storeEscrow(enrollmentToken: String, ciphertextB64: String, kdf: KdfParams, oprfKeyB64: String) async throws {
        let _: EmptyResp = try await post("accounts/escrow", StoreEscrowReq(enrollment_token: enrollmentToken, ciphertext_b64: ciphertextB64, kdf_params: kdf, oprf_key_b64: oprfKeyB64))
    }
    func recoverStart(email: String) async throws {
        let _: EmptyResp = try await post("accounts/recover/start", RecoverStartReq(email: email))
    }
    func recoverVerify(email: String, code: String) async throws -> String {
        let r: RecoverVerifyResp = try await post("accounts/recover/verify", RecoverVerifyReq(email: email, code: code)); return r.recovery_token
    }
    func escrowFetch(recoveryToken: String, blindedPoint: String) async throws -> EscrowFetchResp {
        try await post("accounts/escrow/fetch", EscrowFetchReq(recovery_token: recoveryToken, blinded_point: blindedPoint))
    }
}
```

- [ ] **Step 2: Verify it builds** (`xcodebuild ... build`). If a unit-test target exists, add a Codable round-trip test asserting `KdfParams`/DTO JSON keys match the server (snake_case). Confirm the JSON field names (`enrollment_token`, `ciphertext_b64`, `oprf_key_b64`, `kdf_params`, `blinded_point`, `evaluated_point`) exactly match the server handlers.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Networking/AccountService.swift ios/AINMMessenger/AINMMessengerTests 2>/dev/null
rtk proxy git commit -m "feat(ios): AccountService REST client for account endpoints"
```

---

### Task 6: `KeyManager` export/import

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/Crypto/KeyManager.swift`

- [ ] **Step 1: Add two methods** to the `KeyManager` actor (it already does `getOrCreateKeypair()` and stores `privateKey.rawRepresentation` in Keychain via the `KeyStorage` protocol):

```swift
/// The 32-byte Ed25519 private seed for escrow sealing. Throws if no keypair exists yet.
func exportPrivateKeyBytes() throws -> Data {
    let kp = try getOrCreateKeypair()
    return kp.privateKey.rawRepresentation   // 32 bytes
}

/// Restore a keypair from 32 recovered bytes, persisting it to the Keychain (recovery path).
func importKeypair(from rawPrivateKey: Data) throws {
    // Validate by constructing the key; throws if not 32 valid bytes.
    _ = try Curve25519.Signing.PrivateKey(rawRepresentation: rawPrivateKey)
    try storage.save(rawPrivateKey)
}
```

(Confirm `storage` is the actor's `KeyStorage` field and `save(_:)` is its method — per the existing implementation.)

- [ ] **Step 2: Verify builds.** If a test target exists, add: generate keypair → export bytes → delete → import those bytes → `getOrCreateKeypair().publicKeyBase64url` equals the original (round-trip).

- [ ] **Step 3: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Crypto/KeyManager.swift
rtk proxy git commit -m "feat(ios): KeyManager export/import for escrow + recovery"
```

---

## WAVE 2 — Escrow client + biometrics

### Task 7: `EscrowClient` (FFI ↔ AccountService bridge)

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/Crypto/EscrowClient.swift`

Uses the generated FFI symbols (`escrowEnroll`, `EscrowRecovery`) + `AccountService` + `KeyManager`.

- [ ] **Step 1: Write `EscrowClient.swift`**

```swift
import Foundation
// import the generated module if UniFFI emits one (e.g. `import ainm_escrow_ffi`)

struct EscrowClient {
    let account: AccountService
    let keyManager: KeyManager

    /// Enrollment: seal the device key under the PIN and upload to the server.
    func enrollAndUpload(enrollmentToken: String, pin: String) async throws {
        let secret = try await keyManager.exportPrivateKeyBytes()       // 32 bytes
        let result = try escrowEnroll(secret: [UInt8](secret), pin: pin) // FFI
        // result.kdfParamsJson is a JSON object string; decode into the typed KdfParams for the request.
        let kdf = try JSONDecoder().decode(AccountService.KdfParams.self, from: Data(result.kdfParamsJson.utf8))
        try await account.storeEscrow(
            enrollmentToken: enrollmentToken,
            ciphertextB64: result.ciphertextB64,
            kdf: kdf,
            oprfKeyB64: result.oprfKeyB64
        )
    }

    /// Recovery: one blinded round-trip per PIN guess; on success restores the key into the Keychain.
    /// Returns true if the key was restored; throws AccountError.escrowDestroyed on 410.
    func recover(recoveryToken: String, pin: String) async throws -> Bool {
        // We need kdf_params to blind. Do an initial fetch with a placeholder blinded point? No —
        // the server returns kdf_params alongside every evaluation. So we MUST blind first, which
        // needs kdf_params. Resolve the ordering: the client stores kdf_params from enrollment is
        // NOT available on a fresh device. Therefore the FIRST fetch uses a "probe": blind with the
        // server-provided kdf_params requires it up front.
        //
        // CONTRACT NOTE: the server's escrow/fetch returns kdf_params in the SAME response as the
        // evaluation, but blinding needs kdf_params BEFORE calling. The server's `fetch_and_evaluate`
        // applies the OPRF to the blinded point the client sends — and the client derived that point
        // from Argon2id(pin, salt). The salt lives in kdf_params. So the client needs kdf_params first.
        //
        // The server already has the kdf_params; expose them without consuming an attempt. See Task 7a.
        fatalError("see Task 7a: needs a no-cost kdf_params fetch before blinding")
    }
}
```

**STOP — this surfaces a real protocol gap (see Task 7a). Do Task 7a first, then complete `recover`.**

- [ ] **Step 2:** (completed after Task 7a) — implement `recover` per 7a's resolution.

---

### Task 7a: Resolve the recovery `kdf_params` ordering (server + client)

**Problem:** to blind a PIN guess the client needs the `salt`/Argon2 params (`kdf_params`), but on a fresh device it doesn't have them — and the server only returns `kdf_params` *in the evaluation response*, which also *consumes an attempt*. The client must obtain `kdf_params` **before** blinding, without burning a guess.

**Decision (cheapest correct fix):** add a no-cost `GET` that returns only `kdf_params` for an authenticated recovery token (does NOT touch `eval_attempts`, does NOT return the ciphertext or evaluate anything).

**Files:**
- Modify (server): `server/src/server.rs` (new handler + route), `server/src/escrow.rs` (a read-only `get_kdf_params`)
- Test (server): `server/tests/integration.rs`
- Then complete: `ios/.../EscrowClient.swift`

- [ ] **Step 1 (server): `escrow.rs` read-only fetch**

```rust
/// Read kdf_params without touching eval_attempts (needed by the client to blind before guessing).
pub async fn get_kdf_params(pool: &PgPool, account_id: uuid::Uuid) -> Result<Option<(KdfParams, bool)>, StorageError> {
    let row: Option<(serde_json::Value, bool)> = sqlx::query_as(
        "SELECT kdf_params, destroyed FROM key_escrow WHERE account_id = $1",
    ).bind(account_id).fetch_optional(pool).await?;
    match row {
        Some((json, destroyed)) => {
            let kdf: KdfParams = serde_json::from_value(json).map_err(|e| StorageError::Duplicate(e.to_string()))?;
            Ok(Some((kdf, destroyed)))
        }
        None => Ok(None),
    }
}
```

- [ ] **Step 2 (server): handler + route** in `server.rs` — `POST /accounts/recover/params {recovery_token}` → `200 {kdf_params}` (404 if none, 410 if destroyed). Verify the `"recovery"` token, call `get_kdf_params`. Register in all 3 builders.

```rust
#[derive(serde::Deserialize)] pub struct RecoverParamsReq { pub recovery_token: String }
#[derive(serde::Serialize)] pub struct RecoverParamsResp { pub kdf_params: crate::escrow::KdfParams }

pub async fn recover_params_handler(State(state): State<ServerState>, Json(req): Json<RecoverParamsReq>)
 -> Result<Json<RecoverParamsResp>, (StatusCode, String)> {
    let Some(pool) = state.db.as_ref() else { return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into())); };
    let aid = state.token_signer.verify(&req.recovery_token, "recovery", crate::protocol::now_unix())
        .map_err(|e| (StatusCode::UNAUTHORIZED, format!("{e:?}")))?;
    let account_id = uuid::Uuid::parse_str(&aid).map_err(|_| (StatusCode::BAD_REQUEST, "bad token subject".into()))?;
    match crate::escrow::get_kdf_params(pool, account_id).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))? {
        Some((_, true)) => Err((StatusCode::GONE, "recovery blob destroyed".into())),
        Some((kdf, false)) => Ok(Json(RecoverParamsResp { kdf_params: kdf })),
        None => Err((StatusCode::NOT_FOUND, "no escrow".into())),
    }
}
```

- [ ] **Step 3 (server test):** extend `test_escrow_store_and_fetch` (or add one) to assert `get_kdf_params` returns the same salt that `enroll` produced and does NOT change `eval_attempts`. Run `cargo test -p ainm-server --test integration -- --nocapture` (only the 2 known failures).

- [ ] **Step 4 (server commit):**
```bash
rtk proxy git add server/src/escrow.rs server/src/server.rs server/tests/integration.rs
rtk proxy git commit -m "feat(escrow): add no-cost recover/params endpoint (kdf_params before blinding)"
```

- [ ] **Step 5 (client): add `recoverParams` to `AccountService.swift`** (`POST /accounts/recover/params {recovery_token}` → `{kdf_params}`), then complete `EscrowClient.recover`:

```swift
func recover(recoveryToken: String, pin: String) async throws -> Bool {
    let kdf = try await account.recoverParams(recoveryToken: recoveryToken)  // {kdf_params}
    let kdfJson = String(data: try JSONEncoder().encode(kdf), encoding: .utf8)!
    let rec = try EscrowRecovery.begin(pin: pin, kdfParamsJson: kdfJson)       // FFI: blind
    let blinded = try rec.blindedPoint()
    let resp = try await account.escrowFetch(recoveryToken: recoveryToken, blindedPoint: blinded) // may throw escrowDestroyed (410)
    let secret = try rec.finalize(evaluatedPoint: resp.evaluated_point, ciphertextB64: resp.ciphertext_b64) // FFI: wrong PIN throws
    try await keyManager.importKeypair(from: Data(secret))                     // restore key
    return true
}
```

(Wrong PIN → `finalize` throws `EscrowFfiError` (AEAD failure) → caller shows "incorrect PIN, try again"; `escrowDestroyed` → terminal.)

- [ ] **Step 6 (client commit):**
```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Networking/AccountService.swift ios/AINMMessenger/AINMMessenger/Crypto/EscrowClient.swift
rtk proxy git commit -m "feat(ios): EscrowClient enroll/recover via FFI + recover/params"
```

---

### Task 8: `BiometricManager` (FaceID gate)

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/Crypto/BiometricManager.swift`

- [ ] **Step 1: Write it** (LocalAuthentication; local unlock gate only — never replaces the key):

```swift
import LocalAuthentication

struct BiometricManager {
    enum BioError: Error, LocalizedError { case unavailable, failed
        var errorDescription: String? { self == .unavailable ? "Biometrics unavailable" : "Authentication failed" } }

    static func isAvailable() -> Bool {
        var e: NSError?
        return LAContext().canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &e)
    }

    /// Prompt FaceID/TouchID. Returns when authenticated; throws on failure.
    static func authenticate(reason: String = "Unlock AINM") async throws {
        let ctx = LAContext()
        var e: NSError?
        guard ctx.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &e) else { throw BioError.unavailable }
        let ok = try await ctx.evaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, localizedReason: reason)
        if !ok { throw BioError.failed }
    }
}
```

- [ ] **Step 2:** Add `NSFaceIDUsageDescription` to the app `Info.plist` (e.g. "AINM uses Face ID to unlock your account on this device."). Verify build.

- [ ] **Step 3: Commit**
```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Crypto/BiometricManager.swift ios/AINMMessenger/AINMMessenger/Info.plist
rtk proxy git commit -m "feat(ios): BiometricManager FaceID local-unlock gate"
```

---

## WAVE 3 — Onboarding + recovery UI + AppState wiring

### Task 9: AppState account actions

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/App/AppState.swift`

- [ ] **Step 1: Add onboarding state + actions.** Add an `enum OnboardingPhase { case none, signup, recovery }` and an `@Observable`-tracked property, plus actions that compose the services:

```swift
// In AppState (an @Observable / actor-isolated model — match the existing style):
private(set) var onboardingError: String?

/// Full signup: signup -> (user enters code) -> verify -> bind key -> enroll escrow -> active -> connect.
func completeSignup(serverURL: String, email: String, username: String, code: String, pin: String) async -> Bool {
    do {
        let base = URL(string: serverURL)!
        let account = AccountService(baseURL: base)
        let token = try await account.verify(email: email, code: code)   // assumes signup() already called to send code
        let kp = try await keyManager.getOrCreateKeypair()
        try await account.bindKey(enrollmentToken: token, publicKeyB64: kp.publicKeyBase64url, deviceLabel: UIDevice.current.name)
        try await EscrowClient(account: account, keyManager: keyManager).enrollAndUpload(enrollmentToken: token, pin: pin)
        await connect()  // handshake now succeeds against the bound, active account
        return connectionState == .connected
    } catch { onboardingError = (error as? LocalizedError)?.errorDescription ?? "\(error)"; return false }
}

/// Recovery on a fresh device: recover/start -> verify -> (PIN loop) -> restore key -> connect.
func completeRecovery(serverURL: String, email: String, code: String, pin: String) async -> Bool {
    do {
        let account = AccountService(baseURL: URL(string: serverURL)!)
        let token = try await account.recoverVerify(email: email, code: code)
        let ok = try await EscrowClient(account: account, keyManager: keyManager).recover(recoveryToken: token, pin: pin)
        guard ok else { return false }
        await connect()
        return connectionState == .connected
    } catch { onboardingError = (error as? LocalizedError)?.errorDescription ?? "\(error)"; return false }
}
```

(Adapt to the actual `AppState` shape — it already owns `keyManager`/`client`/`connect()`. The signup-code-send (`account.signup`) is triggered from the first onboarding screen before the code screen.)

- [ ] **Step 2: Build.** Commit:
```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/App/AppState.swift
rtk proxy git commit -m "feat(ios): AppState signup/recovery orchestration"
```

---

### Task 10: Onboarding + recovery SwiftUI flow

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/UI/Onboarding/OnboardingFlowView.swift` (+ step subviews), `RecoveryFlowView.swift`
- Modify: `ios/AINMMessenger/AINMMessenger/App/AINMMessengerApp.swift`, possibly remove/replace the old single-screen `OnboardingView.swift`

- [ ] **Step 1: Multi-step signup view** — a `NavigationStack`-driven flow: Server URL → Email (calls `account.signup`, advances) → Code → Username → PIN (+ confirm) → enable Face ID → spinner while `completeSignup` runs. Use `@State` step enum and bind fields. Include a "Recover an existing account" entry that pushes `RecoveryFlowView`.

```swift
struct OnboardingFlowView: View {
    @Bindable var appState: AppState
    @State private var step: Step = .server
    enum Step { case server, email, code, username, pin, faceID, working }
    @State private var serverURL = "ws://localhost:8080"
    @State private var email = "", code = "", username = "", pin = "", pinConfirm = ""
    // ... per-step subviews; on PIN submit -> step = .faceID -> BiometricManager.authenticate (optional) -> step = .working -> appState.completeSignup(...)
    var body: some View { /* switch step { ... } with Continue buttons + error display from appState.onboardingError */ }
}
```

(Write the full view with each step as a small subview. Keep it functional, not pixel-perfect — text fields, Continue buttons, a progress view, and `appState.onboardingError` display. Reuse the styling idioms from the existing `OnboardingView.swift`.)

- [ ] **Step 2: Recovery view** — Server URL → Email (calls `account.recoverStart`) → Code → PIN (loop: wrong PIN re-prompts; `escrowDestroyed` shows a terminal message) → spinner while `completeRecovery` runs.

- [ ] **Step 3: Route in `AINMMessengerApp.swift`** — replace `OnboardingView(appState:)` in the `default:` branch of the `connectionState` switch with `OnboardingFlowView(appState: appState)`.

- [ ] **Step 4: Build** (`xcodebuild ... build`). Commit:
```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/Onboarding ios/AINMMessenger/AINMMessenger/App/AINMMessengerApp.swift
rtk proxy git rm ios/AINMMessenger/AINMMessenger/UI/OnboardingView.swift 2>/dev/null || true
rtk proxy git commit -m "feat(ios): multi-step signup + recovery onboarding flow"
```

---

## WAVE 4 — End-to-end against a dev server (human-run)

### Task 11: Live E2E + interop verification

**This wave requires running the dev server + the app in a simulator. A subagent should prepare the steps; the human runs them.**

- [ ] **Step 1: Start a dev server with the mock email sender** (codes printed to the log). From `server/`:
```bash
AINM_DATABASE__URL=postgres://ainm:dev_password@localhost:5432/ainm \
AINM_TOKEN_SECRET=dev-e2e-secret \
cargo run -p ainm-server -- <federation flags as needed for the /ws + REST server>
```
(Confirm the server binds the account REST routes + `/ws`. The MockEmailSender logs `[email:mock] code <CODE> -> <email>`.)

- [ ] **Step 2: Signup E2E** — run the app in the simulator, go through signup (read the OTP from the server log), set a PIN. Assert: account becomes `active` (DB: `SELECT status FROM accounts WHERE username=...`), the WebSocket handshake **succeeds** (it now verifies the bound key + timestamp), and the app reaches the connected state.

- [ ] **Step 3: Recovery E2E** — delete the app's Keychain entry (fresh device sim: erase the simulator or delete the app), run recovery with the same email + PIN. Assert: the key is restored (the recovered `publicKeyBase64url` equals the original bound key), the handshake succeeds again. Then assert wrong-PIN behavior (re-prompt) and, after exhausting `max_attempts`, the `410`/destroyed terminal message.

- [ ] **Step 4: Interop assertion (the critical one)** — confirm the FFI client and Rust server agree: a key escrowed by the iOS app (Task 2's `escrowEnroll` via FFI) is recoverable through the live server's `/accounts/escrow/fetch`. (This is the wire-compat proof the whole FFI approach exists to guarantee.) If recovery returns the original key bytes, the contract holds.

- [ ] **Step 5: Document the result** in `docs/superpowers/notes/2026-06-01-ios-accounts-e2e.md` (what was tested, screenshots/logs, any deviations) and commit.

---

## Definition of Done (this plan → spec §10 / DoD #6)

1. ✅ Shared `ainm-escrow` crate; server uses it; server tests still green (Task 1).
2. ✅ `ainm-escrow-ffi` UniFFI crate with a passing Rust round-trip through the exported API (Task 2).
3. ✅ `AinmEscrow.xcframework` builds and links into the app; Swift bindings generated (Task 3).
4. ✅ iOS handshake sends `timestamp` (Task 4) — the app can connect to the enforced server.
5. ✅ `AccountService` covers all account endpoints incl. the new `recover/params` (Tasks 5, 7a).
6. ✅ `KeyManager` export/import; `EscrowClient` enroll/recover via FFI; `BiometricManager` (Tasks 6, 7, 8).
7. ✅ Multi-step signup + recovery SwiftUI flow wired into the app (Tasks 9, 10).
8. ✅ Live E2E: a real person signs up, connects (bound-key handshake succeeds), and recovers on a fresh device; FFI↔server interop confirmed (Task 11).

## Notes / known follow-ups (not in this plan)
- `server_signed_attestation` on `.well-known` still deferred (`TODO(B)`).
- Multi-device QR, key rotation + 72h revert, hardware-enclave escrow, passkeys — still out (spec §11).
- Decide framework-in-git vs CI-built (Task 3 Step 5) with the team; add the Rust→XCFramework step to iOS CI.
- The new `recover/params` endpoint (Task 7a) is a small server addition surfaced by the recovery ordering; it's additive and backward-compatible.
- **Naming cleanup:** `ios/.../Models/JsonRpc.swift` and `JsonRpcTests.swift` contain a hardcoded `"ainm-demo"` string (left untouched by the server rename). While in those files for Task 4, replace it with the product name (e.g. `"ainm-server"` or the appropriate client identifier) — confirm what the string represents first (likely a default server name/identifier), and never reintroduce "demo".
