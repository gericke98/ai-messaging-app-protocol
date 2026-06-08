import GRDB
import Foundation

/// Idempotent persistence for thread participants. The merge is order-independent so the
/// three write points (thread-create seed, delivery upsert, lazy backfill) can run in any
/// order any number of times and converge: earliest joinedAt wins, role is creator-sticky,
/// and type upgrades human→agent only (a confirmed agent never regresses to human).
struct ParticipantRepository: Sendable {
    let dbPool: DatabasePool

    /// Upsert within an existing write transaction (for callers already inside `dbPool.write`).
    static func upsert(
        _ db: Database,
        threadId: String,
        identity: String,
        type: String,
        role: String,
        joinedAt: String
    ) throws {
        if let existing = try ParticipantRecord.fetchOne(
            db, key: ["threadId": threadId, "identity": identity]
        ) {
            let mergedType = (existing.type == "agent" || type == "agent") ? "agent" : "human"
            let mergedRole = (existing.role == "creator" || role == "creator") ? "creator" : "member"
            let mergedJoinedAt = min(existing.joinedAt, joinedAt)   // RFC3339 → lexicographic = chronological
            // No-op if nothing changed (avoids a redundant write).
            if mergedType == existing.type, mergedRole == existing.role, mergedJoinedAt == existing.joinedAt {
                return
            }
            try ParticipantRecord(
                threadId: threadId, identity: identity,
                type: mergedType, joinedAt: mergedJoinedAt, role: mergedRole
            ).update(db)
        } else {
            try ParticipantRecord(
                threadId: threadId, identity: identity,
                type: type, joinedAt: joinedAt, role: role
            ).insert(db)
        }
    }

    /// Async convenience: open a write and upsert.
    func upsert(
        threadId: String, identity: String, type: String, role: String, joinedAt: String
    ) async throws {
        try await dbPool.write { db in
            try ParticipantRepository.upsert(
                db, threadId: threadId, identity: identity, type: type, role: role, joinedAt: joinedAt)
        }
    }
}
