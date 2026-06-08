import SwiftUI

/// Left-aligned agent / other-participant message (UI-SPEC D-01, D-02), Editorial Warm.
/// Airy: no wrapping card. Identity avatar + serif sender name on the first message of a
/// run; serif prose laid open on the paper. Rich blocks keep their own tokenized container.
struct AgentMessageView: View {
    let blocks: [ContentBlock]
    let senderName: String
    let senderType: String
    var senderAddress: String? = nil
    var showHeader: Bool = false
    var timestamp: String?
    var onAction: ((_ blockIndex: Int, _ actionId: String, _ data: [String: Any]) -> Void)? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.Spacing.xs) {
            if showHeader {
                HStack(spacing: Theme.Spacing.sm) {
                    AvatarView(
                        participantType: senderType,
                        size: 30,
                        address: senderAddress,
                        displayName: senderName
                    )
                    Text(senderName)
                        .font(Theme.Typography.senderName())
                        .foregroundStyle(Theme.Color.inkPrimary)
                }
            }
            ForEach(Array(blocks.enumerated()), id: \.offset) { index, block in
                ContentBlockRenderer(block: block, isHumanBubble: false, onAction: { actionId, data in
                    onAction?(index, actionId, data)
                })
            }
            if let timestamp {
                Text(timestamp)
                    .font(Theme.Typography.caption())
                    .foregroundStyle(Theme.Color.inkSecondary)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
