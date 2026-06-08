#if DEBUG
import Foundation
import GRDB

/// Hermetic database seed loader for UAT runs.
///
/// Two entry points, invoked in order from `AINMMessengerApp.swift`:
///  1. `preparePristineDatabase()` — deletes `ainm.sqlite` (+ WAL/SHM) BEFORE
///     `AppDatabase.makeDefault()` opens a pool. Running first avoids Pitfall 6
///     (GRDB ValueObservation firing with stale+fresh state during a live seed).
///  2. `loadFixtureIfRequested(fixture:into:)` — decodes a bundled
///     `scenario-NN-*.json` fixture and inserts every row in one `dbPool.write`
///     transaction so observations see a single atomic change.
///
/// Fixture files live in the APP target bundle at `AINMMessenger/Testing/Fixtures/`
/// and are resolved via `Bundle.main.url(forResource:withExtension:)` — the
/// synchronized root group copies them into the built `.app`.
enum SeedLoader {

    enum SeedError: Swift.Error, CustomStringConvertible {
        case fixtureNotFound(String)
        case decodeFailed(String, underlying: any Swift.Error)
        case databaseNotReady

        var description: String {
            switch self {
            case .fixtureNotFound(let name):
                return "Fixture not found in app bundle: \(name).json"
            case .decodeFailed(let name, let underlying):
                return "Failed to decode fixture \(name): \(underlying)"
            case .databaseNotReady:
                return "AppDatabase must be initialized before loadFixtureIfRequested"
            }
        }
    }

    /// Delete the SQLite main + WAL + SHM sidecar files in Application Support.
    /// MUST run BEFORE `AppDatabase.makeDefault()` to avoid ValueObservation
    /// flicker (Pitfall 6 in 14-RESEARCH.md). Idempotent — no-op when files are
    /// already absent (fresh simulator container, or second call in the same launch).
    static func preparePristineDatabase() throws {
        let fm = FileManager.default
        let appSupport = try fm.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        let dbURL = appSupport.appendingPathComponent("ainm.sqlite")
        // WAL + SHM sidecars live next to the main DB file.
        for suffix in ["", "-wal", "-shm"] {
            let url = suffix.isEmpty
                ? dbURL
                : URL(fileURLWithPath: dbURL.path + suffix)
            try? fm.removeItem(at: url)
        }
    }

    /// Decode the named fixture from `Bundle.main` and insert every row in a
    /// single transaction. `appState.appDatabase` must be set (post-`setup()`).
    ///
    /// Writing all rows in one `dbPool.write { ... }` closure satisfies GRDB's
    /// "one transaction per write call" guarantee, so ValueObservation fires once
    /// with the final state — UI never renders an intermediate empty snapshot.
    @MainActor
    static func loadFixtureIfRequested(
        fixture name: String,
        into appState: AppState
    ) async throws {
        guard let dbPool = appState.appDatabase?.dbPool else {
            throw SeedError.databaseNotReady
        }

        let url = bundleURL(forFixture: name)
        guard let resolvedURL = url else {
            throw SeedError.fixtureNotFound(name)
        }
        let data = try Data(contentsOf: resolvedURL)
        let fixture: ScenarioFixture
        do {
            fixture = try JSONDecoder().decode(ScenarioFixture.self, from: data)
        } catch {
            throw SeedError.decodeFailed(name, underlying: error)
        }

        try await dbPool.write { db in
            try insertFixture(fixture, into: db)
        }
    }

    /// Synchronous by-name loader for unit tests: decodes the named fixture and
    /// inserts every row into `db` in a single synchronous write transaction.
    ///
    /// This mirrors `loadFixtureIfRequested` exactly so the same JSON is exercised
    /// by both the app boot path and the unit-test suite — without requiring an
    /// `AppState` wrapper or async context.
    ///
    /// Bundle resolution: prefers `Bundle.main` (app / UAT host), then falls back
    /// to the bundle containing `SeedLoaderBundleAnchor` (unit-test host where
    /// `Bundle.main` is the XCTest runner and the JSON lives in the app bundle).
    static func loadFixture(named name: String, into db: AppDatabase) throws {
        guard let resolvedURL = bundleURL(forFixture: name) else {
            throw SeedError.fixtureNotFound(name)
        }
        let data = try Data(contentsOf: resolvedURL)
        let fixture: ScenarioFixture
        do {
            fixture = try JSONDecoder().decode(ScenarioFixture.self, from: data)
        } catch {
            throw SeedError.decodeFailed(name, underlying: error)
        }
        try db.dbPool.write { grdb in
            try insertFixture(fixture, into: grdb)
        }
    }

    // MARK: - Private helpers

    private static func bundleURL(forFixture name: String) -> URL? {
        Bundle.main.url(forResource: name, withExtension: "json")
            ?? Bundle(for: SeedLoaderBundleAnchor.self)
                .url(forResource: name, withExtension: "json")
    }

    private static func insertFixture(_ fixture: ScenarioFixture, into db: Database) throws {
        for thread in fixture.threads {
            try thread.asRecord().insert(db)
        }
        for participant in fixture.participants {
            try participant.asRecord().insert(db)
        }
        for message in fixture.messages {
            try message.asRecord().insert(db)
        }
        for toolCall in fixture.toolCalls {
            try toolCall.asRecord().insert(db)
        }
        for fact in fixture.archivalFacts {
            try fact.asRecord().insert(db)
        }
        for card in fixture.agentCards {
            try card.asRecord().insert(db)
        }
        for agent in fixture.savedAgents {
            try agent.asRecord().insert(db)
        }
    }
}

/// Private anchor class used only for `Bundle(for:)` resolution in unit tests.
/// Must be a class (not a struct or enum) — `Bundle(for:)` accepts only `AnyClass`.
private final class SeedLoaderBundleAnchor {}
#endif
