import CryptoKit
import Foundation

/// Manages Ed25519 keypair lifecycle using Apple CryptoKit with iOS Keychain storage.
///
/// Generates keypairs via `Curve25519.Signing.PrivateKey`, persists the raw private key
/// bytes in the Keychain using `kSecClassGenericPassword`, and retrieves them across
/// app restarts. Conforms to actor isolation for Swift 6 strict concurrency safety.
///
/// Keychain configuration:
/// - Service: "com.ainm.messenger"
/// - Account: "ed25519.privateKey"
/// - Accessibility: `kSecAttrAccessibleAfterFirstUnlock` (survives background refresh)
actor KeyManager {

    // MARK: - Constants

    private static let keychainService = "com.ainm.messenger"
    private static let keychainAccount = "ed25519.privateKey"

    // MARK: - Types

    struct KeyPairInfo: Sendable {
        let privateKey: Curve25519.Signing.PrivateKey
        let publicKey: Curve25519.Signing.PublicKey
        let publicKeyBase64url: String
    }

    enum KeyManagerError: Error, Sendable {
        case keychainSaveFailed(OSStatus)
        case keychainLoadFailed(OSStatus)
        case keychainDeleteFailed(OSStatus)
        case invalidKeyData
    }

    // MARK: - Storage Protocol (enables testing with in-memory backend)

    /// Abstraction over Keychain storage to support in-memory testing.
    protocol KeyStorage: Sendable {
        func save(_ data: Data) throws
        func load() throws -> Data?
        func delete() throws
    }

    // MARK: - Keychain Implementation

    /// Production Keychain storage using Security framework directly.
    struct KeychainStorage: KeyStorage, Sendable {
        func save(_ data: Data) throws {
            // Delete any existing item first to avoid errSecDuplicateItem
            let deleteQuery: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: KeyManager.keychainService,
                kSecAttrAccount as String: KeyManager.keychainAccount,
            ]
            SecItemDelete(deleteQuery as CFDictionary)

            let addQuery: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: KeyManager.keychainService,
                kSecAttrAccount as String: KeyManager.keychainAccount,
                kSecValueData as String: data,
                kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock,
            ]
            let status = SecItemAdd(addQuery as CFDictionary, nil)
            guard status == errSecSuccess else {
                throw KeyManagerError.keychainSaveFailed(status)
            }
        }

        func load() throws -> Data? {
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: KeyManager.keychainService,
                kSecAttrAccount as String: KeyManager.keychainAccount,
                kSecReturnData as String: true,
                kSecMatchLimit as String: kSecMatchLimitOne,
            ]
            var result: AnyObject?
            let status = SecItemCopyMatching(query as CFDictionary, &result)

            if status == errSecItemNotFound {
                return nil
            }
            guard status == errSecSuccess else {
                throw KeyManagerError.keychainLoadFailed(status)
            }
            return result as? Data
        }

        func delete() throws {
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: KeyManager.keychainService,
                kSecAttrAccount as String: KeyManager.keychainAccount,
            ]
            let status = SecItemDelete(query as CFDictionary)
            guard status == errSecSuccess || status == errSecItemNotFound else {
                throw KeyManagerError.keychainDeleteFailed(status)
            }
        }
    }

    // MARK: - In-Memory Storage (for unit tests)

    /// Thread-safe in-memory storage for unit testing without Keychain access.
    final class InMemoryStorage: KeyStorage, @unchecked Sendable {
        private let lock = NSLock()
        private var stored: Data?

        func save(_ data: Data) throws {
            lock.lock()
            defer { lock.unlock() }
            stored = data
        }

        func load() throws -> Data? {
            lock.lock()
            defer { lock.unlock() }
            return stored
        }

        func delete() throws {
            lock.lock()
            defer { lock.unlock() }
            stored = nil
        }
    }

    // MARK: - Properties

    private let storage: KeyStorage

    // MARK: - Initialization

    /// Create a KeyManager with the default Keychain storage.
    init() {
        self.storage = KeychainStorage()
    }

    /// Create a KeyManager with custom storage (for testing).
    init(storage: KeyStorage) {
        self.storage = storage
    }

    // MARK: - Public API

    /// Get existing keypair from Keychain, or generate a new one and store it.
    ///
    /// On first call, generates a fresh Ed25519 keypair and persists it.
    /// On subsequent calls, loads the existing keypair from storage.
    func getOrCreateKeypair() throws -> KeyPairInfo {
        if let existingData = try storage.load() {
            guard existingData.count == 32 else {
                throw KeyManagerError.invalidKeyData
            }
            let privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: existingData)
            return makeInfo(privateKey)
        }
        let privateKey = Curve25519.Signing.PrivateKey()
        try storage.save(privateKey.rawRepresentation)
        return makeInfo(privateKey)
    }

    /// Delete stored keypair (for key rotation or testing).
    func deleteKeypair() throws {
        try storage.delete()
    }

    // MARK: - Private Helpers

    private func makeInfo(_ pk: Curve25519.Signing.PrivateKey) -> KeyPairInfo {
        KeyPairInfo(
            privateKey: pk,
            publicKey: pk.publicKey,
            publicKeyBase64url: pk.publicKey.rawRepresentation.base64urlEncodedString()
        )
    }
}
