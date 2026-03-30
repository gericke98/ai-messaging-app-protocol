import Foundation

/// URL scheme parser for ainm:// deep links.
/// Supports navigating to threads and specific messages within threads.
enum DeepLinkHandler {
    enum Route {
        case thread(threadId: String)
        case message(threadId: String, messageId: String)
    }

    /// Parse an ainm:// URL into a navigation route.
    /// Supported patterns:
    ///   ainm://thread/{thread_id}
    ///   ainm://thread/{thread_id}/message/{message_id}
    static func parse(_ url: URL) -> Route? {
        guard url.scheme == "ainm" else { return nil }

        let urlString = url.absoluteString
        guard urlString.hasPrefix("ainm://thread/") else { return nil }
        let remainder = String(urlString.dropFirst("ainm://thread/".count))
        let parts = remainder.components(separatedBy: "/")

        guard let threadId = parts.first, !threadId.isEmpty else { return nil }

        if parts.count >= 3, parts[1] == "message", !parts[2].isEmpty {
            return .message(threadId: threadId, messageId: parts[2])
        }

        return .thread(threadId: threadId)
    }
}
