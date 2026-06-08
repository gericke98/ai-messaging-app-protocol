import SwiftUI

/// Left-aligned agent/other-user message container per UI-SPEC D-01, D-02.
/// Full-width flat container with secondary background, agent badge when senderType == "agent".
struct AgentMessageView: View {
    let blocks: [ContentBlock]
    let senderName: String
    let senderType: String
    var senderAddress: String? = nil
    var showHeader: Bool = false
    var timestamp: String?
    var onAction: ((_ blockIndex: Int, _ actionId: String, _ data: [String: Any]) -> Void)? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if showHeader {
                HStack(spacing: 6) {
                    AvatarView(
                        participantType: senderType,
                        size: 24,
                        address: senderAddress,
                        displayName: senderName
                    )
                    Text(senderName)
                        .font(.subheadline.bold())
                    if senderType == "agent" {
                        Text("Agent")
                            .font(.caption.bold())
                            .foregroundStyle(.secondary)
                    }
                }
            }
            ForEach(Array(blocks.enumerated()), id: \.offset) { index, block in
                ContentBlockRenderer(block: block, isHumanBubble: false, onAction: { actionId, data in
                    onAction?(index, actionId, data)
                })
            }
            if let timestamp {
                Text(timestamp)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(12)
        .background(Color(.secondarySystemGroupedBackground))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }
}
