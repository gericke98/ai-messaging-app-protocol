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
///
/// `@unchecked Sendable`: UserDefaults is documented as thread-safe; we hold it only to
/// allow test injection of a suite-isolated instance.
struct AccountRegistry: @unchecked Sendable {
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
        // AccountSummary is trivially Codable, so encoding cannot fail; force-try so a
        // future non-Codable field surfaces as a crash instead of silent data loss.
        let data = try! JSONEncoder().encode(list)
        defaults.set(data, forKey: Self.accountsKey)
    }
}
