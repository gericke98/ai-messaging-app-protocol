import SwiftUI

/// Renders a group of consecutive messages from the same sender.
/// Per D-03: avatar+name on first message only, 4pt spacing within group, 16pt between groups.
struct MessageGroupView: View {
    let group: MessageGroup
    let contentBlocksProvider: (MessageRecord) -> [ContentBlock]
    let timestampFormatter: (String) -> String
    var onAction: ((_ messageId: String, _ blockIndex: Int, _ actionId: String, _ data: [String: Any]) -> Void)? = nil

    var body: some View {
        VStack(alignment: group.isFromCurrentUser ? .trailing : .leading, spacing: 4) {
            ForEach(Array(group.messages.enumerated()), id: \.element.id) { index, message in
                let blocks = contentBlocksProvider(message)
                let isFirst = index == 0
                let isLast = index == group.messages.count - 1

                if group.isFromCurrentUser {
                    MessageBubbleView(
                        blocks: blocks,
                        senderName: group.senderName,
                        showHeader: isFirst,
                        timestamp: isLast ? timestampFormatter(message.timestamp) : nil
                    )
                } else {
                    AgentMessageView(
                        blocks: blocks,
                        senderName: group.senderName,
                        senderType: group.senderType,
                        showHeader: isFirst,
                        timestamp: isLast ? timestampFormatter(message.timestamp) : nil,
                        onAction: { blockIndex, actionId, data in
                            onAction?(message.id, blockIndex, actionId, data)
                        }
                    )
                }
            }
        }
        .padding(.horizontal, 16)
    }
}
