import SwiftUI

/// In-progress streaming message (UI-SPEC Component 1), rendered as the "living"
/// counterpart of a settled AgentMessageView: identity avatar with a halo while
/// streaming, serif name + prose laid open on the paper, an identity-colored caret
/// at the active block, and a quiet "✺ thinking…" status line. A "Not verified" badge
/// appears if the signed message does not arrive within 5s of stream.end.
struct StreamingMessageView: View {
    let state: StreamingViewModel.StreamingMessageState
    var isMultiParticipant: Bool = false
    var homeHost: String? = nil

    var body: some View {
        HStack(alignment: .top, spacing: Theme.Spacing.sm) {
            AvatarView(
                participantType: "agent",
                size: 30,
                address: state.from,
                displayName: senderName,
                showAgentHalo: showCursor
            )

            VStack(alignment: .leading, spacing: Theme.Spacing.xs) {
                ParticipantByline(
                    senderAddress: state.from,
                    senderName: senderName,
                    directedTo: nil,
                    isMultiParticipant: isMultiParticipant,
                    homeHost: homeHost
                )

                if showCursor {
                    Text("✺ thinking…")
                        .font(Theme.Typography.caption())
                        .foregroundStyle(Theme.Color.inkSecondary)
                        .accessibilityLabel("Agent is thinking")
                }

                VStack(alignment: .leading, spacing: Theme.Spacing.sm) {
                    ForEach(Array(state.blocks.enumerated()), id: \.offset) { index, block in
                        streamingBlockView(block: block, isLast: index == state.blocks.count - 1)
                    }
                }

                if state.status == .notVerified {
                    HStack(spacing: Theme.Spacing.xs) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .font(Theme.Typography.caption())
                        Text("Not verified")
                            .font(Theme.Typography.caption())
                    }
                    .foregroundStyle(Theme.Color.inkSecondary)
                    .accessibilityLabel("Message not verified")
                    .accessibilityHint("The signed message was not received from the server within the expected time.")
                }
            }
        }
        .padding(.horizontal, Theme.Spacing.lg)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(state.status == .streaming ? "Agent is generating a response" : "Agent response")
    }

    // MARK: - Private

    private var senderName: String {
        state.from.components(separatedBy: "@").first ?? state.from
    }

    private var caretColor: Color {
        ParticipantColor.color(for: state.from)
    }

    @ViewBuilder
    private func streamingBlockView(
        block: StreamingViewModel.StreamingBlockState,
        isLast: Bool
    ) -> some View {
        if block.type == "code" {
            VStack(alignment: .leading, spacing: Theme.Spacing.xs) {
                if let lang = block.language {
                    Text(lang)
                        .font(Theme.Typography.caption())
                        .foregroundStyle(Theme.Color.inkSecondary)
                }
                HStack(spacing: 0) {
                    Text(block.content)
                        .font(.system(.body, design: .monospaced))
                        .foregroundStyle(Theme.Color.inkPrimary)
                    if isLast && showCursor {
                        CursorView(height: 10, color: caretColor)
                    }
                }
            }
            .padding(Theme.Spacing.md)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.Color.surface)
            .clipShape(RoundedRectangle(cornerRadius: Theme.Radius.chip))
        } else {
            HStack(alignment: .firstTextBaseline, spacing: 0) {
                Text(block.content)
                    .font(Theme.Typography.agentProse())
                    .foregroundStyle(Theme.Color.inkPrimary)
                if isLast && showCursor {
                    CursorView(height: 12, color: caretColor)
                }
            }
        }
    }

    private var showCursor: Bool {
        state.status == .streaming
    }
}
