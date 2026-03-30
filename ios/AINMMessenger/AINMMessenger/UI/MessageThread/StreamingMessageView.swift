import SwiftUI

/// In-progress streaming message rendering (UI-SPEC Component 1).
/// Renders an ephemeral agent message with independently streaming blocks,
/// a blinking cursor at the end of the active block, and a "Not verified" badge
/// when the signed permanent message does not arrive within 5 seconds of stream.end.
struct StreamingMessageView: View {
    let state: StreamingViewModel.StreamingMessageState

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            // Agent avatar (same AvatarView from Phase 11)
            AvatarView(participantType: "agent")
                .frame(width: 36, height: 36)

            VStack(alignment: .leading, spacing: 4) {
                // Sender name + "Agent" badge
                HStack(spacing: 4) {
                    Text(senderName)
                        .font(.subheadline.bold())
                    Text("Agent")
                        .font(.caption2)
                        .padding(.horizontal, 4)
                        .padding(.vertical, 2)
                        .background(Color(.tertiarySystemFill))
                        .clipShape(RoundedRectangle(cornerRadius: 4))
                }

                // Streaming blocks in vertical stack
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(Array(state.blocks.enumerated()), id: \.offset) { index, block in
                        streamingBlockView(
                            block: block,
                            isLast: index == state.blocks.count - 1
                        )
                    }
                }

                // Not verified badge (shown after 5s timeout)
                if state.status == .notVerified {
                    HStack(spacing: 4) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .font(.caption)
                        Text("Not verified")
                            .font(.caption)
                    }
                    .foregroundStyle(.secondary)
                    .accessibilityLabel("Message not verified")
                    .accessibilityHint("The signed message was not received from the server within the expected time.")
                }
            }
        }
        .padding(.horizontal, 16)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(state.status == .streaming ? "Agent is generating a response" : "Agent response")
    }

    // MARK: - Private

    private var senderName: String {
        state.from.components(separatedBy: "@").first ?? state.from
    }

    @ViewBuilder
    private func streamingBlockView(
        block: StreamingViewModel.StreamingBlockState,
        isLast: Bool
    ) -> some View {
        if block.type == "code" {
            // Code block with language label and monospaced font
            VStack(alignment: .leading, spacing: 4) {
                if let lang = block.language {
                    Text(lang)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                HStack(spacing: 0) {
                    Text(block.content)
                        .font(.system(.body, design: .monospaced))
                    if isLast && showCursor {
                        CursorView(height: 10)
                    }
                }
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color(.tertiarySystemBackground))
            .clipShape(RoundedRectangle(cornerRadius: 8))
        } else {
            // Text block
            HStack(spacing: 0) {
                Text(block.content)
                    .font(.body)
                if isLast && showCursor {
                    CursorView(height: 12)
                }
            }
        }
    }

    private var showCursor: Bool {
        state.status == .streaming
    }
}
