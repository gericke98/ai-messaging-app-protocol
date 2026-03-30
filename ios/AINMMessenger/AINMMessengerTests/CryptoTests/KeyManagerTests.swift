import CryptoKit
import Foundation
import Testing

@testable import AINMMessenger

/// Tests for KeyManager Ed25519 keypair generation and storage.
/// Uses InMemoryStorage to avoid Keychain dependency in unit tests.
struct KeyManagerTests {

    // MARK: - Keypair Generation

    @Test("Generate keypair returns valid KeyPairInfo with 32-byte public key")
    func testGenerateKeypair() async throws {
        let storage = KeyManager.InMemoryStorage()
        let manager = KeyManager(storage: storage)
        let info = try await manager.getOrCreateKeypair()

        // Ed25519 public key is always 32 bytes
        #expect(info.publicKey.rawRepresentation.count == 32)
        // Private key raw representation is 32 bytes (seed)
        #expect(info.privateKey.rawRepresentation.count == 32)
    }

    // MARK: - Persistence

    @Test("Second call returns same keypair (persistence)")
    func testPersistence() async throws {
        let storage = KeyManager.InMemoryStorage()
        let manager = KeyManager(storage: storage)

        let first = try await manager.getOrCreateKeypair()
        let second = try await manager.getOrCreateKeypair()

        #expect(first.publicKey.rawRepresentation == second.publicKey.rawRepresentation)
        #expect(first.publicKeyBase64url == second.publicKeyBase64url)
    }

    // MARK: - Delete and Regenerate

    @Test("Delete then regenerate produces different keypair")
    func testDeleteAndRegenerate() async throws {
        let storage = KeyManager.InMemoryStorage()
        let manager = KeyManager(storage: storage)

        let original = try await manager.getOrCreateKeypair()
        try await manager.deleteKeypair()
        let regenerated = try await manager.getOrCreateKeypair()

        // Extremely unlikely (1 in 2^256) to generate the same key
        #expect(original.publicKey.rawRepresentation != regenerated.publicKey.rawRepresentation)
    }

    // MARK: - Base64url Format

    @Test("Public key base64url has correct format: no +, /, = and length 43")
    func testPublicKeyBase64urlFormat() async throws {
        let storage = KeyManager.InMemoryStorage()
        let manager = KeyManager(storage: storage)
        let info = try await manager.getOrCreateKeypair()

        let b64url = info.publicKeyBase64url

        // 32 bytes -> ceil(32 * 4/3) = 43 base64url chars (no padding)
        #expect(b64url.count == 43)
        #expect(!b64url.contains("+"))
        #expect(!b64url.contains("/"))
        #expect(!b64url.contains("="))
    }

    // MARK: - Signing Capability

    @Test("Generated keypair can sign and verify data")
    func testKeypairSignsAndVerifies() async throws {
        let storage = KeyManager.InMemoryStorage()
        let manager = KeyManager(storage: storage)
        let info = try await manager.getOrCreateKeypair()

        let message = "test message".data(using: .utf8)!
        let signature = try info.privateKey.signature(for: message)
        let isValid = info.publicKey.isValidSignature(signature, for: message)

        #expect(isValid)
    }

    // MARK: - Invalid Data Handling

    @Test("Invalid stored key data throws error")
    func testInvalidKeyData() async throws {
        let storage = KeyManager.InMemoryStorage()
        // Store invalid data (wrong length)
        try storage.save(Data([0x01, 0x02, 0x03]))

        let manager = KeyManager(storage: storage)
        await #expect(throws: KeyManager.KeyManagerError.self) {
            try await manager.getOrCreateKeypair()
        }
    }
}
