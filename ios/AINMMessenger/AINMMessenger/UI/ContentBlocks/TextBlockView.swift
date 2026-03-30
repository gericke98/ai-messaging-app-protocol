import SwiftUI

/// Renders a TextBlock with Markdown support via AttributedString.
/// Supports bold, italic, code spans, and tappable links.
/// Accepts isHumanBubble to set white text on accent background.
struct TextBlockView: View {
    let content: ContentBlock.TextContent
    var isHumanBubble: Bool = false

    var body: some View {
        Group {
            if let attributed = try? AttributedString(
                markdown: content.text,
                options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
            ) {
                let styled = applyBubbleStyle(to: attributed)
                let highlighted = MentionTextRenderer.highlightMentions(
                    in: styled,
                    mentions: content.mentions,
                    isHumanBubble: isHumanBubble
                )
                Text(highlighted)
                    .font(.body)
            } else {
                Text(content.text)
                    .font(.body)
                    .foregroundStyle(isHumanBubble ? Color.white : Color(.label))
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func applyBubbleStyle(to attributed: AttributedString) -> AttributedString {
        var result = attributed
        if isHumanBubble {
            result.foregroundColor = .white
        }
        return result
    }
}
