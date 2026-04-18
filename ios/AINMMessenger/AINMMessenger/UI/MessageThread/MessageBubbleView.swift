import SwiftUI

/// Right-aligned human message chat bubble per UI-SPEC D-01.
/// Accent background, white text, 12pt corners with 4pt bottom-right tail.
struct MessageBubbleView: View {
    let blocks: [ContentBlock]
    let senderName: String
    var showHeader: Bool = false
    var timestamp: String?
    /// Stable message id for UI test targeting (scenarios 5 + 6).
    /// Default empty for preview/placeholder use; real call sites pass message.id.
    var messageId: String = ""

    var body: some View {
        HStack {
            Spacer()
            VStack(alignment: .trailing, spacing: 4) {
                if showHeader {
                    Text(senderName)
                        .font(.subheadline.bold())
                        .foregroundStyle(.white)
                }
                ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in
                    ContentBlockRenderer(block: block, isHumanBubble: true)
                }
                if let timestamp {
                    Text(timestamp)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .frame(maxWidth: UIScreen.main.bounds.width * 0.75, alignment: .trailing)
            .background(Color.accentColor)
            .clipShape(UnevenRoundedRectangle(topLeadingRadius: 12, bottomLeadingRadius: 12, bottomTrailingRadius: 4, topTrailingRadius: 12))
            .accessibilityIdentifier(UITestID.SubThread.messageRow(messageId: messageId))
        }
    }
}
