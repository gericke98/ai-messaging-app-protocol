import SwiftUI

/// Right-aligned human message bubble (UI-SPEC D-01), Editorial Warm.
/// Ink background, paper text, 16pt corners with a 4pt bottom-right tail.
/// Max width adapts to the real container width (split-view/iPad/rotation safe).
struct MessageBubbleView: View {
    let blocks: [ContentBlock]
    let senderName: String
    var showHeader: Bool = false
    var timestamp: String?
    /// Stable message id for UI test targeting (scenarios 5 + 6).
    var messageId: String = ""

    @State private var containerWidth: CGFloat = 0

    var body: some View {
        HStack {
            Spacer(minLength: Theme.Spacing.xl)
            VStack(alignment: .trailing, spacing: Theme.Spacing.xs) {
                if showHeader {
                    Text(senderName)
                        .font(Theme.Typography.label())
                        .foregroundStyle(Theme.Color.humanBubbleText)
                }
                ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in
                    ContentBlockRenderer(block: block, isHumanBubble: true)
                }
                if let timestamp {
                    Text(timestamp)
                        .font(Theme.Typography.caption())
                        .foregroundStyle(Theme.Color.humanBubbleText.opacity(0.7))
                }
            }
            .padding(.horizontal, Theme.Spacing.md)
            .padding(.vertical, Theme.Spacing.sm)
            .frame(maxWidth: containerWidth > 0 ? containerWidth * 0.75 : nil, alignment: .trailing)
            .background(Theme.Color.humanBubble)
            .clipShape(UnevenRoundedRectangle(
                topLeadingRadius: Theme.Radius.bubble,
                bottomLeadingRadius: Theme.Radius.bubble,
                bottomTrailingRadius: Theme.Spacing.xs,
                topTrailingRadius: Theme.Radius.bubble
            ))
            .accessibilityIdentifier(UITestID.SubThread.messageRow(messageId: messageId))
        }
        .background {
            GeometryReader { proxy in
                Color.clear
                    .onAppear { containerWidth = proxy.size.width }
                    .onChange(of: proxy.size.width) { _, w in containerWidth = w }
            }
        }
    }
}
