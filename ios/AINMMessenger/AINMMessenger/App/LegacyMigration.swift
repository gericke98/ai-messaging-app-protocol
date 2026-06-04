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
