import Foundation
import Observation
import GRDB

/// ViewModel for the Conversation List screen (Screen 1 per UI-SPEC).
/// Drives the list via GRDB ValueObservation, providing real-time updates
/// when threads or messages change in the local database.
@MainActor
@Observable
final class ConversationListViewModel {
    var conversations: [ConversationRow] = []
    var isLoading: Bool = false
    private var observationTask: Task<Void, Never>?
    private let repository: ThreadRepository

    init(repository: ThreadRepository) {
        self.repository = repository
    }

    func startObserving() {
        observationTask = Task {
            isLoading = true
            let observation = repository.observeConversations()
            do {
                for try await rows in observation.values(in: repository.dbPool) {
                    self.conversations = rows
                    self.isLoading = false
                }
            } catch {
                self.isLoading = false
            }
        }
    }

    func stopObserving() {
        observationTask?.cancel()
        observationTask = nil
    }

    /// Extract preview text from contentJson. Decodes first content block's fallback text.
    func previewText(for row: ConversationRow) -> String {
        guard let json = row.lastMessageContent,
              let data = json.data(using: .utf8),
              let blocks = try? JSONDecoder().decode([ContentBlock].self, from: data),
              let first = blocks.first else {
            return ""
        }
        return first.fallbackText
    }

    /// Format relative timestamp from RFC 3339 string using RelativeDateTimeFormatter.
    func relativeTimestamp(for dateString: String?) -> String {
        guard let dateString else { return "" }
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        guard let date = formatter.date(from: dateString) else {
            // Try without fractional seconds
            let basic = ISO8601DateFormatter()
            basic.formatOptions = [.withInternetDateTime]
            guard let d = basic.date(from: dateString) else { return "" }
            return relativeString(from: d)
        }
        return relativeString(from: date)
    }

    private func relativeString(from date: Date) -> String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}
