import Foundation

/// Pure decision: classify a thread participant as "human" or "agent" using the only
/// AUTHORITATIVE signals available client-side (there is no sender-type on the wire).
/// Precedence: self → human; then the thread's agent address; then a card-resolved saved
/// agent; else default human.
///
/// Note: the per-message content `source` is deliberately NOT used. It is sender-controlled
/// and the only signal that could permanently mislabel a human (e.g. one relaying agent-
/// authored content) given the human→agent upgrade is non-reversible. An un-saved federated
/// agent that matches none of the authoritative signals classifies as human until it is
/// saved / resolved — a real authoritative sender-type (card resolution or server-provided)
/// is the right answer there, not `source`.
enum ParticipantClassifier {

    static func classify(
        address: String,
        isSelf: Bool,
        agentAddress: String?,
        savedAgentAddresses: Set<String>
    ) -> String {
        if isSelf { return "human" }
        if let agentAddress, address == agentAddress { return "agent" }
        if savedAgentAddresses.contains(address) { return "agent" }
        return "human"
    }
}
