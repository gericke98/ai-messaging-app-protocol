import SwiftUI

/// Treatment B: a finished A2A burst as a bordered "digest" card. Collapsed it shows overlapped
/// agent avatars + identity-tinted names + a message count; tapping expands it in place to the
/// full run of message groups (provided by `groupContent`, normally `MessageGroupView`), with a
/// quiet collapse control. Display-only; reuses Editorial Warm tokens.
struct A2ACapsuleView<GroupContent: View>: View {
    let capsule: A2ACapsule
    let isExpanded: Bool
    let homeHost: String?
    let onToggle: () -> Void
    @ViewBuilder let groupContent: (MessageGroup) -> GroupContent

    private let maxAvatars = 3

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.Spacing.sm) {
            header
            if isExpanded {
                VStack(alignment: .leading, spacing: Theme.Spacing.lg) {
                    ForEach(capsule.groups) { group in
                        groupContent(group)
                    }
                }
                .padding(.leading, Theme.Spacing.sm)
                Button(action: { Haptics.select(); onToggle() }) {
                    HStack(spacing: Theme.Spacing.xs) {
                        Image(systemName: "chevron.up")
                        Text("Collapse")
                    }
                    .font(Theme.Typography.caption())
                    .foregroundStyle(Theme.Color.inkSecondary)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(Theme.Spacing.md)
        .background(Theme.Color.surface)
        .overlay(
            RoundedRectangle(cornerRadius: Theme.Radius.card)
                .strokeBorder(Theme.Color.surfaceBorder, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: Theme.Radius.card))
        .padding(.horizontal, Theme.Spacing.lg)
        .animation(Theme.Motion.messageAppear, value: isExpanded)
        .contentShape(Rectangle())
        .accessibilityElement(children: isExpanded ? .contain : .ignore)
        .accessibilityLabel(isExpanded ? "Agent deliberation, expanded" : accessibilityLabel)
        .accessibilityHint(isExpanded ? "" : "Double-tap to expand")
    }

    private var header: some View {
        Button(action: { Haptics.select(); onToggle() }) {
            HStack(spacing: Theme.Spacing.sm) {
                avatarStack
                namesAndCount
                Spacer(minLength: 0)
                Image(systemName: isExpanded ? "chevron.up" : "chevron.down")
                    .font(Theme.Typography.caption())
                    .foregroundStyle(Theme.Color.inkSecondary)
            }
        }
        .buttonStyle(.plain)
    }

    private var avatarStack: some View {
        let shown = Array(capsule.agents.prefix(maxAvatars))
        return HStack(spacing: -8) {
            ForEach(Array(shown.enumerated()), id: \.offset) { _, address in
                AvatarView(participantType: "agent", size: 24, address: address,
                           displayName: localPart(address))
                    .overlay(Circle().strokeBorder(Theme.Color.surface, lineWidth: 1.5))
            }
            if capsule.agents.count > maxAvatars {
                Text("+\(capsule.agents.count - maxAvatars)")
                    .font(Theme.Typography.caption())
                    .foregroundStyle(Theme.Color.inkSecondary)
                    .padding(.leading, Theme.Spacing.sm)
            }
        }
    }

    private var namesAndCount: some View {
        VStack(alignment: .leading, spacing: 1) {
            HStack(spacing: Theme.Spacing.xs) {
                let names = Array(capsule.agents.prefix(2))
                ForEach(Array(names.enumerated()), id: \.offset) { idx, address in
                    if idx > 0 {
                        Text(capsule.agents.count == 2 ? "↔" : ",")
                            .font(Theme.Typography.caption())
                            .foregroundStyle(Theme.Color.inkSecondary)
                    }
                    Text(localPart(address))
                        .font(Theme.Typography.senderName())
                        .foregroundStyle(ParticipantColor.color(for: address))
                }
                if capsule.agents.count > 2 {
                    Text("+\(capsule.agents.count - 2)")
                        .font(Theme.Typography.caption())
                        .foregroundStyle(Theme.Color.inkSecondary)
                }
            }
            Text("\(capsule.messageCount) messages")
                .font(Theme.Typography.caption())
                .foregroundStyle(Theme.Color.inkSecondary)
        }
    }

    private func localPart(_ address: String) -> String {
        address.components(separatedBy: "@").first ?? address
    }

    private var accessibilityLabel: String {
        let names = capsule.agents.map(localPart).joined(separator: ", ")
        return "Agent deliberation, \(names), \(capsule.messageCount) messages, collapsed"
    }
}
