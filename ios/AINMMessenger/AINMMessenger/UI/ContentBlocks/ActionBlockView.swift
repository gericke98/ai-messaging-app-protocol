import SwiftUI

/// Renders an ActionBlock as a compact read-only inline record.
/// Per UI-SPEC Component 9: arrow icon, italic fallback text, no background.
/// ActionBlocks are action response records -- display-only, no interaction needed.
struct ActionBlockView: View {
    let content: ContentBlock.ActionContent

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "arrow.turn.down.right")
                .font(.system(size: 14))
                .foregroundStyle(.secondary)

            Text(content.fallback)
                .font(.subheadline)
                .italic()
                .foregroundStyle(.secondary)
        }
        .padding(.vertical, 4)
    }
}
