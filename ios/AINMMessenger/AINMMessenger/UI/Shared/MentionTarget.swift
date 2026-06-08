import Foundation

/// Pure helper: extract the first mentioned address from a message's content blocks.
/// "First" = the lowest-offset mention in the first text block that has any mention.
enum MentionTarget {

    static func firstMentionAddress(in blocks: [ContentBlock]) -> String? {
        for block in blocks {
            if case .text(let content) = block,
               let mentions = content.mentions, !mentions.isEmpty {
                return mentions.min(by: { $0.offset < $1.offset })?.address
            }
        }
        return nil
    }
}
