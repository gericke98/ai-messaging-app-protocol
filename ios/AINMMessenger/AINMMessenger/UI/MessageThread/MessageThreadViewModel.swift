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

    /// Weak reference to StreamingViewModel for ephemeral-to-permanent transition.
    /// Set by MessageThreadView on appear.
    weak var streamingViewModel: StreamingViewModel?

    private let threadId: String
    private let currentUserIdentity: String
    private let repository: MessageRepository
    private let threadRepository: ThreadRepository
    private var participants: [String: ParticipantRecord] = [:]  // identity -> record
    private var allMessages: [MessageRecord] = []
    private var oldestTimestamp: String?
    private var newestTimestamp: String?
    private var observationTask: Task<Void, Never>?

    init(threadId: String, currentUserIdentity: String, database: AppDatabase) {
        self.threadId = threadId
        self.currentUserIdentity = currentUserIdentity
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
