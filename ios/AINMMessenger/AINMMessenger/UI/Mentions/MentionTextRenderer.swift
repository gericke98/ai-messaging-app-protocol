import SwiftUI

/// Applies visual highlights to mention spans in attributed text.
/// Per UI-SPEC: accent blue bold for agent/default, white bold in human bubbles.
enum MentionTextRenderer {
    /// Build an AttributedString from text content with mention highlights.
    /// Processes mentions in reverse offset order to avoid index shifting.
    static func highlightMentions(
        in attributed: AttributedString,
        mentions: [ContentBlock.Mention]?,
        isHumanBubble: Bool
    ) -> AttributedString {
        guard let mentions = mentions, !mentions.isEmpty else { return attributed }
        var result = attributed

        // Sort by offset descending to avoid index shifting
        for mention in mentions.sorted(by: { $0.offset > $1.offset }) {
            // Bounds check
            let startOffset = mention.offset
            let endOffset = mention.offset + mention.length
            guard startOffset >= 0,
                  endOffset <= result.characters.count else { continue }

            let startIndex = result.characters.index(
                result.characters.startIndex, offsetBy: startOffset
            )
            let endIndex = result.characters.index(
                result.characters.startIndex, offsetBy: endOffset
            )
            let range = startIndex ..< endIndex

            // Apply highlight style per UI-SPEC
            result[range].foregroundColor = isHumanBubble ? .white : .accentColor
            result[range].font = .body.bold()
        }
        return result
    }
}
