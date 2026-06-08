import SwiftUI

/// Renders a TextBlock with Markdown (bold/italic/code/links) via AttributedString.
/// Agent prose uses the editorial serif on ink; human-bubble text uses SF on the
/// human-bubble text color.
struct TextBlockView: View {
    let content: ContentBlock.TextContent
    var isHumanBubble: Bool = false

    private var proseFont: Font { isHumanBubble ? Theme.Typography.body() : Theme.Typography.agentProse() }
    private var proseColor: Color { isHumanBubble ? Theme.Color.humanBubbleText : Theme.Color.inkPrimary }

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
                    .font(proseFont)
                    .foregroundStyle(proseColor)
            } else {
                Text(content.text)
                    .font(proseFont)
                    .foregroundStyle(proseColor)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func applyBubbleStyle(to attributed: AttributedString) -> AttributedString {
        var result = attributed
        result.foregroundColor = isHumanBubble ? Theme.Color.humanBubbleText : Theme.Color.inkPrimary
        return result
    }
}
