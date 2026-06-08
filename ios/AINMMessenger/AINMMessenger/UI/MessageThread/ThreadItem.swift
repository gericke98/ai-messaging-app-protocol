import Foundation

/// One renderable row in the message thread: either a normal sender group or a
/// collapsed A2A digest capsule. Identifiable with prefixed ids so a capsule (whose
/// id equals its first group's id) never collides with that group's own id in a ForEach.
enum ThreadItem: Identifiable {
    case group(MessageGroup)
    case capsule(A2ACapsule)

    var id: String {
        switch self {
        case .group(let g):   return "g-\(g.id)"
        case .capsule(let c): return "c-\(c.id)"
        }
    }
}

/// A folded run of consecutive agent-to-agent message groups (≥3 messages, ≥2 distinct
/// agents, in a multi-participant thread, bounded by a non-A2A item). Display-only.
struct A2ACapsule: Identifiable {
    let id: String              // == first folded group's id (stable across rebuilds)
    let groups: [MessageGroup]  // the folded groups, in order
    let agents: [String]        // distinct agent addresses, first-seen order
    let messageCount: Int       // total MessageRecords across the run
}
