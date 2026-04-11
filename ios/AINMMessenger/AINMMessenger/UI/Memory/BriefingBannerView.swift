import SwiftUI

/// Compact banner shown in a thread when an agent receives a briefing.
/// Tapping navigates to the full memory graph view.
struct BriefingBannerView: View {
    let briefing: AgentBriefing
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 10) {
                Image(systemName: "brain.head.profile")
                    .font(.subheadline)
                    .foregroundStyle(Color.accentColor)
                VStack(alignment: .leading, spacing: 1) {
                    Text("Agent briefed")
                        .font(.caption.bold())
                        .foregroundStyle(Color(.label))
                    Text("\(briefing.scope) scope \u{2022} \(briefing.consentLevel) consent")
                        .font(.caption2)
                        .foregroundStyle(Color(.secondaryLabel))
                }
                Spacer()
                Image(systemName: "chevron.right")
                    .font(.caption2)
                    .foregroundStyle(Color(.tertiaryLabel))
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
            .background(Color.accentColor.opacity(0.08))
        }
        .buttonStyle(.plain)
    }
}
