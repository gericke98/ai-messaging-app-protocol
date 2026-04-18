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
        guard let url = Bundle.main.url(forResource: name, withExtension: "json") else {
            throw SeedError.fixtureNotFound(name)
        }
        let data = try Data(contentsOf: url)
        let fixture: ScenarioFixture
        do {
            fixture = try JSONDecoder().decode(ScenarioFixture.self, from: data)
        } catch {
            throw SeedError.decodeFailed(name, underlying: error)
        }

        guard let dbPool = appState.appDatabase?.dbPool else {
            throw SeedError.databaseNotReady
        }

        try await dbPool.write { db in
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
        }
    }
}
#endif
