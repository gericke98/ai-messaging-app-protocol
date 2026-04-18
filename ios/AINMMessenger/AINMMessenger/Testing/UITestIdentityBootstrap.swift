#if DEBUG
import CryptoKit
import Foundation

/// Installs a deterministic Ed25519 keypair for the `uat-user@localhost` identity
/// so every UAT run starts from the same cryptographic state.
///
/// Mirrors `KeyManager`'s Keychain contract (service="com.ainm.messenger",
/// account="ed25519.privateKey") — after `resetIdentity()` runs, the next call to
/// `KeyManager().getOrCreateKeypair()` loads this keypair instead of generating a new one.
///
/// Do NOT regenerate `uatSeedBytes` — fixture signatures and cross-run reproducibility
/// depend on the exact 32-byte literal below.
enum UITestIdentityBootstrap {

    /// 32-byte deterministic seed for the `uat-user@localhost` private key.
    /// Layout: ASCII "UAT_USER@LOCALHOST" + zero padding + 0xDEADBEEFFEEDFACE marker.
    /// Curve25519 signing keys are defined by any 32-byte scalar — CryptoKit's
    /// `Curve25519.Signing.PrivateKey(rawRepresentation:)` treats these bytes as the
    /// deterministic private scalar; the same bytes always produce the same public key.
    static let uatSeedBytes: [UInt8] = [
        0x55, 0x41, 0x54, 0x5F, 0x55, 0x53, 0x45, 0x52, // "UAT_USER"
        0x40, 0x4C, 0x4F, 0x43, 0x41, 0x4C, 0x48, 0x4F, // "@LOCALHO"
        0x53, 0x54, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // "ST" + zero padding
        0xDE, 0xAD, 0xBE, 0xEF, 0xFE, 0xED, 0xFA, 0xCE  // magic marker
    ]

    /// Logical identity address that the seeded keypair represents.
    /// Every scenario fixture sets `identity: "uat-user@localhost"` to match.
    static let identityAddress = "uat-user@localhost"

    enum BootstrapError: Swift.Error, CustomStringConvertible {
        case seedSizeInvalid(Int)

        var description: String {
            switch self {
            case .seedSizeInvalid(let n):
                return "UAT seed must be exactly 32 bytes for Curve25519.Signing.PrivateKey, got \(n)"
            }
        }
    }

    /// Deletes any existing keypair from Keychain and installs the deterministic one
    /// derived from `uatSeedBytes`. Idempotent — calling multiple times produces
    /// identical public keys (and identical signatures for the same message).
    ///
    /// Must run BEFORE `AppState.setup()` / `AINMClient.connect()` because the client
    /// reads the Keychain identity during setup.
    static func resetIdentity() async throws {
        guard uatSeedBytes.count == 32 else {
            throw BootstrapError.seedSizeInvalid(uatSeedBytes.count)
        }

        // Step 1: remove any existing keypair. KeyManager.deleteKeypair() handles
        // errSecItemNotFound as a no-op, so this is safe on a fresh simulator.
        let manager = KeyManager()
        try await manager.deleteKeypair()

        // Step 2: construct the deterministic private key from the fixed seed.
        let seed = Data(uatSeedBytes)
        let privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: seed)

        // Step 3: persist via the same Keychain service/account KeyManager uses for
        // load. KeychainStorage is a non-isolated Sendable struct nested inside the
        // KeyManager actor — its `save(_:)` method is safely callable from any
        // context. This guarantees `KeyManager().getOrCreateKeypair()` loads this
        // keypair on the next call instead of generating a fresh random one.
        let storage = KeyManager.KeychainStorage()
        try storage.save(privateKey.rawRepresentation)
    }

    /// Deterministic public key as base64 (raw representation, NOT base64url).
    /// Useful for unit-test assertions that want to verify signatures against a
    /// known-stable value without recomputing from the seed bytes at the call site.
    static var deterministicPublicKeyBase64: String {
        let seed = Data(uatSeedBytes)
        // Force-try is safe: 32 valid bytes always produce a valid Curve25519 private key.
        let privateKey = try! Curve25519.Signing.PrivateKey(rawRepresentation: seed)
        return privateKey.publicKey.rawRepresentation.base64EncodedString()
    }
}
#endif
