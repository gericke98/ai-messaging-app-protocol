import Foundation

/// Bridges the UniFFI escrow crypto (`escrowEnroll` / `EscrowRecovery`) to the server
/// REST API (``AccountService``) and the local Keychain (``KeyManager``).
///
/// Enrollment seals the device's Ed25519 private key under a user PIN and uploads the
/// escrow blobs (enrollment-token authed). Recovery, on a fresh device, performs the
/// blinded OPRF round-trip for a single PIN guess and — on success — restores the key
/// into the Keychain.
///
/// The high-level FFI symbols (`escrowEnroll`, `EnrollResult`, `EscrowRecovery`) are
/// compiled into the app module from `Generated/ainm_escrow_ffi.swift`, so they are
/// referenced as top-level symbols with no extra import.
struct EscrowClient {
    let account: AccountService
    let keyManager: KeyManager

    /// Seal the device key under the PIN and upload the escrow blobs (enrollment-token authed).
    ///
    /// - The 32-byte Ed25519 seed is exported, sealed under the PIN-derived key by the FFI,
    ///   and the resulting ciphertext / KDF params / OPRF key are uploaded in one call.
    /// - `oprfKeyB64` is the server's secret OPRF key: uploaded once, NEVER persisted client-side.
    func enrollAndUpload(enrollmentToken: String, pin: String) async throws {
        let secret = try await keyManager.exportPrivateKeyBytes()              // Data, 32 bytes
        let result = try escrowEnroll(secret: secret, pin: pin)                // FFI (Data in)
        // result.kdfParamsJson is a JSON object string; decode into the typed KdfParams for upload.
        let kdf = try JSONDecoder().decode(
            AccountService.KdfParams.self,
            from: Data(result.kdfParamsJson.utf8)
        )
        try await account.storeEscrow(
            enrollmentToken: enrollmentToken,
            ciphertextB64: result.ciphertextB64,
            kdf: kdf,
            oprfKeyB64: result.oprfKeyB64
        )
        // NOTE: oprfKeyB64 is the server's secret OPRF key — uploaded once, NEVER persisted client-side.
    }

    /// Errors a caller distinguishes for UX.
    enum RecoverError: Error {
        /// The PIN was incorrect (AEAD open failed). The server already counted this attempt.
        case wrongPin
        /// The escrow blob is gone (too many failed attempts); recovery is no longer possible.
        case destroyed
    }

    /// Recovery: fetch kdf_params (free) → blind → fetch+evaluate (one attempt) → finalize → restore key.
    ///
    /// - Throws ``RecoverError/wrongPin`` if the PIN is incorrect (`finalize` AEAD open fails).
    /// - Throws ``RecoverError/destroyed`` if the blob is gone (server returns 410 Gone).
    func recover(recoveryToken: String, pin: String) async throws {
        let kdf = try await account.recoverParams(recoveryToken: recoveryToken)         // no-cost
        let kdfJson = String(decoding: try JSONEncoder().encode(kdf), as: UTF8.self)
        let rec = try EscrowRecovery.begin(pin: pin, kdfParamsJson: kdfJson)             // FFI: blind
        let blinded = try rec.blindedPoint()
        let resp: AccountService.EscrowFetchResp
        do {
            resp = try await account.escrowFetch(recoveryToken: recoveryToken, blindedPoint: blinded)
        } catch let e as AccountService.AccountError {
            if case .escrowDestroyed = e { throw RecoverError.destroyed }
            throw e
        }
        let secret: Data
        do {
            // FFI: wrong PIN → EscrowFfiError (AEAD open failed).
            secret = try rec.finalize(evaluatedPoint: resp.evaluated_point, ciphertextB64: resp.ciphertext_b64)
        } catch {
            // finalize failed = AEAD open failed = wrong PIN (the server already counted this attempt).
            throw RecoverError.wrongPin
        }
        try await keyManager.importKeypair(from: secret)                                  // restore into Keychain
    }
}
