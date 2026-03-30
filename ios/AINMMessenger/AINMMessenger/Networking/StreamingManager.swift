import Foundation

/// Actor that manages active streaming sessions, accumulates deltas per block,
/// and handles stream lifecycle (start, delta, end, error, cancel).
///
/// Thread-safe by design: Swift actor isolation prevents data races on
/// the `activeStreams` dictionary and all mutable state.
actor StreamingManager {

    /// Represents an in-progress streaming response from an agent.
    /// Each stream contains one or more blocks that receive independent deltas.
    struct ActiveStream: Sendable {
        let streamId: String
        let messageId: String
        let from: String
        let threadId: String
        let timestamp: String
        let blockLayout: [StreamStartParams.BlockLayout]
        var accumulatedContent: [String]   // indexed by block position
        var language: [String?]            // language hint per block
    }

    /// All currently active streams, keyed by streamId.
    private var activeStreams: [String: ActiveStream] = [:]

    // MARK: - Lifecycle Handlers

    /// Register a new stream from a `stream.start` notification.
    /// Returns the newly created ActiveStream for UI initialization.
    @discardableResult
    func handleStreamStart(_ params: StreamStartParams) -> ActiveStream {
        let stream = ActiveStream(
            streamId: params.streamId,
            messageId: params.messageId,
            from: params.from,
            threadId: params.threadId,
            timestamp: params.timestamp,
            blockLayout: params.blocks,
            accumulatedContent: Array(repeating: "", count: params.blocks.count),
            language: Array(repeating: nil, count: params.blocks.count)
        )
        activeStreams[params.streamId] = stream
        return stream
    }

    /// Accumulate a delta for a specific block index.
    /// Returns the streamId, blockIndex, and full accumulated content for that block,
    /// or nil if the stream/index is invalid.
    func handleDelta(_ params: StreamDeltaParams) -> (streamId: String, blockIndex: Int, fullContent: String)? {
        guard var stream = activeStreams[params.streamId],
              params.blockIndex >= 0 && params.blockIndex < stream.accumulatedContent.count else { return nil }
        stream.accumulatedContent[params.blockIndex] += params.delta
        if let lang = params.language {
            stream.language[params.blockIndex] = lang
        }
        activeStreams[params.streamId] = stream
        return (params.streamId, params.blockIndex, stream.accumulatedContent[params.blockIndex])
    }

    /// Finalize a stream on `stream.end`. Removes from active tracking.
    /// Returns the final ActiveStream state for persistence, or nil if not found.
    func handleStreamEnd(_ params: StreamEndParams) -> ActiveStream? {
        return activeStreams.removeValue(forKey: params.streamId)
    }

    /// Handle a stream error. Removes from active tracking.
    /// Returns the stream state at time of error for logging/UI, or nil if not found.
    func handleStreamError(_ params: StreamErrorParams) -> ActiveStream? {
        return activeStreams.removeValue(forKey: params.streamId)
    }

    // MARK: - Cancel Support

    /// Check if a stream exists (for cancel validation before sending the request).
    /// Returns true if the stream is currently active.
    func cancelStream(_ streamId: String) -> Bool {
        return activeStreams[streamId] != nil
    }

    // MARK: - Queries

    /// Get the current state of an active stream.
    func getActiveStream(_ streamId: String) -> ActiveStream? {
        return activeStreams[streamId]
    }

    /// Get all active stream IDs for a given thread.
    /// Used by `cancelAllStreams(inThread:)` on AINMClient.
    func activeStreamIds(forThread threadId: String) -> [String] {
        return activeStreams.values.filter { $0.threadId == threadId }.map(\.streamId)
    }

    // MARK: - Cleanup

    /// Remove all active streams. Called on disconnect to reset state.
    func clearAllStreams() {
        activeStreams.removeAll()
    }
}
