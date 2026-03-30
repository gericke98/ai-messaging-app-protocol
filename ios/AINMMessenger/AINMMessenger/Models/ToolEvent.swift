import Foundation
import SwiftUI

/// Wire-format event for tool call lifecycle notifications.
/// Decoded from `ainm.tool.call.started`, `ainm.tool.call.progress`, and
/// `ainm.tool.call.completed` JSON-RPC notifications.
struct ToolCallEvent: Codable, Sendable {
    let callId: String
    let name: String?
    let description: String?
    let input: [String: AnyCodableValue]?
    let progress: ToolProgress?
    let resultSummary: String?
    let durationMs: Int?
    let status: String?         // "success" or "error"
    let errorMessage: String?

    struct ToolProgress: Codable, Sendable {
        let current: Int?
        let total: Int?
        let detail: String?
    }

    enum CodingKeys: String, CodingKey {
        case callId = "call_id"
        case name, description, input, progress
        case resultSummary = "result_summary"
        case durationMs = "duration_ms"
        case status
        case errorMessage = "error_message"
    }
}

// MARK: - Duration Formatting (UI-SPEC Section 5.4)

extension Int {
    /// Format millisecond duration for display in tool call indicators.
    /// <1s: "123ms", <1m: "1.2s", >=1m: "2m 15s"
    var formattedDuration: String {
        if self < 1000 {
            return "\(self)ms"
        } else if self < 60_000 {
            return String(format: "%.1fs", Double(self) / 1000.0)
        } else {
            let m = self / 60_000
            let s = (self % 60_000) / 1000
            return "\(m)m \(s)s"
        }
    }
}

// MARK: - Client-Side Lifecycle State

/// Observable state for a single tool call, driving ToolCallIndicatorView.
/// Keyed by call_id, updated as lifecycle events arrive.
@MainActor @Observable
final class ToolCallState: Identifiable {
    let id: String  // call_id
    var name: String
    var status: ToolCallStatus
    var input: [String: AnyCodableValue]?
    var progress: ToolCallEvent.ToolProgress?
    var resultSummary: String?
    var durationMs: Int?
    var errorMessage: String?
    var isExpanded: Bool = false
    var startedAt: Date = Date()

    enum ToolCallStatus: String, Sendable {
        case running, completed, error, timedOut
    }

    init(
        id: String,
        name: String,
        status: ToolCallStatus = .running,
        input: [String: AnyCodableValue]? = nil
    ) {
        self.id = id
        self.name = name
        self.status = status
        self.input = input
    }
}
