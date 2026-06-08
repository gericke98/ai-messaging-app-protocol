import SwiftUI

/// Reusable avatar circle. When an `address` is supplied, renders a per-identity
/// colored circle with initials (so distinct participants are visually separable).
/// Without an address, preserves the prior type-tinted symbol behavior so existing
/// call sites are unchanged.
struct AvatarView: View {
    let participantType: String       // "human" or "agent"
    var size: CGFloat = 36
    var address: String? = nil        // drives per-identity color + initials
    var displayName: String? = nil    // initials source (falls back to address)
    var showAgentHalo: Bool = false   // soft ring for agents (opt-in; used in SP2)

    private var isAgent: Bool { participantType == "agent" }

    var body: some View {
        Group {
            if let address {
                identityAvatar(address: address)
            } else {
                legacyAvatar
            }
        }
        .accessibilityLabel(accessibilityText)
    }

    // MARK: Identity-colored avatar

    @ViewBuilder
    private func identityAvatar(address: String) -> some View {
        let fill = ParticipantColor.color(for: address)
        let initials = ParticipantColor.initials(for: displayName ?? address)
        Circle()
            .fill(fill)
            .frame(width: size, height: size)
            .overlay {
                Text(initials)
                    .font(.system(size: size * 0.42, weight: .semibold, design: .rounded))
                    .foregroundStyle(.white)
            }
            .overlay {
                if showAgentHalo && isAgent {
                    Circle().stroke(fill.opacity(0.35), lineWidth: max(2, size * 0.06))
                        .scaleEffect(1.18)
                }
            }
    }

    // MARK: Legacy type-tinted avatar (unchanged behavior)

    private var legacyAvatar: some View {
        Circle()
            .fill(isAgent ? Color.orange.opacity(0.15) : Color.blue.opacity(0.15))
            .frame(width: size, height: size)
            .overlay {
                Image(systemName: isAgent ? "cpu.fill" : "person.fill")
                    .font(.system(size: size * 0.45))
                    .foregroundStyle(isAgent ? .orange : .blue)
            }
    }

    private var accessibilityText: String {
        if let displayName { return "\(displayName) avatar" }
        return isAgent ? "Agent avatar" : "Human avatar"
    }
}
