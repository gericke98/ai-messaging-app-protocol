import SwiftUI

/// Dispatches ContentBlock enum cases to the appropriate view component.
/// Per D-10: multi-block messages render as vertical stack, each block full available width.
struct ContentBlockRenderer: View {
    let block: ContentBlock
    var isHumanBubble: Bool = false

    var body: some View {
        switch block {
        case .text(let content):
            TextBlockView(content: content, isHumanBubble: isHumanBubble)
        case .code(let content):
            CodeBlockView(content: content)
        case .image(let content):
            ImageBlockView(content: content)
        case .file(let content):
            FileBlockView(content: content)
        case .unknown(let fallback):
            Text(fallback)
                .font(.body)
                .foregroundStyle(.secondary)
        }
    }
}
