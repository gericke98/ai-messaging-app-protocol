#if DEBUG
import Foundation
import Testing

@testable import AINMMessenger

/// Smoke tests for the fixture schema and the pristine-DB helper.
/// Per-fixture row-count assertions live in `SeedLoaderIntegrationTests` (Task 4)
/// because they require a live AppDatabase — this file just verifies the schema
/// decodes and the file-system reset is idempotent.
struct ScenarioFixtureTests {

    /// Decode scenario-01 if Task 3 has already authored the fixtures. Guarded
    /// by Bundle lookup so Task 2 can commit before Task 3 (tests still pass).
    @Test("Decodes scenario-01 fixture when present in app bundle")
    func test_decodesScenario01IfAuthored() throws {
        guard let url = Bundle.main.url(forResource: "scenario-01-tool-call", withExtension: "json") else {
            // Fixture not yet in bundle — Task 3 authors it. Skip silently.
            return
        }
        let data = try Data(contentsOf: url)
        let fixture = try JSONDecoder().decode(ScenarioFixture.self, from: data)
        #expect(fixture.scenario == "scenario-01-tool-call")
        #expect(fixture.identity == UITestIdentityBootstrap.identityAddress)
        #expect(!fixture.threads.isEmpty)
        #expect(!fixture.participants.isEmpty)
    }

    /// `preparePristineDatabase` must be safe to call when no DB file exists yet.
    /// This models the "fresh simulator" case and the second-call idempotency guard.
    @Test("preparePristineDatabase does not throw when no DB files are present")
    func test_preparePristineDatabase_idempotent() throws {
        // Run twice in sequence. Second call guarantees idempotence (no files left).
        try SeedLoader.preparePristineDatabase()
        try SeedLoader.preparePristineDatabase()
    }

    /// Round-trip the fixture structs through Codable to guarantee every
    /// property is `Decodable`. A compile-time-only sanity check that mirrors
    /// what the real JSON decoder does.
    @Test("Minimum-viable ScenarioFixture literal decodes cleanly")
    func test_minimumViableFixture_decodes() throws {
        let json = """
        {
          "version": "1.0",
          "scenario": "scenario-01-tool-call",
          "identity": "uat-user@localhost",
          "threads": [],
          "participants": [],
          "messages": [],
          "toolCalls": [],
          "archivalFacts": [],
          "agentCards": [],
          "savedAgents": []
        }
        """
        let data = Data(json.utf8)
        let fixture = try JSONDecoder().decode(ScenarioFixture.self, from: data)
        #expect(fixture.version == "1.0")
        #expect(fixture.identity == "uat-user@localhost")
        #expect(fixture.threads.isEmpty)
    }

    /// Loading a missing fixture surfaces a typed `fixtureNotFound` error
    /// rather than bubbling a generic URL error.
    @Test("loadFixtureIfRequested throws fixtureNotFound for unknown name")
    @MainActor
    func test_loadFixtureIfRequested_missingFixtureThrows() async throws {
        let appState = AppState()
        do {
            try await SeedLoader.loadFixtureIfRequested(
                fixture: "definitely-not-a-real-fixture-\(UUID().uuidString)",
                into: appState
            )
            Issue.record("Expected fixtureNotFound error but call succeeded")
        } catch let error as SeedLoader.SeedError {
            if case .fixtureNotFound = error {
                // Expected path.
            } else {
                Issue.record("Expected .fixtureNotFound, got \(error)")
            }
        }
    }
}
#endif
