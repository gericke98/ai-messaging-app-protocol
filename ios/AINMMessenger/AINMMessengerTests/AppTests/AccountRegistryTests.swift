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
