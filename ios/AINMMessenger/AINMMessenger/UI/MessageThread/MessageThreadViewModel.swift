import Foundation
import Observation
import GRDB

/// Group of consecutive messages from the same sender per D-03.
struct MessageGroup: Identifiable {
    let id: String       // first message ID
    let senderAddress: String
    let senderType: String  // "human" or "agent"
    let senderName: String  // display name portion of AINM address
    let messages: [MessageRecord]
    let isFromCurrentUser: Bool
}

@MainActor
@Observable
final class MessageThreadViewModel {
    var messageGroups: [MessageGroup] = []
    var isLoadingMore = false
    var hasMoreMessages = true
    var threadTitle: String = ""
    var participantCount: Int = 0
    var hasAgentParticipant: Bool = false

    /// Weak reference to StreamingViewModel for ephemeral-to-permanent transition.
    /// Set by MessageThreadView on appear.
    weak var streamingViewModel: StreamingViewModel?

    private let threadId: String
    private let currentUserIdentity: String
    private let repository: MessageRepository
    private let threadRepository: ThreadRepository
    private let database: AppDatabase
    private var participants: [String: ParticipantRecord] = [:]  // identity -> record

    /// Participants exposed as MentionCandidate array for ComposerView's @-autocomplete
    /// (Plan 14-06 deviation Rule 2 — previously not wired, autocomplete rendered empty).
    /// Excludes the current user so self-mentions don't appear in the picker.
    /// ParticipantRecord has no displayName column (Phase 13 simplification), so we
    /// derive display name from the identity local-part — capitalizing to match common
    /// UX (e.g. `remy@localhost` → `Remy`). This mirrors the fixture displayName where
    /// identity `remy@localhost` had displayName `Remy`.
    var mentionCandidates: [MentionCandidate] {
        participants.values
            .filter { $0.identity != currentUserIdentity }
            .map { p in
                let localPart = p.identity.components(separatedBy: "@").first ?? p.identity
                let displayName = localPart.prefix(1).uppercased() + localPart.dropFirst()
                return MentionCandidate(
                    id: p.identity,
                    displayName: displayName,
                    type: p.type
                )
            }
            .sorted { $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending }
    }
    private var allMessages: [MessageRecord] = []
    private var oldestTimestamp: String?
    private var newestTimestamp: String?
    private var observationTask: Task<Void, Never>?
    /// Cache of sub-thread info keyed by parent message ID.
    private var subThreadCache: [String: (threadId: String, replyCount: Int)] = [:]

    init(threadId: String, currentUserIdentity: String, database: AppDatabase) {
        self.threadId = threadId
        self.currentUserIdentity = currentUserIdentity
        self.database = database
        self.repository = MessageRepository(dbPool: database.dbPool)
        self.threadRepository = ThreadRepository(dbPool: database.dbPool)
    }

    func loadInitial() async {
        // Mark thread as read
        try? await threadRepository.markAsRead(threadId: threadId)

        // Load participants
        let parts = (try? await repository.fetchParticipants(threadId: threadId)) ?? []
        for p in parts { participants[p.identity] = p }
        participantCount = parts.count
        hasAgentParticipant = parts.contains { $0.type == "agent" }

        // Load thread title
        if let thread = try? await threadRepository.dbPool.read({ db in
            try ThreadRecord.fetchOne(db, key: threadId)
        }) {
            threadTitle = thread.title ?? "Conversation"
        }

        // Load initial page
        let messages = (try? await repository.fetchMessages(threadId: threadId)) ?? []
        self.allMessages = Array(messages)
        self.oldestTimestamp = allMessages.first?.timestamp
        self.newestTimestamp = allMessages.last?.timestamp
        self.hasMoreMessages = messages.count == 50
        rebuildGroups()

        // Load sub-thread cache for indicator badges
        await loadSubThreadCache()

        // Start observing new messages
        startObservingNewMessages()
    }

    func loadOlderMessages() async {
        guard !isLoadingMore, hasMoreMessages, let oldest = oldestTimestamp else { return }
        isLoadingMore = true
        let older = (try? await repository.fetchMessages(threadId: threadId, olderThan: oldest)) ?? []
        if !older.isEmpty {
            allMessages.insert(contentsOf: older, at: 0)
            oldestTimestamp = allMessages.first?.timestamp
        }
        hasMoreMessages = older.count == 50
        isLoadingMore = false
        rebuildGroups()
    }

    func stopObserving() {
        observationTask?.cancel()
        observationTask = nil
    }

    private func startObservingNewMessages() {
        guard let newest = newestTimestamp else { return }
        observationTask = Task {
            let observation = repository.observeNewMessages(threadId: threadId, newerThan: newest)
            do {
                for try await newMessages in observation.values(in: repository.dbPool) {
                    // Append only messages not already in allMessages
                    let existingIds = Set(allMessages.map(\.id))
                    let trulyNew = newMessages.filter { !existingIds.contains($0.id) }
                    if !trulyNew.isEmpty {
                        allMessages.append(contentsOf: trulyNew)
                        newestTimestamp = allMessages.last?.timestamp
                        rebuildGroups()

                        // Ephemeral-to-permanent transition: remove streaming placeholder
                        // when the signed GRDB message arrives with matching messageId.
                        for msg in trulyNew {
                            streamingViewModel?.removePlaceholderForMessage(messageId: msg.id)
                        }
                    }
                }
            } catch {
                // Observation ended
            }
        }
    }

    private func rebuildGroups() {
        var groups: [MessageGroup] = []
        var current: [MessageRecord] = []
        var currentSender: String?

        for message in allMessages {
            if message.fromAddress == currentSender {
                current.append(message)
            } else {
                if !current.isEmpty, let sender = currentSender {
                    groups.append(makeGroup(messages: current, sender: sender))
                }
                current = [message]
                currentSender = message.fromAddress
            }
        }
        if !current.isEmpty, let sender = currentSender {
            groups.append(makeGroup(messages: current, sender: sender))
        }
        messageGroups = groups
    }

    private func makeGroup(messages: [MessageRecord], sender: String) -> MessageGroup {
        let participant = participants[sender]
        let senderType = participant?.type ?? "human"
        // Extract display name from AINM address (part before @)
        let senderName = sender.components(separatedBy: "@").first ?? sender
        return MessageGroup(
            id: messages[0].id,
            senderAddress: sender,
            senderType: senderType,
            senderName: senderName,
            messages: messages,
            isFromCurrentUser: sender == currentUserIdentity
        )
    }

    /// Decode content blocks from a MessageRecord's contentJson field.
    func contentBlocks(for message: MessageRecord) -> [ContentBlock] {
        guard let data = message.contentJson.data(using: .utf8),
              let blocks = try? JSONDecoder().decode([ContentBlock].self, from: data) else {
            return [.unknown(fallback: "[Could not decode message content]")]
        }
        return blocks
    }

    /// Return sub-thread info for a message if it has a branching sub-thread.
    func subThreadInfo(forMessageId messageId: String) -> (threadId: String, replyCount: Int)? {
        subThreadCache[messageId]
    }

    /// Public refresh to reload the sub-thread cache — called on view re-appear after
    /// a branch + back navigation so the reply-count badge on the parent message
    /// reflects newly-created sub-threads without re-creating the viewModel.
    /// (Plan 14-06 deviation Rule 1 — reply badge stale after branch+back.)
    func refreshSubThreadCache() async {
        await loadSubThreadCache()
    }

    /// Load sub-thread data for all messages in the current thread.
    private func loadSubThreadCache() async {
        do {
            let cache: [String: (String, Int)] = try await database.dbPool.read { db in
                // Find all sub-threads whose parentThreadId matches our thread
                let subThreads = try ThreadRecord
                    .filter(Column("parentThreadId") == self.threadId)
                    .fetchAll(db)
                var result: [String: (String, Int)] = [:]
                for subThread in subThreads {
                    guard let parentMessageId = subThread.parentMessageId else { continue }
                    // Count messages in this sub-thread
                    let count = try MessageRecord
                        .filter(Column("threadId") == subThread.id)
                        .filter(Column("type") == "message")
                        .fetchCount(db)
                    result[parentMessageId] = (subThread.id, count)
                }
                return result
            }
            subThreadCache = cache
        } catch {
            // Non-critical -- indicators just won't show
        }
    }

    /// Format timestamp for display.
    func formatTimestamp(_ timestamp: String) -> String {
        guard let date = ISO8601DateFormatter.ainm.date(from: timestamp) else {
            // Try without fractional seconds
            let basic = ISO8601DateFormatter()
            basic.formatOptions = [.withInternetDateTime]
            guard let d = basic.date(from: timestamp) else { return "" }
            let formatter = RelativeDateTimeFormatter()
            formatter.unitsStyle = .abbreviated
            return formatter.localizedString(for: d, relativeTo: Date())
        }
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}
