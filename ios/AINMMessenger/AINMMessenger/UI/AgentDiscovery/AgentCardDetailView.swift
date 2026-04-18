import SwiftUI

/// Full Agent Card profile view per UI-SPEC Component 5.
/// Shows header, capability badges, skills, tools, and a pinned Start Conversation button.
struct AgentCardDetailView: View {
    let identity: String
    let card: AgentCard
    let appState: AppState

    @Environment(\.dismiss) private var dismiss

    var body: some View {
        ScrollView {
            VStack(spacing: 0) {
                headerSection
                capabilityBadges
                skillsSection
                toolsSection
            }
            .padding(.horizontal, 24)
            .padding(.bottom, 80) // Space for pinned button
        }
        .accessibilityIdentifier(UITestID.AgentDiscovery.detail)
        .safeAreaInset(edge: .bottom) {
            startConversationButton
        }
        .navigationBarTitleDisplayMode(.inline)
    }

    // MARK: - Header

    private var headerSection: some View {
        VStack(spacing: 4) {
            AvatarView(participantType: "agent", size: 64)
                .padding(.bottom, 8)

            Text(card.name)
                .font(.title2.bold())

            Text(card.description)
                .font(.body)
                .foregroundStyle(Color(.secondaryLabel))
                .multilineTextAlignment(.center)

            Text(identity)
                .font(.caption)
                .foregroundStyle(Color(.tertiaryLabel))
                .padding(.top, 2)
        }
        .padding(.top, 16)
    }

    // MARK: - Capability Badges

    private var capabilityBadges: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                if card.capabilities.streaming {
                    capabilityBadge("Streaming", color: .green)
                        .accessibilityIdentifier(UITestID.AgentDiscovery.detailBadgeStreaming)
                }
                if card.ainm?.memory.archivalSupported == true {
                    capabilityBadge("Memory", color: .purple)
                }
                if card.ainm?.threading.subThreads == true {
                    capabilityBadge("Sub-threads", color: .blue)
                }
                if card.capabilities.pushNotifications {
                    capabilityBadge("Push", color: .orange)
                }
            }
            .padding(.horizontal, 4)
        }
        .padding(.top, 16)
    }

    private func capabilityBadge(_ text: String, color: Color) -> some View {
        HStack(spacing: 4) {
            Circle()
                .fill(color)
                .frame(width: 6, height: 6)
            Text(text)
                .font(.caption.bold())
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(Color(.tertiarySystemFill))
        .clipShape(Capsule())
    }

    // MARK: - Skills

    private var skillsSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Skills")
                .font(.subheadline.bold())
                .foregroundStyle(Color(.secondaryLabel))
                .accessibilityIdentifier(UITestID.AgentDiscovery.detailSkillsHeader)

            ForEach(card.skills) { skill in
                VStack(alignment: .leading, spacing: 4) {
                    Text(skill.name)
                        .font(.subheadline.bold())

                    Text(skill.description)
                        .font(.body)
                        .foregroundStyle(Color(.secondaryLabel))

                    if let tags = skill.tags, !tags.isEmpty {
                        HStack(spacing: 6) {
                            ForEach(tags, id: \.self) { tag in
                                Text(tag)
                                    .font(.caption.bold())
                                    .padding(.horizontal, 8)
                                    .padding(.vertical, 4)
                                    .background(Color(.tertiarySystemFill))
                                    .clipShape(Capsule())
                            }
                        }
                    }
                }
                .padding(.vertical, 4)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.top, 24)
    }

    // MARK: - Tools

    @ViewBuilder
    private var toolsSection: some View {
        if let tools = card.ainm?.tools, !tools.isEmpty {
            VStack(alignment: .leading, spacing: 12) {
                Text("Tools")
                    .font(.subheadline.bold())
                    .foregroundStyle(Color(.secondaryLabel))

                ForEach(tools) { tool in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(tool.name)
                            .font(.subheadline.bold().monospaced())

                        Text(tool.description)
                            .font(.body)
                            .foregroundStyle(Color(.secondaryLabel))
                    }
                    .padding(.vertical, 4)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.top, 24)
        }
    }

    // MARK: - Start Conversation

    private var startConversationButton: some View {
        Button {
            Task {
                do {
                    let threadId = try await appState.createThread(
                        title: "Chat with \(card.name)",
                        firstMessageText: "Started a conversation with \(card.name)"
                    )
                    appState.selectedThreadId = threadId
                    dismiss()
                } catch {
                    // Handle error silently for now; thread creation failure is rare
                }
            }
        } label: {
            Text("Start Conversation")
                .frame(maxWidth: .infinity)
                .frame(height: 44)
        }
        .buttonStyle(.borderedProminent)
        .padding(.horizontal, 16)
        .padding(.bottom, 8)
        .accessibilityLabel("Start conversation with \(card.name)")
        .accessibilityIdentifier(UITestID.AgentDiscovery.detailStartConversation)
    }
}
