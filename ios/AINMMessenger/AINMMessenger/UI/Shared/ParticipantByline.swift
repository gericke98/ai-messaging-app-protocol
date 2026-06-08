import SwiftUI

/// Editorial Warm participant byline shared by settled (`AgentMessageView`) and live
/// (`StreamingMessageView`) agent messages so they can't drift. Renders:
///
///   [serif name]  [· host]?   →  [serif targetName] [· targetHost]?
///
/// - Name is identity-tinted when `isMultiParticipant`, else calm `inkPrimary`.
/// - `· host` suffixes appear only for foreign hosts (different from `homeHost`).
/// - The directional `→ target` tag appears only when `directedTo` is non-nil.
struct ParticipantByline: View {
    let senderAddress: String
    let senderName: String
    var directedTo: String? = nil
    var isMultiParticipant: Bool = false
    var homeHost: String? = nil

    var body: some View {
        HStack(spacing: Theme.Spacing.xs) {
            Text(senderName)
                .font(Theme.Typography.senderName())
                .foregroundStyle(senderColor)

            if let host = senderHostSuffix {
                Text("· \(host)")
                    .font(Theme.Typography.caption())
                    .foregroundStyle(Theme.Color.inkSecondary)
            }

            if let target = directedTo {
                Text("→")
                    .font(Theme.Typography.caption())
                    .foregroundStyle(Theme.Color.inkSecondary)
                Text(localPart(of: target))
                    .font(Theme.Typography.senderName())
                    .foregroundStyle(ParticipantColor.color(for: target))
                if let targetHost = hostSuffix(for: target) {
                    Text("· \(targetHost)")
                        .font(Theme.Typography.caption())
                        .foregroundStyle(Theme.Color.inkSecondary)
                }
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(accessibilityText)
    }

    // MARK: - Derivations

    private var senderColor: Color {
        isMultiParticipant ? ParticipantColor.color(for: senderAddress) : Theme.Color.inkPrimary
    }

    private var senderHostSuffix: String? {
        hostSuffix(for: senderAddress)
    }

    /// Foreign-only host suffix for any address (nil when local or hostless).
    private func hostSuffix(for address: String) -> String? {
        ParticipantProvenance.isForeignHost(address, homeHost: homeHost)
            ? ParticipantProvenance.hostSuffix(of: address)
            : nil
    }

    private func localPart(of address: String) -> String {
        address.components(separatedBy: "@").first ?? address
    }

    /// One spoken phrase: "Researcher, on acme.test, to Assistant, on home.test".
    private var accessibilityText: String {
        var parts = [senderName]
        if let host = senderHostSuffix { parts.append("on \(host)") }
        if let target = directedTo {
            parts.append("to \(localPart(of: target))")
            if let targetHost = hostSuffix(for: target) { parts.append("on \(targetHost)") }
        }
        return parts.joined(separator: ", ")
    }
}
