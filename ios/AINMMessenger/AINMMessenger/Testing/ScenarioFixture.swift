#if DEBUG
import Foundation
import GRDB

// MARK: - Top-level fixture

/// Top-level Codable shape loaded from `AINMMessenger/Testing/Fixtures/scenario-*.json`.
/// One JSON field per SQLite column for each seeded record type — zero impedance
/// mismatch with the GRDB records in `AINMMessenger/Persistence/`.
///
/// Versioned so 14-04/14-06 (and later maintenance) can detect incompatible fixture
/// revisions via the top-level `version` field.
struct ScenarioFixture: Decodable, Sendable {
    let version: String
    let scenario: String
    let identity: String                    // Must equal UITestIdentityBootstrap.identityAddress
    let threads: [FixtureThread]
    let participants: [FixtureParticipant]
    let messages: [FixtureMessage]
    let toolCalls: [FixtureToolCall]
    let archivalFacts: [FixtureArchivalFact]
    let agentCards: [FixtureAgentCard]
    let savedAgents: [FixtureSavedAgent]
}

// MARK: - Thread

struct FixtureThread: Decodable, Sendable {
    // Mirrors ThreadRecord (AINMMessenger/Persistence/ThreadRecord.swift).
    let id: String
    let title: String?
    let createdAt: String                   // RFC 3339 string; ThreadRecord stores it verbatim
    let createdBy: String
    let parentThreadId: String?
    let parentMessageId: String?
    let hasUnread: Bool

    func asRecord() -> ThreadRecord {
        ThreadRecord(
            id: id,
            title: title,
            createdAt: createdAt,
            createdBy: createdBy,
            parentThreadId: parentThreadId,
            parentMessageId: parentMessageId,
            hasUnread: hasUnread
        )
    }
}

// MARK: - Participant

struct FixtureParticipant: Decodable, Sendable {
    // Mirrors ParticipantRecord (AINMMessenger/Persistence/ParticipantRecord.swift).
    // `displayName` is fixture-local metadata for @-mention autocomplete seeding
    // (not persisted — the UI resolves display names through participant type + identity).
    let threadId: String
    let identity: String
    let type: String                        // "human" | "agent"
    let joinedAt: String
    let role: String                        // "creator" | "member"
    let displayName: String?

    func asRecord() -> ParticipantRecord {
        ParticipantRecord(
            threadId: threadId,
            identity: identity,
            type: type,
            joinedAt: joinedAt,
            role: role
        )
    }
}

// MARK: - Message

struct FixtureMessage: Decodable, Sendable {
    // Mirrors MessageRecord (AINMMessenger/Persistence/MessageRecord.swift).
    let id: String
    let threadId: String
    let fromAddress: String
    let timestamp: String
    let type: String
    let contentJson: String                 // Serialized `[ContentBlock]`
    let signature: String
    let serverTimestamp: String?

    func asRecord() -> MessageRecord {
        MessageRecord(
            id: id,
            threadId: threadId,
            fromAddress: fromAddress,
            timestamp: timestamp,
            type: type,
            contentJson: contentJson,
            signature: signature,
            serverTimestamp: serverTimestamp
        )
    }
}

// MARK: - Tool call

struct FixtureToolCall: Decodable, Sendable {
    // Mirrors ToolCallRecord (AINMMessenger/Persistence/ToolCallRecord.swift).
    let callId: String
    let threadId: String
    let name: String
    let status: String                      // "running" | "completed" | "error" | "timedOut"
    let input: String?                      // JSON string of tool input parameters
    let resultSummary: String?
    let durationMs: Int?
    let errorMessage: String?
    let startedAt: String
    let completedAt: String?

    func asRecord() -> ToolCallRecord {
        ToolCallRecord(
            callId: callId,
            threadId: threadId,
            name: name,
            status: status,
            input: input,
            resultSummary: resultSummary,
            durationMs: durationMs,
            errorMessage: errorMessage,
            startedAt: startedAt,
            completedAt: completedAt
        )
    }
}

// MARK: - Archival memory fact

struct FixtureArchivalFact: Decodable, Sendable {
    // Mirrors MemoryFactRecord (AINMMessenger/Persistence/MemoryFactRecord.swift).
    // The `agentId` alias is exposed for fixture authors — it maps directly onto
    // MemoryFactRecord's `storedBy` column (no separate `agentId` column exists).
    let factId: String
    let key: String
    let value: String                       // JSON string (see MemoryFactRecord.init(from:))
    let tags: String?                       // JSON-encoded array string, or nil
    let storedBy: String                    // AINM address of the agent that stored this fact
    let storedAt: String

    func asRecord() -> MemoryFactRecord {
        MemoryFactRecord(
            factId: factId,
            key: key,
            value: value,
            tags: tags,
            storedBy: storedBy,
            storedAt: storedAt
        )
    }
}

// MARK: - Saved agent

struct FixtureSavedAgent: Decodable, Sendable {
    // Mirrors SavedAgentRecord (Persistence/SavedAgentRecord.swift).
    let address: String
    let displayName: String
    let cardJson: String
    let addedAt: String

    func asRecord() -> SavedAgentRecord {
        SavedAgentRecord(
            address: address,
            displayName: displayName,
            cardJson: cardJson,
            addedAt: addedAt
        )
    }
}

// MARK: - Agent card

struct FixtureAgentCard: Decodable, Sendable {
    // Mirrors AgentCardRecord (AINMMessenger/Persistence/AgentCardRecord.swift).
    // AgentCardRecord's primary key is `url`; `identity` is the AINM address.
    // Fixture authors supply the full serialized AgentCard JSON via `cardJson`.
    let url: String
    let identity: String
    let cardJson: String
    let etag: String?
    let fetchedAt: String

    func asRecord() -> AgentCardRecord {
        AgentCardRecord(
            url: url,
            identity: identity,
            cardJson: cardJson,
            etag: etag,
            fetchedAt: fetchedAt
        )
    }
}
#endif
