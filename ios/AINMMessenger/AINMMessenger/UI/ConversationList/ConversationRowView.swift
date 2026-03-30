import SwiftUI

/// Single conversation row for the conversation list per UI-SPEC Screen 1.
/// Shows avatar, thread title, last message preview, relative timestamp, and unread dot.
struct ConversationRowView: View {
    let conversation: ConversationRow
    let previewText: String
    let relativeTime: String

    var body: some View {
        HStack(spacing: 12) {
            // Leading avatar -- default to "human" for v1
            AvatarView(participantType: "human", size: 36)

            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Text(conversation.title ?? "Untitled")
                        .font(.subheadline.bold())
                        .lineLimit(1)
                    Spacer()
                    Text(relativeTime)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                HStack {
                    Text(previewText)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                    Spacer(minLength: 0)
                    if conversation.hasUnread {
                        Circle()
                            .fill(Color.accentColor)
                            .frame(width: 8, height: 8)
                            .accessibilityLabel("Unread")
                    }
                }
            }
        }
        .padding(.vertical, 8)
        .padding(.horizontal, 16)
        .frame(minHeight: 64)
        .contentShape(Rectangle())
    }
}
