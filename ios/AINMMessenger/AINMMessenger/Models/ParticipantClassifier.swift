import Foundation

/// Pure decision: classify a thread participant as "human" or "agent" using the only
/// signals available client-side (there is no authoritative sender-type on the wire).
/// Precedence: self → human; then thread's agent address; then a card-resolved saved
/// agent; then the per-message content `source`; else default human.
enum ParticipantClassifier {

    static func classify(
        address: String,
        isSelf: Bool,
        agentAddress: String?,
        savedAgentAddresses: Set<String>,
        contentSource: String?
    ) -> String {
        if isSelf { return "human" }
        if let agentAddress, address == agentAddress { return "agent" }
        if savedAgentAddresses.contains(address) { return "agent" }
        if contentSource == "agent" { return "agent" }
        return "human"
    }
}
