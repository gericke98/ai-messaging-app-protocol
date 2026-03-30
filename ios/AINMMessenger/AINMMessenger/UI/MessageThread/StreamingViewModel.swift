import Foundation
import Observation

/// Observable view model bridging StreamingManager to SwiftUI with throttled updates.
/// Maintains ephemeral streaming message state that renders below permanent GRDB messages.
/// When stream.end arrives, transitions to "finalizing" state; if the signed message
/// does not arrive within 5 seconds, shows "Not verified" badge.
@MainActor
@Observable
final class StreamingViewModel {

    /// Represents the UI state of a single in-progress streaming message.
    struct StreamingMessageState: Identifiable, Sendable {
        let id: String          // streamId
        let messageId: String
        let from: String
        let threadId: String
        var blocks: [StreamingBlockState]
        var status: StreamStatus = .streaming
        var finalizingStartTime: Date?

        enum StreamStatus: Sendable {
            case streaming
            case cancelling
            case finalizing
            case notVerified
        }
    }

    /// State of a single content block within a streaming message.
    struct StreamingBlockState: Sendable {
        let type: String   // "text" or "code"
        var content: String = ""
        var language: String?
    }

    /// All active streaming messages, keyed by streamId.
    var activeStreams: [String: StreamingMessageState] = [:]

    // MARK: - Stream Lifecycle

    /// Called when a new stream starts. Creates the ephemeral placeholder.
    func streamStarted(_ stream: StreamingManager.ActiveStream) {
        let blocks = stream.blockLayout.map { layout in
            StreamingBlockState(type: layout.type, content: "", language: nil)
        }
        let state = StreamingMessageState(
            id: stream.streamId,
            messageId: stream.messageId,
            from: stream.from,
            threadId: stream.threadId,
            blocks: blocks
        )
        activeStreams[stream.streamId] = state
    }

    /// Called on each delta. Updates the accumulated content for a specific block.
    func updateStreamContent(streamId: String, blockIndex: Int, content: String) {
        guard var stream = activeStreams[streamId],
              blockIndex >= 0 && blockIndex < stream.blocks.count else { return }
        stream.blocks[blockIndex].content = content
        activeStreams[streamId] = stream
    }

    /// Update the language hint for a code block during streaming.
    func updateBlockLanguage(streamId: String, blockIndex: Int, language: String) {
        guard var stream = activeStreams[streamId],
              blockIndex >= 0 && blockIndex < stream.blocks.count else { return }
        stream.blocks[blockIndex].language = language
        activeStreams[streamId] = stream
    }

    /// Mark a stream as cancelling (user tapped Stop Generating).
    func markCancelling(streamId: String) {
        activeStreams[streamId]?.status = .cancelling
    }

    /// Called when stream.end arrives. Transitions to finalizing state.
    /// Starts a 5-second timeout: if the signed permanent message does not arrive,
    /// transitions to .notVerified to display the "Not verified" badge.
    func streamEnded(streamId: String, status: String) {
        guard var stream = activeStreams[streamId] else { return }
        stream.status = .finalizing
        stream.finalizingStartTime = Date()
        activeStreams[streamId] = stream

        // 5-second verification timeout
        let capturedId = streamId
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(5))
            if let s = activeStreams[capturedId], s.status == .finalizing {
                activeStreams[capturedId]?.status = .notVerified
            }
        }
    }

    /// Remove a stream entirely (e.g., on error).
    func removeStream(streamId: String) {
        activeStreams.removeValue(forKey: streamId)
    }

    /// Remove streaming placeholder when the permanent message arrives (matched by messageId).
    /// Called by MessageThreadViewModel when GRDB observation delivers a new message
    /// whose ID matches a streaming placeholder's messageId.
    func removePlaceholderForMessage(messageId: String) {
        if let key = activeStreams.first(where: { $0.value.messageId == messageId })?.key {
            activeStreams.removeValue(forKey: key)
        }
    }

    // MARK: - Queries

    /// Get all active streaming messages for a given thread, sorted by streamId.
    func activeStreamsForThread(_ threadId: String) -> [StreamingMessageState] {
        activeStreams.values.filter { $0.threadId == threadId }.sorted { $0.id < $1.id }
    }

    /// Check if there are any actively streaming messages in a thread.
    func hasActiveStreams(inThread threadId: String) -> Bool {
        activeStreams.values.contains { $0.threadId == threadId && $0.status == .streaming }
    }
}
