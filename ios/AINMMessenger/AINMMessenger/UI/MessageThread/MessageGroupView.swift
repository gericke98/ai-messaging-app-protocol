import SwiftUI

/// Renders a group of consecutive messages from the same sender.
/// Per D-03: avatar+name on first message only, 4pt spacing within group, 16pt between groups.
struct MessageGroupView: View {
    let group: MessageGroup
    let contentBlocksProvider: (MessageRecord) -> [ContentBlock]
    let timestampFormatter: (String) -> String
    var onAction: ((_ messageId: String, _ blockIndex: Int, _ actionId: String, _ data: [String: Any]) -> Void)? = nil
    /// Whether we are inside a sub-thread (hides "Branch Thread" per one-level limit).
    var isInSubThread: Bool = false
    /// Returns (subThreadId, replyCount) for a message that has a sub-thread, or nil.
    var subThreadInfo: ((_ messageId: String) -> (threadId: String, replyCount: Int)?)? = nil
    /// Called when user taps sub-thread indicator or branches a new thread.
    var onNavigateToSubThread: ((_ threadId: String) -> Void)? = nil
    /// Called when user long-presses to branch a thread from a message.
    var onBranchThread: ((_ messageId: String) -> Void)? = nil

    var body: some View {
        VStack(alignment: group.isFromCurrentUser ? .trailing : .leading, spacing: 4) {
            ForEach(Array(group.messages.enumerated()), id: \.element.id) { index, message in
                let blocks = contentBlocksProvider(message)
                let isFirst = index == 0
                let isLast = index == group.messages.count - 1

                VStack(alignment: group.isFromCurrentUser ? .trailing : .leading, spacing: 2) {
                    if group.isFromCurrentUser {
                        MessageBubbleView(
                            blocks: blocks,
                            senderName: group.senderName,
                            showHeader: isFirst,
                            timestamp: isLast ? timestampFormatter(message.timestamp) : nil,
                            messageId: message.id
                        )
                    } else {
                        AgentMessageView(
                            blocks: blocks,
                            senderName: group.senderName,
                            senderType: group.senderType,
                            senderAddress: group.senderAddress,
                            showHeader: isFirst,
                            timestamp: isLast ? timestampFormatter(message.timestamp) : nil,
                            onAction: { blockIndex, actionId, data in
                                onAction?(message.id, blockIndex, actionId, data)
                            }
                        )
                    }

                    // Sub-thread indicator badge below messages that have sub-threads
                    if let info = subThreadInfo?(message.id) {
                        SubThreadIndicatorView(
                            replyCount: info.replyCount,
                            parentMessageId: message.id
                        ) {
                            onNavigateToSubThread?(info.threadId)
                        }
                    }
                }
                .accessibilityIdentifier(UITestID.SubThread.messageRow(messageId: message.id))
                .contextMenu {
                    if !isInSubThread {
                        Button {
                            onBranchThread?(message.id)
                        } label: {
                            Label("Branch Thread", systemImage: "arrow.branch")
                        }
                    }
                }
            }
        }
        .padding(.horizontal, 16)
    }
}
