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
