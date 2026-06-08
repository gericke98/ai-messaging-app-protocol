import SwiftUI

/// Single conversation row (UI-SPEC Screen 1), Editorial Warm.
/// Identity-colored avatar (legacy fallback when no address), serif title,
/// warm preview/timestamp, accent unread dot.
struct ConversationRowView: View {
    let conversation: ConversationRow
    let previewText: String
    let relativeTime: String

    var body: some View {
        HStack(spacing: Theme.Spacing.md) {
            AvatarView(
                participantType: participantType,
                size: 36,
                address: rowAddress,
                displayName: conversation.title
            )

            VStack(alignment: .leading, spacing: Theme.Spacing.xs) {
                HStack {
                    Text(conversation.title ?? "Untitled")
                        .font(Theme.Typography.senderName())
                        .foregroundStyle(Theme.Color.inkPrimary)
                        .lineLimit(1)
                    Spacer()
                    Text(relativeTime)
                        .font(Theme.Typography.caption())
                        .foregroundStyle(Theme.Color.inkSecondary)
                }
                HStack {
                    Text(previewText)
                        .font(Theme.Typography.caption())
                        .foregroundStyle(Theme.Color.inkSecondary)
                        .lineLimit(2)
                    Spacer(minLength: 0)
                    if conversation.hasUnread {
                        Circle()
                            .fill(Theme.Color.accent)
                            .frame(width: 8, height: 8)
                            .accessibilityLabel("Unread")
                    }
                }
            }
        }
        .padding(.vertical, Theme.Spacing.sm)
        .padding(.horizontal, Theme.Spacing.lg)
        .frame(minHeight: 64)
        .contentShape(Rectangle())
    }

    /// Address driving the identity avatar: the thread's primary agent address
    /// (LOOP-1 `thread.agentAddress`). nil → legacy avatar fallback (no regression).
    private var rowAddress: String? {
        conversation.agentAddress
    }

    /// Legacy-fallback glyph type (only used by AvatarView when `rowAddress` is nil):
    /// an agent thread (has a primary agent address) → "agent"; otherwise a human thread.
    private var participantType: String {
        conversation.agentAddress != nil ? "agent" : "human"
    }
}
