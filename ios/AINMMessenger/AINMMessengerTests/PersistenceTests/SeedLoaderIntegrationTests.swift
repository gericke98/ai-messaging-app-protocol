#if DEBUG
import CryptoKit
import Foundation
import GRDB
import Testing

@testable import AINMMessenger

/// End-to-end integration tests for the Phase 14 seed bootstrap path.
///
/// Each test builds a throwaway `AppDatabase` via `AppDatabase.makeInMemory()` —
/// the DB lives in a uniquely-named file under `NSTemporaryDirectory()` and is
/// discarded at test completion. Keychain-touching tests inject
/// `KeyManager.InMemoryStorage` instead of talking to the real system Keychain
/// because iOS Simulator unit-test hosts cannot obtain the required entitlement
/// (`SecItemDelete` returns `errSecMissingEntitlement` / -34018). End-to-end
/// Keychain coverage lives in Plan 14-06's XCUITest suite, which runs against
/// the real app host.
@MainActor
struct SeedLoaderIntegrationTests {

    /// Expected row counts per fixture. Numbers reflect the exact content of the
    /// scenario-NN-*.json files shipped in Plan 14-03 Task 3. If a fixture grows
    /// new rows later, update the corresponding tuple.
    /// `nonisolated` because the Swift Testing macro expands `arguments:` from
    /// the test-runner context, which is not main-actor-isolated.
    nonisolated static let fixtureExpectations: [(fixture: String,
                                                  threads: Int,
                                                  participants: Int,
                                                  messages: Int,
                                                  toolCalls: Int,
                                                  archival: Int,
                                                  agentCards: Int)] = [
        ("scenario-01-tool-call",       1, 2, 1, 0, 0, 0),
        ("scenario-02-mentions",        1, 3, 1, 0, 0, 0),
        ("scenario-03-agent-discovery", 0, 0, 0, 0, 0, 2),
        ("scenario-04-memory",          1, 2, 0, 0, 4, 0),
        ("scenario-05-sub-thread",      1, 2, 1, 0, 0, 0),
        ("scenario-06-deep-link",       1, 2, 1, 0, 0, 0),
        ("scenario-07-a2a-burst",       1, 3, 5, 0, 0, 0),
    ]

    @Test(
        "SeedLoader inserts exactly the expected row counts for every fixture",
        arguments: fixtureExpectations
    )
    func test_seedLoader_loadsEachFixtureWithExpectedRowCounts(
        _ tc: (fixture: String,
               threads: Int,
               participants: Int,
               messages: Int,
               toolCalls: Int,
               archival: Int,
               agentCards: Int)
    ) async throws {
        // Fresh in-memory-style database per case (unique temp file; see AppDatabase.makeInMemory).
        let db = try AppDatabase.makeInMemory()
        let appState = AppState()
        appState.appDatabase = db

        try await SeedLoader.loadFixtureIfRequested(fixture: tc.fixture, into: appState)

        // Table names reflect AppDatabase migrations (see AppDatabase.swift):
        //   v1: thread, message, participant    v3: toolCall
        //   v5: memoryFact                      v4: agentCard
        let counts = try await db.dbPool.read { database in
            (
                threads:      try Int.fetchOne(database, sql: "SELECT COUNT(*) FROM thread")      ?? 0,
                participants: try Int.fetchOne(database, sql: "SELECT COUNT(*) FROM participant") ?? 0,
                messages:     try Int.fetchOne(database, sql: "SELECT COUNT(*) FROM message")     ?? 0,
                toolCalls:    try Int.fetchOne(database, sql: "SELECT COUNT(*) FROM toolCall")    ?? 0,
                archival:     try Int.fetchOne(database, sql: "SELECT COUNT(*) FROM memoryFact")  ?? 0,
                agentCards:   try Int.fetchOne(database, sql: "SELECT COUNT(*) FROM agentCard")   ?? 0
            )
        }

        #expect(counts.threads      == tc.threads,      "fixture=\(tc.fixture) threads")
        #expect(counts.participants == tc.participants, "fixture=\(tc.fixture) participants")
        #expect(counts.messages     == tc.messages,     "fixture=\(tc.fixture) messages")
        #expect(counts.toolCalls    == tc.toolCalls,    "fixture=\(tc.fixture) toolCalls")
        #expect(counts.archival     == tc.archival,     "fixture=\(tc.fixture) archival")
        #expect(counts.agentCards   == tc.agentCards,   "fixture=\(tc.fixture) agentCards")
    }

    /// `UITestIdentityBootstrap.resetIdentity(storage:)` must produce a
    /// KeyManager-loadable deterministic keypair. Delete → reset → compare: the
    /// public key derived from the seed must equal the public key a `KeyManager`
    /// subsequently loads from the same storage.
    ///
    /// We inject `KeyManager.InMemoryStorage` here because iOS Simulator unit-test
    /// hosts cannot access the system Keychain (SecItemDelete returns
    /// `errSecMissingEntitlement` / -34018). Production callers use the Keychain-
    /// backed no-arg `resetIdentity()` path — covered by the live app boot in 14-06.
    @Test("UITestIdentityBootstrap installs a keypair that KeyManager then loads deterministically")
    func test_identityBootstrap_regeneratesDeterministicKey() async throws {
        let storage = KeyManager.InMemoryStorage()

        // First reset.
        try await UITestIdentityBootstrap.resetIdentity(storage: storage)
        let firstPub = UITestIdentityBootstrap.deterministicPublicKeyBase64

        // Second reset — must yield identical public key (idempotent).
        try await UITestIdentityBootstrap.resetIdentity(storage: storage)
        let secondPub = UITestIdentityBootstrap.deterministicPublicKeyBase64
        #expect(firstPub == secondPub, "Public key must be stable across repeated resetIdentity()")
        #expect(!firstPub.isEmpty)

        // KeyManager must load the same keypair on the next getOrCreateKeypair().
        let manager = KeyManager(storage: storage)
        let keypair = try await manager.getOrCreateKeypair()
        #expect(
            keypair.publicKey.rawRepresentation.base64EncodedString() == firstPub,
            "KeyManager must load the deterministic keypair that resetIdentity() installed"
        )

        // Seed-byte drift guard — duplicates UITestIdentityBootstrapTests's check
        // because this integration suite runs in a different execution context
        // (post-Keychain-touch) and we want drift to fail here too.
        #expect(UITestIdentityBootstrap.uatSeedBytes.count == 32)
        let expectedPrefix: [UInt8] = Array("UAT_USER@LOCALHOST".utf8)
        let actualPrefix = Array(UITestIdentityBootstrap.uatSeedBytes.prefix(expectedPrefix.count))
        #expect(actualPrefix == expectedPrefix,
                "Seed ASCII prefix drifted — signed fixtures will no longer verify")

        // Signed payloads must verify against the deterministic public key. This
        // exercises the full signing path that live AINMClient uses and catches any
        // keypair-install regression that merely changed the public-key bytes.
        let payload = Data("uat-canonical-payload".utf8)
        let signature = try keypair.privateKey.signature(for: payload)
        #expect(keypair.publicKey.isValidSignature(signature, for: payload),
                "Keypair loaded back from storage must sign+verify successfully")
    }
}
#endif
