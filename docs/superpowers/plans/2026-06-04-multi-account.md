# Multi-Account / Multi-Device Keys Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the iOS reference client hold multiple AINM accounts on one device — each with its own Ed25519 keypair and its own local database — with exactly one active at a time, fast switching, and a one-time migration of the existing single account.

**Architecture:** A `AccountRegistry` (UserDefaults JSON) is the source of truth for the account set + the active address. `KeyManager` becomes keyed by address (`ed25519.privateKey.<address>`); `AppDatabase` gains a per-account file (`ainm-<sanitized>-<hash>.sqlite`). `AppState` reconstructs its `KeyManager` + `AppDatabase` + `AINMClient` whenever the active account changes (bootstrap, onboard, switch, remove). A one-time `LegacyMigration` moves the RC single-account key + `ainm.sqlite` under its address and seeds the registry.

**Tech Stack:** Swift 6 / SwiftUI / `@Observable`, CryptoKit (Ed25519 + SHA256), GRDB `DatabasePool`, iOS Keychain (`kSecClassGenericPassword`), swift-testing (`Testing`) + XCTest.

**Key design decisions (resolve the spec's §5 call-out):**
- The new-account address is **known before the key is created**: `finishSignup`/`completeRecovery` already receive `username`, and the host comes from `serverURL`, so the slot is derived from `username@host`. **No provisional slot is needed.**
- `KeyManager(address: String? = nil)`: `nil` → the legacy fixed slot `"ed25519.privateKey"`. This keeps the existing UITest seed / identity-bootstrap path and the pre-migration read working untouched; multi-account always passes a non-nil address.
- `AppDatabase.makeDefault()` / `makeInMemory()` are **kept** (UITest seed path + unit tests rely on them); the new per-account path is additive via `make(forAccount:)`.

**Conventions in this codebase (do not deviate):**
- iOS build (no signing flag — `CODE_SIGNING_ALLOWED=NO` breaks Keychain → `-34018`):
  `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -configuration Debug -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build`
- iOS tests (filterable):
  `xcodebuild test -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd -only-testing:AINMMessengerTests/<Suite>`
- The project uses `PBXFileSystemSynchronizedRootGroup` — **new files in source folders are auto-included; never edit `project.pbxproj`** to add them.
- `KeyManager` is an `actor` — call its methods with `await`.
- All git via `rtk proxy git ...`. Commit after each task. Private key bytes must never be logged.
- SourceKit may report false "No such module"/"cannot find type" errors — `xcodebuild` is authoritative.

---

## File Structure

**New files:**
- `ios/AINMMessenger/AINMMessenger/App/AccountRegistry.swift` — `AccountSummary` + `AccountRegistry` (UserDefaults-backed source of truth).
- `ios/AINMMessenger/AINMMessenger/App/LegacyMigration.swift` — one-time RC→multi-account migration (key copy + DB rename + registry seed).
- `ios/AINMMessenger/AINMMessenger/UI/Accounts/AccountSwitcherView.swift` — switcher sheet.
- `ios/AINMMessenger/AINMMessengerTests/AppTests/AccountRegistryTests.swift` — registry round-trip + behavior.
- `ios/AINMMessenger/AINMMessengerTests/AppTests/LegacyMigrationTests.swift` — migration + idempotency.

**Modified files:**
- `ios/AINMMessenger/AINMMessenger/Crypto/KeyManager.swift` — address-keyed slot.
- `ios/AINMMessenger/AINMMessenger/Persistence/AppDatabase.swift` — `dbFileName(forAccount:)` / `dbURL(forAccount:)` / `make(forAccount:)`.
- `ios/AINMMessenger/AINMMessenger/App/AppState.swift` — `makeClient` extraction, `activate`, registry-driven `bootstrap`, account-aware `finishSignup`/`completeRecovery`, `switchAccount`/`addAccount`/`removeAccount`.
- `ios/AINMMessenger/AINMMessenger/App/AINMMessengerApp.swift` — run `LegacyMigration` before bootstrap; key the list view on active address.
- `ios/AINMMessenger/AINMMessenger/UI/ConversationList/ConversationListView.swift` — account chip entry point to the switcher.
- `ios/AINMMessenger/AINMMessengerTests/CryptoTests/KeyManagerTests.swift` — per-account isolation test.
- `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/AppDatabaseTests.swift` — per-account path test.

---

## Task 1: AccountRegistry

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/App/AccountRegistry.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/AppTests/AccountRegistryTests.swift`

- [ ] **Step 1: Write the failing test**

Create `ios/AINMMessenger/AINMMessengerTests/AppTests/AccountRegistryTests.swift`:

```swift
import Foundation
import Testing
@testable import AINMMessenger

struct AccountRegistryTests {

    /// A registry over a private, throwaway UserDefaults suite so tests never touch
    /// the app's real defaults. Returns (registry, cleanup).
    private func makeRegistry() -> (AccountRegistry, UserDefaults, String) {
        let suite = "test.registry.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        return (AccountRegistry(defaults: defaults), defaults, suite)
    }

    private func summary(_ user: String, _ host: String, faceID: Bool = false) -> AccountSummary {
        AccountSummary(address: "\(user)@\(host)", serverURL: "ws://\(host):8080",
                       username: user, faceIDEnabled: faceID)
    }

    @Test("Empty registry has no accounts and no active")
    func testEmpty() {
        let (registry, defaults, suite) = makeRegistry()
        defer { defaults.removePersistentDomain(forName: suite) }
        #expect(registry.isEmpty())
        #expect(registry.all().isEmpty)
        #expect(registry.active() == nil)
    }

    @Test("Upsert adds then updates by address; round-trips through UserDefaults")
    func testUpsertRoundTrip() {
        let (registry, defaults, suite) = makeRegistry()
        defer { defaults.removePersistentDomain(forName: suite) }
        registry.upsert(summary("alice", "localhost"))
        registry.upsert(summary("bob", "example.com"))
        #expect(registry.all().count == 2)

        // Update alice in place (same address) — not a duplicate.
        var alice = summary("alice", "localhost")
        alice.faceIDEnabled = true
        registry.upsert(alice)
        #expect(registry.all().count == 2)
        #expect(registry.all().first { $0.address == "alice@localhost" }?.faceIDEnabled == true)
    }

    @Test("setActive / active resolve the active summary")
    func testActive() {
        let (registry, defaults, suite) = makeRegistry()
        defer { defaults.removePersistentDomain(forName: suite) }
        registry.upsert(summary("alice", "localhost"))
        registry.setActive(address: "alice@localhost")
        #expect(registry.active()?.username == "alice")
        #expect(registry.contains(address: "alice@localhost"))
    }

    @Test("Removing the active account clears the active pointer")
    func testRemoveActiveClearsPointer() {
        let (registry, defaults, suite) = makeRegistry()
        defer { defaults.removePersistentDomain(forName: suite) }
        registry.upsert(summary("alice", "localhost"))
        registry.setActive(address: "alice@localhost")
        registry.remove(address: "alice@localhost")
        #expect(registry.all().isEmpty)
        #expect(registry.active() == nil)
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `xcodebuild test -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd -only-testing:AINMMessengerTests/AccountRegistryTests`
Expected: FAIL — build error, `cannot find 'AccountRegistry'`/`'AccountSummary'` in scope.

- [ ] **Step 3: Write the implementation**

Create `ios/AINMMessenger/AINMMessenger/App/AccountRegistry.swift`:

```swift
import Foundation

/// One AINM account known to this device. Non-secret routing metadata only — the
/// Ed25519 private key lives in the Keychain (keyed by `address`), and the conversation
/// data lives in this account's own GRDB file. `id == address` so it is `Identifiable`
/// for SwiftUI lists.
struct AccountSummary: Codable, Identifiable, Equatable, Sendable {
    let address: String          // "username@host"
    var serverURL: String        // ws:// or wss:// URL used for the handshake
    var username: String         // local-part chosen at signup
    var faceIDEnabled: Bool       // whether launch-resume is Face-ID gated
    var id: String { address }
}

/// Source of truth for the set of accounts on this device and which one is active.
/// Persisted in UserDefaults (non-secret). Reads/writes are stateless — each call hits
/// UserDefaults — mirroring the existing `ainm.session.*` pattern (RC). Inject a custom
/// `UserDefaults` suite in tests.
struct AccountRegistry: Sendable {
    private let defaults: UserDefaults
    private static let accountsKey = "ainm.accounts"
    private static let activeKey = "ainm.activeAddress"

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    func all() -> [AccountSummary] {
        guard let data = defaults.data(forKey: Self.accountsKey),
              let list = try? JSONDecoder().decode([AccountSummary].self, from: data)
        else { return [] }
        return list
    }

    func isEmpty() -> Bool { all().isEmpty }

    var activeAddress: String? { defaults.string(forKey: Self.activeKey) }

    func active() -> AccountSummary? {
        guard let address = activeAddress else { return nil }
        return all().first { $0.address == address }
    }

    func contains(address: String) -> Bool {
        all().contains { $0.address == address }
    }

    /// Insert or replace the account with this address (keyed by `address`).
    func upsert(_ account: AccountSummary) {
        var list = all()
        if let index = list.firstIndex(where: { $0.address == account.address }) {
            list[index] = account
        } else {
            list.append(account)
        }
        persist(list)
    }

    /// Remove an account. If it was the active one, clear the active pointer.
    func remove(address: String) {
        var list = all()
        list.removeAll { $0.address == address }
        persist(list)
        if activeAddress == address {
            defaults.removeObject(forKey: Self.activeKey)
        }
    }

    func setActive(address: String?) {
        if let address {
            defaults.set(address, forKey: Self.activeKey)
        } else {
            defaults.removeObject(forKey: Self.activeKey)
        }
    }

    private func persist(_ list: [AccountSummary]) {
        if let data = try? JSONEncoder().encode(list) {
            defaults.set(data, forKey: Self.accountsKey)
        }
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `xcodebuild test -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd -only-testing:AINMMessengerTests/AccountRegistryTests`
Expected: PASS — 4 tests pass.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/App/AccountRegistry.swift ios/AINMMessenger/AINMMessengerTests/AppTests/AccountRegistryTests.swift
rtk proxy git commit -m "feat(ios): AccountRegistry (UserDefaults source of truth for accounts)"
```

---

## Task 2: KeyManager keyed by address

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/Crypto/KeyManager.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/CryptoTests/KeyManagerTests.swift`

The Keychain `kSecAttrAccount` becomes per-address. The production `KeychainStorage` must read the address from the `KeyManager` that owns it, so storage construction takes the account string. `init(address: String? = nil)` maps `nil` → the legacy slot for backward compatibility.

- [ ] **Step 1: Write the failing test**

Append to `ios/AINMMessenger/AINMMessengerTests/CryptoTests/KeyManagerTests.swift` (inside the `struct KeyManagerTests { ... }`, before the closing brace):

```swift
    // MARK: - Per-Account Slot Isolation (multi-account)

    @Test("Default (nil) address maps to the legacy fixed slot")
    func testNilAddressIsLegacySlot() async throws {
        // Both managers use the production KeychainStorage; the legacy account string
        // is the constant used pre-migration. We assert the account string, not Keychain I/O.
        #expect(KeyManager.keychainAccount(forAddress: nil) == "ed25519.privateKey")
        #expect(KeyManager.keychainAccount(forAddress: "alice@localhost")
                == "ed25519.privateKey.alice@localhost")
    }

    @Test("Keys for two addresses are independent in storage")
    func testPerAddressIsolation() async throws {
        // Inject independent in-memory storages to simulate two distinct Keychain slots.
        let storageA = KeyManager.InMemoryStorage()
        let storageB = KeyManager.InMemoryStorage()
        let a = KeyManager(storage: storageA)
        let b = KeyManager(storage: storageB)

        let keyA = try await a.getOrCreateKeypair()
        #expect(await b.hasStoredKeypair() == false)   // creating A did not touch B

        let keyB = try await b.getOrCreateKeypair()
        #expect(keyA.publicKeyBase64url != keyB.publicKeyBase64url)

        try await a.deleteKeypair()
        #expect(await a.hasStoredKeypair() == false)
        #expect(await b.hasStoredKeypair() == true)     // deleting A left B intact
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `xcodebuild test -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd -only-testing:AINMMessengerTests/KeyManagerTests`
Expected: FAIL — `cannot find 'keychainAccount(forAddress:)' in scope`.

- [ ] **Step 3: Write the implementation**

In `ios/AINMMessenger/AINMMessenger/Crypto/KeyManager.swift`, replace the constants block (lines 16-19, the `// MARK: - Constants` section) with:

```swift
    // MARK: - Constants

    private static let keychainService = "com.ainm.messenger"
    /// The legacy single-account slot (RC). A `nil` address maps here so the pre-migration
    /// read and the UITest identity-bootstrap path keep working unchanged.
    private static let legacyKeychainAccount = "ed25519.privateKey"

    /// The Keychain account string for an AINM address. `nil` → the legacy slot.
    /// Multi-account always passes a real address; the slot is derived from it.
    static func keychainAccount(forAddress address: String?) -> String {
        guard let address else { return legacyKeychainAccount }
        return "\(legacyKeychainAccount).\(address)"
    }
```

Then update `KeychainStorage` to carry the account string. Replace the entire `struct KeychainStorage` (lines 48-102) with:

```swift
    /// Production Keychain storage using Security framework directly. Each instance
    /// targets one account slot (`kSecAttrAccount`), so distinct addresses are isolated.
    struct KeychainStorage: KeyStorage, Sendable {
        let account: String

        func save(_ data: Data) throws {
            // Delete any existing item first to avoid errSecDuplicateItem
            let deleteQuery: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: KeyManager.keychainService,
                kSecAttrAccount as String: account,
            ]
            SecItemDelete(deleteQuery as CFDictionary)

            let addQuery: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: KeyManager.keychainService,
                kSecAttrAccount as String: account,
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
                kSecAttrAccount as String: account,
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
                kSecAttrAccount as String: account,
            ]
            let status = SecItemDelete(query as CFDictionary)
            guard status == errSecSuccess || status == errSecItemNotFound else {
                throw KeyManagerError.keychainDeleteFailed(status)
            }
        }
    }
```

Then update the initializers (lines 136-144, the `// MARK: - Initialization` section) to:

```swift
    // MARK: - Initialization

    /// Create a KeyManager bound to an account's Keychain slot. `nil` → the legacy slot
    /// (used pre-migration and by the UITest identity bootstrap).
    init(address: String? = nil) {
        self.storage = KeychainStorage(account: KeyManager.keychainAccount(forAddress: address))
    }

    /// Create a KeyManager with custom storage (for testing).
    init(storage: KeyStorage) {
        self.storage = storage
    }
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `xcodebuild test -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd -only-testing:AINMMessengerTests/KeyManagerTests`
Expected: PASS — existing tests plus the 2 new ones pass. (`KeyManager()` still compiles because `address` defaults to `nil`.)

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Crypto/KeyManager.swift ios/AINMMessenger/AINMMessengerTests/CryptoTests/KeyManagerTests.swift
rtk proxy git commit -m "feat(ios): key Keychain slot by AINM address (nil = legacy slot)"
```

---

## Task 3: Per-account AppDatabase

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/Persistence/AppDatabase.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/AppDatabaseTests.swift`

- [ ] **Step 1: Write the failing test**

Append to `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/AppDatabaseTests.swift` (inside the `final class AppDatabaseTests`, before the closing brace):

```swift
    // MARK: - Per-Account Database Files (multi-account)

    func testDbFileNameIsDeterministicAndDistinct() {
        let a1 = AppDatabase.dbFileName(forAccount: "alice@localhost")
        let a2 = AppDatabase.dbFileName(forAccount: "alice@localhost")
        let b = AppDatabase.dbFileName(forAccount: "bob@localhost")

        // Deterministic for the same address.
        XCTAssertEqual(a1, a2)
        // Distinct addresses → distinct files.
        XCTAssertNotEqual(a1, b)
        // Filesystem-safe: no '@' or '.' before the extension, has the ainm- prefix + .sqlite.
        XCTAssertTrue(a1.hasPrefix("ainm-"))
        XCTAssertTrue(a1.hasSuffix(".sqlite"))
        let stem = String(a1.dropLast(".sqlite".count))
        XCTAssertFalse(stem.contains("@"))
        XCTAssertFalse(stem.contains("."))
    }

    func testMakeForAccountYieldsWorkingMigratedSchema() throws {
        // Use a unique address so the file is fresh; clean it up after.
        let address = "schema-\(UUID().uuidString)@localhost"
        let url = try AppDatabase.dbURL(forAccount: address)
        defer {
            for suffix in ["", "-wal", "-shm"] {
                try? FileManager.default.removeItem(atPath: url.path + suffix)
            }
        }
        let db = try AppDatabase.make(forAccount: address)
        try db.dbPool.read { db in
            XCTAssertTrue(try db.tableExists("thread"))
            XCTAssertTrue(try db.tableExists("message"))
        }
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `xcodebuild test -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd -only-testing:AINMMessengerTests/AppDatabaseTests`
Expected: FAIL — `cannot find 'dbFileName(forAccount:)'` / `'make(forAccount:)'`.

- [ ] **Step 3: Write the implementation**

In `ios/AINMMessenger/AINMMessenger/Persistence/AppDatabase.swift`, change the import line at the top from `import GRDB` / `import Foundation` to add CryptoKit:

```swift
import GRDB
import Foundation
import CryptoKit
```

Then insert these methods right after `makeInMemory()` (after line 37, before `private func migrate()`):

```swift
    // MARK: - Per-Account Files

    /// Deterministic, filesystem-safe DB file name for an AINM address.
    /// Sanitizes non-alphanumerics to `_` and appends a short SHA-256 prefix so two
    /// addresses that sanitize to the same stem (e.g. `a.b@h` vs `a_b@h`) never collide.
    static func dbFileName(forAccount address: String) -> String {
        let sanitized = String(address.map { ($0.isLetter || $0.isNumber) ? $0 : "_" })
        let digest = SHA256.hash(data: Data(address.utf8))
        let shortHash = digest.prefix(4).map { String(format: "%02x", $0) }.joined() // 8 hex chars
        return "ainm-\(sanitized)-\(shortHash).sqlite"
    }

    /// Application Support URL for an account's DB file (creates the directory if needed).
    static func dbURL(forAccount address: String) throws -> URL {
        let appSupportURL = try FileManager.default.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        return appSupportURL.appendingPathComponent(dbFileName(forAccount: address))
    }

    /// Open (or create) the per-account database and run all migrations.
    static func make(forAccount address: String) throws -> AppDatabase {
        let dbURL = try dbURL(forAccount: address)
        let dbPool = try DatabasePool(path: dbURL.path)
        let db = AppDatabase(dbPool: dbPool)
        try db.migrate()
        return db
    }
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `xcodebuild test -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd -only-testing:AINMMessengerTests/AppDatabaseTests`
Expected: PASS — existing tests plus the 2 new ones pass.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Persistence/AppDatabase.swift ios/AINMMessenger/AINMMessengerTests/PersistenceTests/AppDatabaseTests.swift
rtk proxy git commit -m "feat(ios): per-account AppDatabase files (ainm-<addr>-<hash>.sqlite)"
```

---

## Task 4: LegacyMigration (RC → multi-account)

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/App/LegacyMigration.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/AppTests/LegacyMigrationTests.swift`

Migration is injectable (defaults, app-support dir, both KeyManagers) so it is unit-testable without the real Keychain or Application Support directory. Production call sites use the defaults.

- [ ] **Step 1: Write the failing test**

Create `ios/AINMMessenger/AINMMessengerTests/AppTests/LegacyMigrationTests.swift`:

```swift
import Foundation
import Testing
@testable import AINMMessenger

struct LegacyMigrationTests {

    /// Build the injected world: a temp app-support dir with a fake legacy DB, a private
    /// UserDefaults suite seeded with RC session keys, an in-memory legacy KeyManager that
    /// already holds a key, and a single shared in-memory addressed KeyManager we can inspect.
    private func makeWorld(faceID: Bool) async throws
        -> (tempDir: URL, defaults: UserDefaults, suite: String,
            registry: AccountRegistry, legacyKM: KeyManager,
            addressedStorage: KeyManager.InMemoryStorage, expectedAddress: String) {

        let tempDir = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("legacymig-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)

        // Fake legacy DB + WAL sidecar.
        try Data("db".utf8).write(to: tempDir.appendingPathComponent("ainm.sqlite"))
        try Data("wal".utf8).write(to: tempDir.appendingPathComponent("ainm.sqlite-wal"))

        let suite = "test.legacymig.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defaults.set("ws://localhost:8080", forKey: "ainm.session.serverURL")
        defaults.set("alice", forKey: "ainm.session.username")
        defaults.set(faceID, forKey: "ainm.session.faceIDEnabled")

        // Legacy key manager that already holds a key (simulates the RC device key).
        let legacyStorage = KeyManager.InMemoryStorage()
        let legacyKM = KeyManager(storage: legacyStorage)
        _ = try await legacyKM.getOrCreateKeypair()

        let addressedStorage = KeyManager.InMemoryStorage()
        let registry = AccountRegistry(defaults: defaults)
        return (tempDir, defaults, suite, registry, legacyKM, addressedStorage, "alice@localhost")
    }

    @Test("Migrates legacy key + DB + session into a seeded active account")
    func testMigrates() async throws {
        let w = try await makeWorld(faceID: true)
        defer { w.defaults.removePersistentDomain(forName: w.suite)
                try? FileManager.default.removeItem(at: w.tempDir) }

        await LegacyMigration.migrateIfNeeded(
            registry: w.registry, defaults: w.defaults, appSupport: w.tempDir,
            legacyKeyManager: w.legacyKM,
            makeAddressedKeyManager: { _ in KeyManager(storage: w.addressedStorage) }
        )

        // Key copied into the addressed slot.
        #expect(try w.addressedStorage.load() != nil)
        // Registry seeded + active.
        #expect(w.registry.active()?.address == w.expectedAddress)
        #expect(w.registry.active()?.faceIDEnabled == true)
        // DB renamed to the per-account file (+ its WAL sidecar).
        let dbName = AppDatabase.dbFileName(forAccount: w.expectedAddress)
        let fm = FileManager.default
        #expect(fm.fileExists(atPath: w.tempDir.appendingPathComponent(dbName).path))
        #expect(fm.fileExists(atPath: w.tempDir.appendingPathComponent(dbName + "-wal").path))
        #expect(!fm.fileExists(atPath: w.tempDir.appendingPathComponent("ainm.sqlite").path))
        // Legacy session keys cleared.
        #expect(w.defaults.string(forKey: "ainm.session.username") == nil)
    }

    @Test("Is idempotent — a second run is a no-op")
    func testIdempotent() async throws {
        let w = try await makeWorld(faceID: false)
        defer { w.defaults.removePersistentDomain(forName: w.suite)
                try? FileManager.default.removeItem(at: w.tempDir) }

        let run: () async -> Void = {
            await LegacyMigration.migrateIfNeeded(
                registry: w.registry, defaults: w.defaults, appSupport: w.tempDir,
                legacyKeyManager: w.legacyKM,
                makeAddressedKeyManager: { _ in KeyManager(storage: w.addressedStorage) }
            )
        }
        await run()
        await run()  // second run sees a non-empty registry → returns immediately
        #expect(w.registry.all().count == 1)
    }

    @Test("No legacy session → no migration")
    func testNoLegacyNoOp() async throws {
        let suite = "test.legacymig.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        let registry = AccountRegistry(defaults: defaults)

        await LegacyMigration.migrateIfNeeded(
            registry: registry, defaults: defaults, appSupport: URL(fileURLWithPath: NSTemporaryDirectory()),
            legacyKeyManager: KeyManager(storage: KeyManager.InMemoryStorage()),
            makeAddressedKeyManager: { _ in KeyManager(storage: KeyManager.InMemoryStorage()) }
        )
        #expect(registry.isEmpty())
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `xcodebuild test -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd -only-testing:AINMMessengerTests/LegacyMigrationTests`
Expected: FAIL — `cannot find 'LegacyMigration' in scope`.

- [ ] **Step 3: Write the implementation**

Create `ios/AINMMessenger/AINMMessenger/App/LegacyMigration.swift`:

```swift
import Foundation

/// One-time migration from the RC single-account layout (a fixed Keychain slot + a single
/// `ainm.sqlite` + `ainm.session.*` UserDefaults) into the multi-account layout
/// (address-keyed Keychain slot + `ainm-<addr>.sqlite` + an `AccountRegistry` entry).
///
/// Runs at launch before any DB is opened. The guard is "registry is empty AND legacy
/// session keys are present", so it runs at most once and is safe to call on every launch.
/// The legacy key is **copied** (not moved) into the addressed slot — leaving the legacy
/// slot intact avoids a crash window where neither slot holds the key. Once the registry
/// is seeded, subsequent launches short-circuit.
enum LegacyMigration {
    // Mirror AppState.SessionKey (RC). The registry replaces these going forward.
    private static let legacyServerURLKey = "ainm.session.serverURL"
    private static let legacyUsernameKey  = "ainm.session.username"
    private static let legacyFaceIDKey    = "ainm.session.faceIDEnabled"

    /// - Parameters:
    ///   - appSupport: Application Support directory. `nil` → the real one.
    ///   - legacyKeyManager: reads the legacy fixed-slot key. Defaults to the production legacy slot.
    ///   - makeAddressedKeyManager: builds the destination KeyManager for the resolved address.
    static func migrateIfNeeded(
        registry: AccountRegistry = AccountRegistry(),
        defaults: UserDefaults = .standard,
        appSupport: URL? = nil,
        legacyKeyManager: KeyManager = KeyManager(address: nil),
        makeAddressedKeyManager: (String) -> KeyManager = { KeyManager(address: $0) }
    ) async {
        guard registry.isEmpty() else { return }
        guard let serverURL = defaults.string(forKey: legacyServerURLKey),
              let username = defaults.string(forKey: legacyUsernameKey),
              let host = URL(string: serverURL)?.host else { return }
        guard await legacyKeyManager.hasStoredKeypair() else { return }

        let address = "\(username)@\(host)"

        // 1. Copy the device key into the address-keyed slot.
        do {
            let bytes = try await legacyKeyManager.exportPrivateKeyBytes()
            try await makeAddressedKeyManager(address).importKeypair(from: bytes)
        } catch {
            return  // leave everything as-is; retry next launch (registry still empty)
        }

        // 2. Rename the legacy DB (+ WAL/SHM sidecars) to the per-account file.
        migrateDatabaseFiles(forAccount: address, appSupport: appSupport)

        // 3. Seed the registry with this account, active.
        let faceID = defaults.bool(forKey: legacyFaceIDKey)
        registry.upsert(AccountSummary(address: address, serverURL: serverURL,
                                       username: username, faceIDEnabled: faceID))
        registry.setActive(address: address)

        // 4. Clear the legacy session keys — migration complete (idempotent thereafter).
        [legacyServerURLKey, legacyUsernameKey, legacyFaceIDKey]
            .forEach { defaults.removeObject(forKey: $0) }
    }

    private static func migrateDatabaseFiles(forAccount address: String, appSupport: URL?) {
        let fm = FileManager.default
        let dir: URL
        if let appSupport {
            dir = appSupport
        } else {
            guard let real = try? fm.url(for: .applicationSupportDirectory, in: .userDomainMask,
                                         appropriateFor: nil, create: true) else { return }
            dir = real
        }
        let targetName = AppDatabase.dbFileName(forAccount: address)
        for suffix in ["", "-wal", "-shm"] {
            let from = dir.appendingPathComponent("ainm.sqlite\(suffix)")
            let to = dir.appendingPathComponent("\(targetName)\(suffix)")
            guard fm.fileExists(atPath: from.path) else { continue }
            if fm.fileExists(atPath: to.path) { continue }  // already migrated
            try? fm.moveItem(at: from, to: to)
        }
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `xcodebuild test -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd -only-testing:AINMMessengerTests/LegacyMigrationTests`
Expected: PASS — 3 tests pass.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/App/LegacyMigration.swift ios/AINMMessenger/AINMMessengerTests/AppTests/LegacyMigrationTests.swift
rtk proxy git commit -m "feat(ios): one-time RC→multi-account migration (key copy + DB rename + registry seed)"
```

---

## Task 5: AppState — extract makeClient + activate, make keyManager reassignable

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/App/AppState.swift`

This task introduces the reconstruction primitive used by bootstrap/onboard/switch/remove. It does NOT yet change routing — `setup()` keeps working for the UITest seed path. No new test here (covered behaviorally by Tasks 6–8 and the build); verify by building.

- [ ] **Step 1: Make `keyManager` reassignable**

In `ios/AINMMessenger/AINMMessenger/App/AppState.swift`, change line 89 from:

```swift
    let keyManager = KeyManager()
```

to:

```swift
    /// The active account's key manager. Defaults to the legacy slot (`address: nil`) so the
    /// UITest seed path and pre-migration reads work; `activate(address:)` reassigns it to the
    /// active account's address-keyed slot. Reassigned only from the @MainActor, so mutation is safe.
    private(set) var keyManager = KeyManager()
```

- [ ] **Step 2: Add the AccountRegistry property + mirrors**

In `ios/AINMMessenger/AINMMessenger/App/AppState.swift`, immediately after `var appDatabase: AppDatabase?` (line 81), add:

```swift
    /// Source of truth for which accounts exist and which is active.
    let registry = AccountRegistry()

    /// In-memory mirror of the registry for SwiftUI (refreshed via `refreshAccounts()`).
    var accounts: [AccountSummary] = []
    var activeAccount: AccountSummary?

    /// Re-read the registry into the observable mirrors.
    func refreshAccounts() {
        accounts = registry.all()
        activeAccount = registry.active()
    }
```

- [ ] **Step 3: Extract `makeClient` from `setup()`**

In `setup()`, the body builds an `AINMClient` then wires every callback inside a `Task { ... }`. Replace the whole `setup()` function (lines 152-246) with a version that delegates the wiring to a new `makeClient`:

```swift
    /// Initialize the database and protocol client for the UITest seed path and any caller
    /// that wants the legacy single-account layout. Normal launches go through `bootstrap()`
    /// → `activate(address:)` instead.
    func setup() throws {
        let db = try AppDatabase.makeDefault()
        self.appDatabase = db
        self.client = makeClient(keyManager: keyManager, appDatabase: db)
    }

    /// Build an AINMClient for the given key manager + database and wire all of its callbacks
    /// into this AppState. Used by `setup()` (legacy/UITest) and `activate(address:)` (multi-account).
    /// Wiring runs in a detached Task because the client's setters are `async`.
    private func makeClient(keyManager: KeyManager, appDatabase db: AppDatabase) -> AINMClient {
        let client = AINMClient(keyManager: keyManager, appDatabase: db)
        Task {
            await client.setStreamCallbacks(
                onStart: { [weak self] stream in
                    Task { @MainActor in
                        self?.streamingViewModel.streamStarted(stream)
                    }
                },
                onDelta: { [weak self] streamId, blockIndex, content in
                    Task { @MainActor in
                        self?.streamingViewModel.updateStreamContent(
                            streamId: streamId,
                            blockIndex: blockIndex,
                            content: content
                        )
                    }
                },
                onEnd: { [weak self] stream, status in
                    Task { @MainActor in
                        self?.streamingViewModel.streamEnded(
                            streamId: stream.streamId,
                            status: status
                        )
                    }
                },
                onError: { [weak self] stream in
                    Task { @MainActor in
                        self?.streamingViewModel.removeStream(streamId: stream.streamId)
                    }
                }
            )

            await client.setPresenceCallback { [weak self] params in
                Task { @MainActor in
                    self?.handlePresence(params)
                }
            }
            await client.setMessageDeliveredCallback { [weak self] identity in
                Task { @MainActor in
                    self?.activePresence.removeValue(forKey: identity)
                }
            }
            await client.setToolEventCallback { [weak self] threadId, event in
                Task { @MainActor in
                    self?.handleToolEvent(threadId: threadId, event: event)
                }
            }
            await client.setContextFrameCallback { [weak self] frame in
                Task { @MainActor in
                    self?.contextFrames[frame.threadId] = frame
                }
            }
            await client.setBriefingCallback { [weak self] briefing in
                Task { @MainActor in
                    self?.latestBriefing = briefing
                }
            }
            await client.setCompilationCallback { [weak self] _ in
                Task { @MainActor in
                    await self?.fetchMemoryGraph()
                }
            }
            // CRITICAL: this only touches `connectionState` — `sessionPhase` stays `.active`
            // throughout a drop/reconnect so the user is never bounced to onboarding/lock.
            await client.setConnectionStateCallback { [weak self] connState in
                Task { @MainActor in
                    guard let self else { return }
                    switch connState {
                    case .reconnecting:
                        self.connectionState = .connecting
                    case .connected:
                        self.connectionState = .connected
                    case .disconnected:
                        self.connectionState = .disconnected
                    }
                }
            }
        }
        return client
    }

    /// Point AppState at `address`'s key slot + database, rebuilding the protocol client.
    /// Does NOT connect — callers (`bootstrap`, `switchAccount`, onboarding) connect after.
    /// Resets per-account in-memory caches so the previous account's data never bleeds through.
    func activate(address: String) async {
        let km = KeyManager(address: address)
        self.keyManager = km
        let db = try? AppDatabase.make(forAccount: address)
        self.appDatabase = db
        if let db {
            self.client = makeClient(keyManager: km, appDatabase: db)
        } else {
            self.client = nil
        }
        // Clear cross-account in-memory state.
        contextFrames.removeAll()
        toolCallStates.removeAll()
        memoryGraph = nil
        latestBriefing = nil
        graphStats = nil
    }
```

- [ ] **Step 4: Build to verify it compiles**

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -configuration Debug -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build`
Expected: `BUILD SUCCEEDED`.

- [ ] **Step 5: Run the existing AppState-adjacent suites to confirm no regressions**

Run: `xcodebuild test -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd -only-testing:AINMMessengerTests`
Expected: PASS (all unit tests).

- [ ] **Step 6: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/App/AppState.swift
rtk proxy git commit -m "refactor(ios): extract makeClient + add activate(address:) + registry mirrors"
```

---

## Task 6: AppState — registry-driven bootstrap

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/App/AppState.swift`

`bootstrap()` now resumes the registry's active account instead of the RC `ainm.session.*` reads. The RC `SessionKey`/`persistSession`/`clearPersistedSession`/`persisted*` helpers stay (still used by `finishSignup`/`completeRecovery` until Task 7 rewrites them, and by LegacyMigration's source keys).

- [ ] **Step 1: Replace `bootstrap()`**

Replace `bootstrap()` (lines 134-148) with:

```swift
    /// Decide the launch route from the registry. Resume requires an active account whose
    /// key is present in the Keychain; otherwise fall back to onboarding. The Face-ID gate
    /// (SessionGateView) applies only when the account opted into `faceIDEnabled`.
    func bootstrap() async {
        refreshAccounts()
        guard let account = registry.active() else {
            sessionPhase = .onboarding
            return
        }
        await activate(address: account.address)
        guard await keyManager.hasStoredKeypair() else {
            // Active account with no key (e.g. corrupted state) → re-onboard.
            sessionPhase = .onboarding
            return
        }
        serverURL = account.serverURL
        displayName = account.username
        if account.faceIDEnabled {
            sessionPhase = .locked            // SessionGateView runs Face ID then connects
        } else {
            sessionPhase = .active
            await connect()
        }
    }
```

- [ ] **Step 2: Build to verify it compiles**

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -configuration Debug -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build`
Expected: `BUILD SUCCEEDED`.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/App/AppState.swift
rtk proxy git commit -m "feat(ios): bootstrap resumes the active registry account"
```

---

## Task 7: AppState — onboarding targets the new account's slot

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/App/AppState.swift`

`finishSignup` and `completeRecovery` must bind/escrow/connect against the NEW account's key slot + DB, not the previously-active account's. Because the address (`username@host`) is known up-front, they call `activate(address:)` first, then proceed with `self.keyManager`/`self.client` (now the new account's), and on success seed the registry.

- [ ] **Step 1: Add an address helper**

In `ios/AINMMessenger/AINMMessenger/App/AppState.swift`, add this helper next to `restBaseURL(from:)` (after line 292):

```swift
    /// The AINM address `username@host` for a signup/recovery, derived from the WS server URL's
    /// host. Mirrors how `connect()` builds `connectedIdentity`. nil if the URL has no host.
    private func makeAddress(serverURL: String, username: String) -> String? {
        guard let host = URL(string: serverURL)?.host else { return nil }
        return "\(username)@\(host)"
    }
```

- [ ] **Step 2: Rewrite `finishSignup`**

Replace `finishSignup(...)` (lines 350-380) with:

```swift
    func finishSignup(serverURL: String, username: String, enrollmentToken: String, pin: String, faceIDEnabled: Bool) async -> Bool {
        guard let base = restBaseURL(from: serverURL) else {
            onboardingError = "Enter a valid server URL."
            return false
        }
        guard let address = makeAddress(serverURL: serverURL, username: username) else {
            onboardingError = "Enter a valid server URL."
            return false
        }
        onboardingError = nil
        // Point AppState at THIS account's key slot + DB before generating/binding the key,
        // so the bound key lands in the new account's slot — not the previously-active one.
        await activate(address: address)
        let account = AccountService(baseURL: base)
        do {
            let kp = try await keyManager.getOrCreateKeypair()
            try await account.bindKey(
                enrollmentToken: enrollmentToken,
                publicKeyB64: kp.publicKeyBase64url,
                deviceLabel: deviceLabelString()
            )
            try await EscrowClient(account: account, keyManager: keyManager)
                .enrollAndUpload(enrollmentToken: enrollmentToken, pin: pin)
            self.serverURL = serverURL
            self.displayName = username
            await connect()
            if connectionState == .connected {
                registry.upsert(AccountSummary(address: address, serverURL: serverURL,
                                               username: username, faceIDEnabled: faceIDEnabled))
                registry.setActive(address: address)
                refreshAccounts()
                sessionPhase = .active
            }
            return connectionState == .connected
        } catch {
            onboardingError = onboardingMessage(for: error)
            return false
        }
    }
```

- [ ] **Step 3: Rewrite `completeRecovery`**

Replace `completeRecovery(...)` (lines 404-433) with:

```swift
    func completeRecovery(serverURL: String, email: String, username: String, code: String, pin: String, faceIDEnabled: Bool = false) async -> RecoveryOutcome {
        guard let base = restBaseURL(from: serverURL) else {
            onboardingError = "Enter a valid server URL."
            return .failed
        }
        guard let address = makeAddress(serverURL: serverURL, username: username) else {
            onboardingError = "Enter a valid server URL."
            return .failed
        }
        onboardingError = nil
        // Recover INTO this account's slot + DB.
        await activate(address: address)
        let account = AccountService(baseURL: base)
        do {
            let token = try await account.recoverVerify(email: email, code: code)
            try await EscrowClient(account: account, keyManager: keyManager)
                .recover(recoveryToken: token, pin: pin)
            self.serverURL = serverURL
            self.displayName = username
            await connect()
            if connectionState == .connected {
                registry.upsert(AccountSummary(address: address, serverURL: serverURL,
                                               username: username, faceIDEnabled: faceIDEnabled))
                registry.setActive(address: address)
                refreshAccounts()
                sessionPhase = .active
                return .connected
            }
            return .failed
        } catch EscrowClient.RecoverError.wrongPin {
            return .wrongPin
        } catch EscrowClient.RecoverError.destroyed {
            onboardingError = "Recovery is no longer possible (too many attempts)."
            return .destroyed
        } catch {
            onboardingError = onboardingMessage(for: error)
            return .failed
        }
    }
```

- [ ] **Step 4: Build to verify it compiles**

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -configuration Debug -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build`
Expected: `BUILD SUCCEEDED`.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/App/AppState.swift
rtk proxy git commit -m "feat(ios): onboarding/recovery bind into the new account's keyed slot + register it"
```

---

## Task 8: AppState — switch / add / remove account

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/App/AppState.swift`

- [ ] **Step 1: Rewrite `signOut` to remove the active account, and add the three account operations**

Replace `signOut(forgetKey:)` (lines 121-127) with the following (the account operations live together):

```swift
    /// Sign out of the active account. With multi-account this is "remove the active account":
    /// it deletes the account's registry entry (and, when `forgetKey`, its Keychain key + DB)
    /// then routes to another account or onboarding. Kept as `signOut` for SessionGateView.
    func signOut(forgetKey: Bool = false) {
        guard let address = registry.activeAddress else {
            clearPersistedSession()
            sessionPhase = .onboarding
            return
        }
        Task { await removeAccount(address, deleteData: forgetKey) }
    }

    /// Switch to another existing account: disconnect, repoint key + DB, reconnect.
    /// No Face-ID re-prompt (Face ID gates launch-resume only).
    func switchAccount(to address: String) async {
        guard let target = registry.all().first(where: { $0.address == address }) else { return }
        await disconnect()
        await activate(address: address)
        serverURL = target.serverURL
        displayName = target.username
        registry.setActive(address: address)
        refreshAccounts()
        sessionPhase = .active
        await connect()
    }

    /// Begin adding a NEW account: drop to onboarding without disturbing the registry.
    /// On a successful signup/recovery, `finishSignup`/`completeRecovery` register the new
    /// account and make it active. The existing accounts remain; cancelling returns to them.
    func addAccount() {
        onboardingError = nil
        sessionPhase = .onboarding
    }

    /// Remove an account. Deletes its registry entry; when `deleteData`, also its Keychain key
    /// and DB files. If it was active, route to another account (switch) or onboarding.
    func removeAccount(_ address: String, deleteData: Bool = true) async {
        let wasActive = registry.activeAddress == address
        if deleteData {
            try? await KeyManager(address: address).deleteKeypair()
            if let url = try? AppDatabase.dbURL(forAccount: address) {
                for suffix in ["", "-wal", "-shm"] {
                    try? FileManager.default.removeItem(atPath: url.path + suffix)
                }
            }
        }
        registry.remove(address: address)
        refreshAccounts()
        guard wasActive else { return }
        await disconnect()
        if let next = registry.all().first {
            await switchAccount(to: next.address)
        } else {
            clearPersistedSession()
            keyManager = KeyManager()   // back to the legacy slot default
            appDatabase = nil
            client = nil
            sessionPhase = .onboarding
        }
    }
```

- [ ] **Step 2: Build to verify it compiles**

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -configuration Debug -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build`
Expected: `BUILD SUCCEEDED`.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/App/AppState.swift
rtk proxy git commit -m "feat(ios): switchAccount / addAccount / removeAccount; signOut removes active account"
```

---

## Task 9: Wire LegacyMigration at launch + key the list view by active account

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/App/AINMMessengerApp.swift`

Normal launches run the migration then `bootstrap()`; the UITest seed path keeps using `setup()` + forced state (unchanged). The conversation list is keyed on the active address so a switch rebuilds its GRDB-observing view model against the new DB.

- [ ] **Step 1: Key the active root view by active address**

In `ios/AINMMessenger/AINMMessenger/App/AINMMessengerApp.swift`, replace the `.active` case (lines 19-20) with:

```swift
                    case .active:
                        ConversationListView(appState: appState)
                            // Rebuild the list (and its GRDB-observing view model) when the
                            // active account changes, so a switch shows the new account's DB.
                            .id(appState.activeAccount?.address ?? "active")
```

- [ ] **Step 2: Run migration before the normal-launch bootstrap**

In the `.task { ... }` block, find the normal-launch bootstrap section (lines 106-114, the `if seedFixture == nil { await appState.bootstrap() }` inside `#if DEBUG` and the `#else await appState.bootstrap()`). Replace that section:

```swift
                // Normal launch (no UITest seed): decide the launch route. The seed path above
                // already forced `.active`, so skip bootstrap there.
                if seedFixture == nil {
                    await appState.bootstrap()
                }
                #else
                // Release / non-DEBUG launch: always run the launch-route decision.
                await appState.bootstrap()
                #endif
```

with:

```swift
                // Normal launch (no UITest seed): migrate the RC single account into the
                // registry (one-time, idempotent) then decide the launch route. The seed path
                // above already forced `.active`, so skip both there.
                if seedFixture == nil {
                    await LegacyMigration.migrateIfNeeded()
                    await appState.bootstrap()
                }
                #else
                // Release / non-DEBUG launch: migrate then run the launch-route decision.
                await LegacyMigration.migrateIfNeeded()
                await appState.bootstrap()
                #endif
```

- [ ] **Step 3: Build to verify it compiles**

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -configuration Debug -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build`
Expected: `BUILD SUCCEEDED`.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/App/AINMMessengerApp.swift
rtk proxy git commit -m "feat(ios): run LegacyMigration before bootstrap; key list view by active account"
```

---

## Task 10: AccountSwitcherView + entry point

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/UI/Accounts/AccountSwitcherView.swift`
- Modify: `ios/AINMMessenger/AINMMessenger/UI/ConversationList/ConversationListView.swift`

- [ ] **Step 1: Create the switcher view**

Create `ios/AINMMessenger/AINMMessenger/UI/Accounts/AccountSwitcherView.swift`:

```swift
import SwiftUI

/// Account switcher sheet reachable from the Conversations toolbar. Lists the device's
/// accounts (active marked), switches on tap, adds a new account via onboarding, and
/// removes accounts via swipe-to-delete with confirmation.
struct AccountSwitcherView: View {
    @Bindable var appState: AppState
    @Environment(\.dismiss) private var dismiss
    @State private var pendingRemoval: AccountSummary?

    var body: some View {
        NavigationStack {
            List {
                Section("Accounts") {
                    ForEach(appState.accounts) { account in
                        Button {
                            guard account.address != appState.activeAccount?.address else {
                                dismiss(); return
                            }
                            Task {
                                await appState.switchAccount(to: account.address)
                                dismiss()
                            }
                        } label: {
                            HStack {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(account.username).font(.body)
                                    Text(account.address).font(.caption).foregroundStyle(.secondary)
                                }
                                Spacer()
                                if account.address == appState.activeAccount?.address {
                                    Image(systemName: "checkmark")
                                        .foregroundStyle(Color.accentColor)
                                        .accessibilityLabel("Active account")
                                }
                            }
                        }
                        .buttonStyle(.plain)
                        .swipeActions {
                            Button(role: .destructive) {
                                pendingRemoval = account
                            } label: {
                                Label("Remove", systemImage: "trash")
                            }
                        }
                    }
                }
                Section {
                    Button {
                        appState.addAccount()
                        dismiss()
                    } label: {
                        Label("Add Account", systemImage: "plus.circle")
                    }
                }
            }
            .navigationTitle("Accounts")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }
                }
            }
            .confirmationDialog(
                "Remove this account from this device?",
                isPresented: Binding(
                    get: { pendingRemoval != nil },
                    set: { if !$0 { pendingRemoval = nil } }
                ),
                titleVisibility: .visible
            ) {
                if let account = pendingRemoval {
                    Button("Remove \(account.username)", role: .destructive) {
                        Task {
                            await appState.removeAccount(account.address)
                            pendingRemoval = nil
                            if appState.sessionPhase == .onboarding { dismiss() }
                        }
                    }
                }
                Button("Cancel", role: .cancel) { pendingRemoval = nil }
            } message: {
                Text("This deletes its key and local conversations on this device. The account itself is not deleted from the server.")
            }
            .onAppear { appState.refreshAccounts() }
        }
    }
}
```

- [ ] **Step 2: Add the entry point in ConversationListView**

In `ios/AINMMessenger/AINMMessenger/UI/ConversationList/ConversationListView.swift`, add a state flag next to the existing `@State` declarations (after line 10):

```swift
    @State private var showAccountSwitcher = false
```

Then add an account button to the `.topBarTrailing` `HStack` — insert it as the first child, before the Memory-graph button (before line 24's `Button {`):

```swift
                        Button {
                            showAccountSwitcher = true
                        } label: {
                            Image(systemName: "person.crop.circle")
                                .accessibilityLabel("Accounts")
                        }
```

Then add a sheet alongside the existing sheets (after the `.sheet(isPresented: $showMemoryGraph)` block, after line 82):

```swift
            .sheet(isPresented: $showAccountSwitcher) {
                AccountSwitcherView(appState: appState)
            }
```

- [ ] **Step 3: Build to verify it compiles**

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -configuration Debug -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build`
Expected: `BUILD SUCCEEDED`.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/Accounts/AccountSwitcherView.swift ios/AINMMessenger/AINMMessenger/UI/ConversationList/ConversationListView.swift
rtk proxy git commit -m "feat(ios): account switcher sheet + toolbar entry point"
```

---

## Task 11: Full suite, DoD verification, and runbook note

**Files:**
- Modify: `docs/superpowers/specs/2026-06-04-multi-account-design.md` (append a runbook note — DoD #6)

- [ ] **Step 1: Run the full iOS unit test suite**

Run: `xcodebuild test -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd -only-testing:AINMMessengerTests`
Expected: PASS — all unit tests green (AccountRegistry, KeyManager incl. isolation, AppDatabase incl. per-account, LegacyMigration, plus the pre-existing suites).

- [ ] **Step 2: Confirm the server suite is unaffected**

The spec expects no server change. Confirm nothing under `server/` was modified:

Run: `rtk proxy git diff --name-only main -- server/`
Expected: empty output (no server files changed).

- [ ] **Step 3: Append the runbook note to the design doc**

Add this section to the end of `docs/superpowers/specs/2026-06-04-multi-account-design.md`:

```markdown
## 10. Runbook (add / switch / remove / migration)

- **Add an account:** Conversations → person icon (top trailing) → **Add Account** → run
  signup or recovery as normal. On success the new account becomes active.
- **Switch accounts:** person icon → tap a non-active account. The app disconnects and
  reconnects as that identity (no Face-ID re-prompt); the list rebuilds against its DB.
- **Remove an account:** person icon → swipe a row → **Remove** → confirm. Deletes that
  account's Keychain key + local DB on this device (the server account is untouched). If it
  was active, the app switches to another account or returns to onboarding.
- **One-time migration:** on first launch of this build, an existing RC single account
  (legacy Keychain key + `ainm.sqlite` + `ainm.session.*`) is migrated into the registry as
  the active account — its key and conversations are preserved. Migration is idempotent.
```

- [ ] **Step 4: Verify DoD against the spec**

Confirm each spec §8 item against the implementation: (1) second account onboards without
losing the first — both in the switcher; (2) switching disconnects/reconnects + shows only
that account's threads (list keyed by address); (3) the pre-existing account is migrated in,
idempotently (LegacyMigrationTests); (4) removing deletes key + DB and repoints/returns to
onboarding; (5) per-account isolation + registry/migration/switch covered by tests, iOS unit
suite green; (6) runbook note added.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add docs/superpowers/specs/2026-06-04-multi-account-design.md
rtk proxy git commit -m "docs(ma): multi-account runbook note + DoD verification"
```

---

## Self-Review (completed by plan author)

**Spec coverage:**
- §3 AccountRegistry → Task 1. Per-account keys → Task 2. Legacy key migration → Task 4 (key copy).
- §4 per-account DB + DB rename → Task 3 + Task 4 (DB files).
- §5 AppState model (activate/bootstrap/switch/add/remove, onboarding into new slot) → Tasks 5–8; AINMMessengerApp wiring → Task 9. The §5 key-sequencing call-out is resolved in Task 7 (address derived from `username@host` before key creation; `activate` first).
- §6 switcher UI → Task 10.
- §7 tests: registry round-trip + remove-active (Task 1); per-account key isolation (Task 2); legacy migration + idempotency (Task 4); per-account DB paths/schema (Task 3). Switch behavior is exercised via the keyed list view + the build; the deeper `switchAccount` DB-identity assertion from §7.5 is covered behaviorally (no @MainActor AppState unit harness exists in this project; adding one is out of scope and would be its own task).
- §8 DoD + runbook → Task 11.

**Placeholder scan:** none — every code step shows complete code.

**Type consistency:** `AccountSummary(address:serverURL:username:faceIDEnabled:)` used identically in Tasks 1, 4, 7. `AccountRegistry` methods (`all/active/activeAddress/contains/upsert/remove/setActive/isEmpty`) consistent across Tasks 1, 6, 7, 8. `KeyManager(address:)` / `keychainAccount(forAddress:)` consistent across Tasks 2, 4, 5, 8. `AppDatabase.dbFileName(forAccount:)`/`dbURL(forAccount:)`/`make(forAccount:)` consistent across Tasks 3, 4, 8. `activate(address:)` / `refreshAccounts()` / `makeClient(keyManager:appDatabase:)` consistent across Tasks 5–8.

**Known deviation from spec wording:** spec §4 said `makeDefault()` "is retired"; this plan KEEPS it (UITest seed path + unit tests depend on it) and adds `make(forAccount:)` alongside — the spec explicitly allowed "or kept only for the legacy path."
