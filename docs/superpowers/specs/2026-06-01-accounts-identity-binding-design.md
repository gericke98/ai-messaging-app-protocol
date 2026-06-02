# Accounts & Identity Binding — Design

**Date:** 2026-06-01
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Milestone context:** First sub-project of the **hosted multi-tenant runtime** initiative. Decomposition
(recorded 2026-06-01): A = Accounts & identity binding (this doc) → B = Hosted agent runtime →
C = Per-user agent credentials & quota → D = Isolation/sandboxing → E = Deployment/HA → F = Onboarding UX.
A is first because every later piece needs a real, non-spoofable account identity, and the spoofable-identity
hole is the current blocker to letting strangers in.

**Research basis:** Four parallel research streams (2025-2026 best practices) on (1) Telegram's
agent/bot ease, (2) modern consumer auth UX + passkeys, (3) agent-platform onboarding, (4) crypto-identity
prior art (Nostr / AT Protocol / Matrix / Signal). Key adopted conclusions: keep Ed25519 device key as the
identity anchor; bind handle↔key the AT-Protocol way; recover via Signal-SVR-style PIN + zero-knowledge
server escrow; defer multi-device/rotation/passkeys/hardware-enclave.

## 1. Goal

Give AINM real, non-spoofable user accounts so strangers can sign up "Telegram-easy," and **bind each
account's human handle to its Ed25519 public key** so the server stops trusting any key a client hands it.
v1 ships: email-code signup, handle↔key binding, **anti-spoof handshake enforcement**, PIN + zero-knowledge
key-escrow recovery (software rate-limiting), and the minimal iOS signup/recovery flow so a real person can
create and recover an account end-to-end.

## 2. What exists today / the gap

(From a codebase survey, file:line evidence.)

- **Identity = BYO Ed25519 keypair, no accounts.** `identity.rs` holds address + keypair only;
  `IdentityStore` is in-memory and non-persistent. iOS generates the keypair in Keychain
  (`KeyManager.swift`).
- **Handshake trusts the client-supplied key.** On `ainm.initialize` the server verifies the signature
  against the public key *the client provides* (`signing.rs::verify_init`, `server.rs:451`), with no check
  that the address↔key binding is registered. **Anyone can claim any address by generating a keypair.**
  This is the core security hole.
- **Data is already `user_addr`-scoped** (every query is `WHERE user_addr = $1`: `repository.rs`,
  `migrations/00x`), so the storage model is tenant-shaped — it just rests on spoofable identity.
- **No email/verification, no per-user secrets, no recovery.** LLM keys are a single global env var
  (`main.rs:443`). Connection/registry state is in-memory only.

**The gap this closes:** account lifecycle (signup → verify → active), a persisted handle↔key binding,
server-side enforcement of that binding on connect, and a recovery path that survives device loss without
the server learning the private key.

## 3. Decisions locked (brainstorming)

- **Signup anchor = email + 6-digit code.** Federation-friendly, doubles as the recovery channel, avoids
  SIM-swap/phone-privacy/SMS-deprecation problems. Phone and Sign-in-with-Apple are explicitly **not** v1.
- **Recovery = PIN + zero-knowledge server escrow** (Signal SVR shape). Server stores the Ed25519 private
  key **encrypted**, plus a server-held secret share released only on a correct PIN under a rate limit; the
  server never sees the plaintext key. (Shift from "key never leaves device" to "only an unreadable
  ciphertext, undecryptable without a server-gated share, leaves the device.")
- **Escrow rigor v1 = software rate-limiting**, labeled honestly; hardware enclave (SGX/Nitro) is a later
  hardening pass.
- **Scope = server + minimal iOS flow** (a real person can sign up / recover end-to-end). Polished
  onboarding (agent picking, consent UI) stays in sub-project F.
- **Handle↔key binding = AT-Protocol bidirectional style**, server-enforced and mandatory (not optional like
  Nostr's NIP-05).

## 4. Identity model & addressing

- The **Ed25519 device key remains the cryptographic identity**; the account layer binds a handle + recovery
  to it.
- **Address = `username@<server-domain>`** (e.g. `santi@ainm.app`). **Username is public; email is private**
  (stored only as a hash, used for verification/recovery + duplicate detection).
- Username rules: case-insensitive unique per server; `[a-z0-9_]`, 3–30 chars; a reserved-name blocklist
  (e.g. `admin`, `system`, `compiler`, `support`).
- Face ID / biometric is the **local app-unlock gate** only; it never substitutes for the key.

## 5. Data model — new migration `003_accounts.sql`

- **`accounts`**: `id UUID PK`, `username TEXT` (unique, citext-style lower), `domain TEXT`,
  `address TEXT` (unique, = `username@domain`), `email_hash TEXT` (unique), `email_verified BOOL`,
  `status TEXT` (`pending` | `active` | `locked`), `created_at`, `updated_at`.
- **`account_keys`**: `id`, `account_id FK`, `public_key TEXT` (base64/hex of the Ed25519 pubkey, unique),
  `device_label TEXT`, `enrolled_at`, `revoked_at TIMESTAMPTZ NULL`. Multi-key-ready; **v1 enrolls exactly
  one active key per account**.
- **`email_verifications`**: `id`, `email_hash`, `code_hash` (hash of the OTP), `purpose TEXT`
  (`signup` | `recovery`), `expires_at`, `attempts INT`, `consumed_at NULL`.
- **`key_escrow`** (1:1 with account): `account_id FK`, `ciphertext BYTEA` (OPRF-derived-key-wrapped Ed25519
  private key), `kdf_params JSONB` (Argon2id salt + cost params), `oprf_key BYTEA` (the per-account
  Ristretto255 OPRF secret — see §6), `eval_attempts INT DEFAULT 0`, `max_attempts INT DEFAULT 5`,
  `destroyed BOOL DEFAULT false`, `updated_at`.

Existing `user_addr` columns continue to hold `accounts.address`. Data is fresh, so no backfill/FK migration
is required in v1 (a real FK can be added in a later hardening pass).

## 6. Escrow cryptography (the offline-brute-force-resistant core)

A naive "encrypt the private key with a PIN-derived key and upload it" is **offline-brute-forceable** — PINs
are low entropy, so an attacker who obtains the ciphertext (e.g. via email-account takeover) could crack it
locally. A "server stores a `pin_verifier` and releases a secret share on a correct PIN" scheme is **also
broken**: a low-entropy `pin_verifier` is itself offline-brute-forceable by the server / a DB thief, and any
PIN-derived value the client sends to prove a guess would let the server reconstruct the wrapping key. The
correct primitive (the one Signal SVR3 uses) is an **oblivious pseudorandom function (OPRF)**:

- The server holds a **per-account OPRF secret `oprf_key`** (random Ristretto255 scalar, generated at
  enrollment). The wrapping key is derived by evaluating the OPRF on the PIN: the client computes
  `k_pin = Argon2id(PIN, kdf_params)`, runs the **blinded** OPRF with the server, and gets `oprf_output`;
  **wrapping_key = HKDF(oprf_output)**. The server sees only a blinded point — it learns **neither the PIN
  nor the wrapping key**.
- **Enrollment:** client runs the OPRF once, derives `wrapping_key`, encrypts the Ed25519 private key
  (XChaCha20-Poly1305 or AES-GCM), uploads `ciphertext` + `kdf_params`. Server stores `ciphertext` +
  `oprf_key` + `eval_attempts = 0`.
- **Recovery:** each PIN guess **requires one server OPRF evaluation**, so the server **increments
  `eval_attempts` on every evaluation** (it doesn't even need to know whether the guess was right — a wrong
  PIN simply yields a wrapping key that fails to decrypt the ciphertext client-side). The server returns the
  `ciphertext` alongside the evaluation; the client unblinds, derives the wrapping key, and attempts
  decryption, restoring the **same** Ed25519 key into Keychain on success. On reaching `max_attempts` →
  **delete `oprf_key` and set `destroyed = true`** (the ciphertext becomes permanently undecryptable; fail
  closed, never leak).

Properties: (1) **no offline brute force** — deriving the wrapping key requires a server OPRF evaluation, so
every guess is server-gated and counted; (2) **zero-knowledge** — the server holds only ciphertext + an OPRF
key + a counter, and never observes the PIN or the wrapping key, even during recovery; (3) **fail closed** —
exhaustion destroys the OPRF key.

> **v1 honesty caveat:** the `eval_attempts` counter is enforced in application code, so a malicious server
> operator could refuse to increment/delete and thereby grant unlimited *online* OPRF evaluations — i.e.
> brute-force a low-entropy PIN (≤10⁶ for 6 digits) against its own server. It still cannot brute-force
> offline and never sees the key. This residual online-guessing risk is exactly what the later
> **hardware-enclave** hardening removes (tamper-proof counter). Mitigations available now: offer an
> alphanumeric passphrase instead of a 6-digit PIN; keep `max_attempts` low. Documented, not hidden.

## 7. Flows

### 7.1 Signup / enrollment

1. App → `POST /accounts/signup {email, username}`. Server validates username (rules + uniqueness) and email
   (basic format; optional disposable-domain blocklist), creates a `pending` account, stores `email_hash`,
   generates a 6-digit code, stores `code_hash` (purpose=`signup`, short TTL e.g. 10 min), emails the code.
   Username collision → `409`; caller picks another.
2. App → `POST /accounts/verify {email, code}`. Server checks `code_hash` + expiry + attempts; on success
   marks `email_verified` and returns a short-lived **`enrollment_token`** (e.g. 15 min, single-purpose).
3. App generates/loads the Ed25519 key, → `POST /accounts/keys {public_key, device_label}` (auth:
   `enrollment_token`). Server inserts the `account_keys` row → **binding established**.
4. App sets a PIN and runs the enrollment OPRF (§6) to derive the wrapping key, encrypts the private key, →
   `POST /accounts/escrow {ciphertext, kdf_params}` (auth: `enrollment_token`; the server generated and
   stored `oprf_key` during the enrollment OPRF exchange). Server finalizes `key_escrow`, sets account
   `active`.

### 7.2 Connect (ongoing)

App opens the WebSocket and signs `ainm.initialize` with the device key, exactly as today — but the server
now **enforces the binding** (§8). No new transport.

### 7.3 Recovery (new device / lost phone)

1. App → `POST /accounts/recover/start {email}` → server emails a code (purpose=`recovery`).
2. App → `POST /accounts/recover/verify {email, code}` → returns a short-lived **`recovery_token`**.
3. App → `POST /accounts/escrow/fetch {blinded_point}` (auth: `recovery_token`) — one **blinded OPRF
   evaluation** per PIN guess (§6). Server increments `eval_attempts`, evaluates with `oprf_key`, and returns
   `{evaluated_point, ciphertext, kdf_params}`; the app unblinds → derives the wrapping key → attempts
   decryption. Correct PIN → key restored into Keychain, logged in; wrong PIN → decryption fails locally and
   the user retries. Reaching `max_attempts` → server deletes `oprf_key`, `destroyed = true`, returns
   `410 Gone` with a clear "recovery blob destroyed" message on subsequent attempts.

## 8. Handshake enforcement (the security fix)

On `ainm.initialize`:
1. Parse the claimed `address` + signature + public key.
2. **Look up the account by `address`.** Unknown → reject (`-32xxx` auth error, close connection).
3. **Verify the signature against a registered, non-revoked `account_keys` row** for that account — *not*
   against the client-supplied key blob. Mismatch / no active key → reject.
4. Account `status != active` → reject.

This is the change that makes multi-tenancy real: a client can no longer assert an identity it hasn't been
bound to. (`signing.rs::verify_init` is extended to take the registered key from the DB; `server.rs` upgrade
handler does the lookup before accepting.)

## 9. Server module boundaries

- **`accounts.rs`** (new) — account lifecycle (signup/verify/activate), key binding, recovery orchestration.
  Pure logic split from DB calls where practical for unit-testing.
- **`escrow.rs`** (new, or a submodule of `accounts.rs`) — the Ristretto255 OPRF (blinded evaluation with the
  per-account `oprf_key`), `eval_attempts` rate-limit + `oprf_key` destruction. Pure and unit-testable;
  built on an audited curve lib (e.g. `curve25519-dalek` / a `voprf` crate).
- **`email.rs`** (new) — `trait EmailSender { async fn send_code(...) }`, with a real provider impl
  (HTTP to Resend/SES/Postmark — provider chosen at build time) and a **`MockEmailSender` that records/logs
  the code** for dev + tests (mirrors the `LlmClient` / `MockLlmClient` pattern).
- **`server.rs`** — new REST/RPC routes for the §7 endpoints; handshake enforcement in the WS upgrade path.
- **`.well-known` endpoint** — `GET /.well-known/ainm-identity/<username>` returns
  `{address, public_keys[], server_signed_attestation}` (server signs with its existing federation identity
  key). Federation-ready; cross-server resolution is **not** tested in v1.

## 10. iOS minimal flow (SwiftUI)

- **Onboarding screens:** email entry → code entry → choose username → set PIN → enable Face ID. Reuses
  `KeyManager` for the keypair; adds Argon2id (an Argon2 dependency — not in CryptoKit) + a **Ristretto255
  OPRF client** (blind/unblind; via a small Swift crypto dependency, since CryptoKit doesn't expose
  Ristretto OPRF) + the escrow encrypt/upload. *(Dependency choice is a v1 implementation risk — see §13.)*
- **Recovery screens:** email → code → enter PIN → key restored into Keychain.
- **`AINMClient`** handshake unchanged on the wire; it now succeeds against the enforced binding. New REST
  calls for the §7 endpoints.
- Polished onboarding (agent discovery, consent setup) is **out** (sub-project F).

## 11. Out of scope (v1)

Multi-device QR enrollment; key rotation + 72-hour revert window; hardware-enclave escrow; passkeys;
phone / Sign-in-with-Apple; agent identity & tokens (sub-projects B/C); cross-server account federation;
billing/quota (sub-project C). A real `user_addr` → `accounts` FK constraint (later hardening).

## 12. Testing

- **Unit (Rust):** username validation + reserved names; OTP generation/expiry/attempt-cap; OPRF
  round-trip (blinded eval → unblind → wrapping key matches enrollment) + `eval_attempts` increment +
  `oprf_key` destruction-on-exhaustion; attestation signing.
- **Integration (DB-backed, `demo/tests/`):**
  1. Full happy path: signup → verify → enroll key → store escrow → `active` → WS connect succeeds.
  2. Recovery happy path: recover/start → verify → escrow/fetch with correct PIN → key bytes returned match
     the originally escrowed key.
  3. **Wrong-PIN exhaustion:** `max_attempts` OPRF evaluations → `oprf_key` deleted, `destroyed = true`,
     subsequent fetch fails closed (`410`).
  4. **Spoof rejected:** bind key A to `alice`; attempt `ainm.initialize` as `alice` signed by key B →
     rejected.
  5. **Unknown address rejected:** connect as an unregistered address → rejected.
  6. Username collision → second signup with same username fails; email already-registered handled.
- **iOS:** escrow encrypt/decrypt round-trip unit test (key bytes survive a PIN+share round-trip); the
  onboarding + recovery flow wired against a dev server with `MockEmailSender`.

## 13. Risks

- **Email anti-spam is weaker than phone** (disposable domains). Mitigation: optional disposable-domain
  blocklist; revisit if abused. Not a v1 blocker.
- **Software rate-limiting is operator-trusted** — a malicious server could tamper with the guess counter.
  Documented honestly (§6 caveat); hardware enclave is the planned hardening.
- **Email-account takeover → recovery initiation**, but the PIN (server-gated, rate-limited) still stands
  between an attacker and the key; exhaustion destroys the blob. Acceptable for v1; future MFA can layer on.
- **Large slice.** The implementation plan will sequence it in waves: (1) schema + account lifecycle + email,
  (2) handshake binding enforcement, (3) escrow + recovery, (4) minimal iOS flow.
- **OPRF dependency on both sides.** Rust needs a Ristretto255 VOPRF (e.g. `voprf`/`curve25519-dalek`) and
  iOS needs a matching client; the two must be wire-compatible. Mitigation: pick a spec-conformant
  implementation (RFC 9497 OPRF(ristretto255, SHA-512)) on both ends and add an interop round-trip test
  early. If a wire-compatible iOS lib proves painful, fall back for v1 to documenting the residual risk and a
  simpler construction — but the OPRF is strongly preferred and is the planned path.

## 14. Definition of done

1. New migration creates `accounts`, `account_keys`, `email_verifications`, `key_escrow`.
2. Signup → email-code → verify → key enrollment → escrow → `active` works end-to-end (integration test).
3. Handshake **rejects unknown addresses and key/address mismatches**, accepts a correctly bound key
   (integration tests for both directions).
4. Recovery restores the original Ed25519 key on a correct PIN; wrong-PIN exhaustion destroys the escrow and
   fails closed (integration tests).
5. `email.rs` is trait-based with a mock used in tests; no real emails sent in CI.
6. Minimal iOS signup + recovery flow lets a real person create and recover an account against a dev server.
7. Full server suite green except the two known pre-existing failures (`test_graph_traversal`,
   `test_compilation_dedup_updates_confidence`).
