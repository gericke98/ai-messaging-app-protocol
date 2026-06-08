import Foundation

/// Pure folding pass: turns the sender-grouped thread into a render list where finished
/// agent-to-agent bursts become collapsed capsules. Trigger (all required):
/// multi-participant thread, a *bounded* run of consecutive A2A groups (an A2A group is
/// agent-sent, not from the current user, and not directed at the current user), ≥3 messages
/// total, ≥2 distinct agent senders. The trailing/unbounded run stays inline. Host-free.
enum A2ACapsuleFolder {

    static func fold(
        groups: [MessageGroup],
        currentUserIdentity: String,
        isMultiParticipant: Bool
    ) -> [ThreadItem] {
        guard isMultiParticipant else { return groups.map { .group($0) } }

        func isA2A(_ g: MessageGroup) -> Bool {
            g.senderType == "agent" && !g.isFromCurrentUser && g.directedTo != currentUserIdentity
        }

        var items: [ThreadItem] = []
        var i = 0
        let n = groups.count
        while i < n {
            guard isA2A(groups[i]) else {
                items.append(.group(groups[i]))
                i += 1
                continue
            }
            var j = i
            while j < n, isA2A(groups[j]) { j += 1 }
            let run = Array(groups[i..<j])
            let bounded = j < n
            let messageCount = run.reduce(0) { $0 + $1.messages.count }
            let agents = orderedDistinct(run.map(\.senderAddress))

            if bounded, messageCount >= 3, agents.count >= 2 {
                items.append(.capsule(A2ACapsule(
                    id: run[0].id, groups: run, agents: agents, messageCount: messageCount)))
            } else {
                items.append(contentsOf: run.map { .group($0) })
            }
            i = j
        }
        return items
    }

    private static func orderedDistinct(_ xs: [String]) -> [String] {
        var seen = Set<String>()
        var out: [String] = []
        for x in xs where seen.insert(x).inserted { out.append(x) }
        return out
    }
}
