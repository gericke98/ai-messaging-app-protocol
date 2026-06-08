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
    var directedTo: String? = nil
    var isMultiParticipant: Bool = false
    var homeHost: String? = nil
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
                    ParticipantByline(
                        senderAddress: senderAddress ?? senderName,
                        senderName: senderName,
                        directedTo: directedTo,
                        isMultiParticipant: isMultiParticipant,
                        homeHost: homeHost
                    )
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
