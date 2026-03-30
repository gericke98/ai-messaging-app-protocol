import GRDB
import Foundation

/// Central database access point for the AINM Messenger app.
/// Uses GRDB DatabasePool for concurrent reads with serialized writes.
/// All schema changes flow through the DatabaseMigrator for versioned, idempotent migrations.
struct AppDatabase: Sendable {
    let dbPool: DatabasePool

    /// Create database at the standard Application Support path.
    /// Creates the directory if it doesn't exist.
    static func makeDefault() throws -> AppDatabase {
        let fileManager = FileManager.default
        let appSupportURL = try fileManager.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        let dbURL = appSupportURL.appendingPathComponent("ainm.sqlite")
        let dbPool = try DatabasePool(path: dbURL.path)
        let db = AppDatabase(dbPool: dbPool)
        try db.migrate()
        return db
    }

    /// Create temporary database for testing.
    /// Uses a unique file in the temp directory (DatabasePool requires WAL mode,
    /// which is not supported for in-memory databases).
    static func makeInMemory() throws -> AppDatabase {
        let tempDir = NSTemporaryDirectory()
        let dbPath = (tempDir as NSString).appendingPathComponent("ainm-test-\(UUID().uuidString).sqlite")
        let dbPool = try DatabasePool(path: dbPath)
        let db = AppDatabase(dbPool: dbPool)
        try db.migrate()
        return db
    }

    private func migrate() throws {
        var migrator = DatabaseMigrator()

        // Always run all migrations in development (wipe on schema change).
        // Remove this line for production releases.
        #if DEBUG
        migrator.eraseDatabaseOnSchemaChange = true
        #endif

        migrator.registerMigration("v1_create_threads") { db in
            try db.create(table: "thread") { t in
                t.primaryKey("id", .text).notNull()              // UUIDv7 string
                t.column("title", .text)                         // Optional thread title
                t.column("createdAt", .text).notNull()           // RFC 3339 timestamp
                t.column("createdBy", .text).notNull()           // AINM address
                t.column("parentThreadId", .text)                // Sub-thread parent
                    .references("thread", onDelete: .cascade)
                t.column("parentMessageId", .text)               // Message that spawned sub-thread
            }
        }

        migrator.registerMigration("v1_create_messages") { db in
            try db.create(table: "message") { t in
                t.primaryKey("id", .text).notNull()              // UUIDv7 string
                t.column("threadId", .text).notNull()            // FK to thread
                    .references("thread", onDelete: .cascade)
                t.column("fromAddress", .text).notNull()         // AINM address of sender
                t.column("timestamp", .text).notNull()           // RFC 3339 timestamp
                t.column("type", .text).notNull()                // "message", "thread.create", etc.
                t.column("contentJson", .text).notNull()         // JSON array of content blocks
                t.column("signature", .text).notNull()           // base64url Ed25519 signature
                t.column("serverTimestamp", .text)               // Server-assigned timestamp
            }
            // Index for querying messages by thread, ordered by timestamp
            try db.create(index: "idx_message_thread_timestamp",
                          on: "message",
                          columns: ["threadId", "timestamp"])
        }

        migrator.registerMigration("v1_create_participants") { db in
            try db.create(table: "participant") { t in
                t.column("threadId", .text).notNull()            // FK to thread
                    .references("thread", onDelete: .cascade)
                t.column("identity", .text).notNull()            // AINM address
                t.column("type", .text).notNull()                // "human" or "agent"
                t.column("joinedAt", .text).notNull()            // RFC 3339 timestamp
                t.column("role", .text).notNull()                // "creator" or "member"
                t.primaryKey(["threadId", "identity"])
            }
        }

        try migrator.migrate(dbPool)
    }
}
