import SwiftUI

/// Badge on messages that have sub-threads branching from them.
/// Tapping navigates to the sub-thread. Per UI-SPEC Component 8.
struct SubThreadIndicatorView: View {
    let replyCount: Int
    /// Id of the parent message this indicator is attached to.
    /// Used for UI-test targeting (scenario 5 reply-badge query).
    let parentMessageId: String
    var onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 4) {
                Image(systemName: "bubble.left.and.bubble.right")
                    .font(.system(size: 12))
                    .foregroundStyle(Color(.secondaryLabel))
                Text("\(replyCount) replies")
                    .font(.caption.bold())
                    .foregroundStyle(Color.accentColor)
            }
            .padding(.vertical, 4)
            .padding(.horizontal, 8)
            .background(Color(.tertiarySystemFill))
            .clipShape(RoundedRectangle(cornerRadius: 6))
        }
        .buttonStyle(.plain)
        .padding(.leading, 12)
        .accessibilityLabel("\(replyCount) replies in sub-thread")
        .accessibilityHint("Double tap to open sub-thread")
        .accessibilityIdentifier(UITestID.SubThread.replyBadge(messageId: parentMessageId))
    }
}
